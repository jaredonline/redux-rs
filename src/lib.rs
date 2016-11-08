use std::sync::{Arc, Mutex, RwLock};
use std::default::Default;
use std::fmt::Display;

pub trait Reducer: Clone + Default {
    type Action: Clone;
    type Error: Display;

    fn reduce(&mut self, Self::Action) -> Result<&mut Self, Self::Error>;
}

pub trait Middleware<T: Reducer> {
    fn before(&self, store: &Store<T>, action: T::Action);
    fn after(&self, store: &Store<T>, action: T::Action);
}

pub struct Store<T: Reducer> {
    internal_store: Mutex<InternalStore<T>>,
    subscriptions: Arc<RwLock<Vec<Arc<Subscription<T>>>>>,
    middlewares: Vec<Box<Middleware<T>>>,
}

unsafe impl<T: Reducer> Send for Store<T> {}
unsafe impl<T: Reducer> Sync for Store<T> {}

impl<T: 'static + Reducer> Store<T> {
    pub fn new(middlewares: Vec<Box<Middleware<T>>>) -> Store<T> {
        let initial_data = T::default();

        Store {
            internal_store: Mutex::new(InternalStore {
                data: initial_data,
                is_dispatching: false,
            }),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            middlewares: middlewares,
        }
    }

    pub fn dispatch(&self, action: T::Action) -> Result<T::Action, String> {
        for middleware in &self.middlewares {
            middleware.before(&self, action.clone());
        }
        match self.internal_store.try_lock() {
            Ok(mut guard) => {
                match guard.dispatch(action.clone()) {
                    Err(e) => {
                        return Err(format!("Error during dispatch: {}", e));
                    },
                    _ => {}
                }
            },
            Err(_) => {
                return Err(String::from("Can't dispatch during a reduce. The internal data is locked."));
            }
        }
        // Weird looping to go backwards so that we emulate Redux.js way of handling
        // middleware: wrap down the chain of middlewares, then back up.
        for i in (0 .. self.middlewares.len()).into_iter().rev() {
            let middleware = &self.middlewares[i];
            middleware.after(&self, action.clone());
        }

        // snapshot the active subscriptions here before calling them. This both
        // emulates the Redux.js way of doing them *and* frees up the lock so
        // that a subscription can cause another subscription; also use this
        // loop to grab the ones that are safe to remove and try to remove them
        // after this
        let mut i = 0;
        let mut subs_to_remove = vec![];
        let mut subs_to_use = vec![];
        {
            let subscriptions = self.subscriptions.read().unwrap();
            for subscription in &(*subscriptions) {
                if subscription.is_active() {
                    subs_to_use.push(subscription.clone());
                } else {
                    subs_to_remove.push(i);
                }
                i += 1;
            }
        }

        // on every subscription callback loop we gather the indexes of cancelled
        // subscriptions; if we leave a loop and have cancelled subscriptions, we'll
        // try to remove them here
        if subs_to_remove.len() > 0 {
            match self.subscriptions.try_write() {
                Ok(mut subscriptions) => {
                    for j in subs_to_remove {
                        subscriptions.remove(j);
                    }
                },
                _ => {}
            }
        }

        // actually run the subscriptions here; after this method is over the subs_to_use
        // vec gets dropped, and all the Arcs of subscriptions get decremented
        for subscription in subs_to_use {
            let cb = &subscription.callback;
            cb(&self);
        }

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    pub fn subscribe(&self, callback: Box<Fn(&Store<T>)>) -> Arc<Subscription<T>> {
        let subscription = Arc::new(Subscription::new(callback));
        let s = subscription.clone();
        self.subscriptions.write().unwrap().push(s);
        return subscription;
    }
}

struct InternalStore<T: Reducer> {
    data: T,
    is_dispatching: bool,
}

impl<T: Reducer> InternalStore<T> {
    fn dispatch(&mut self, action: T::Action) -> Result<T::Action, String> {
        if self.is_dispatching {
            return Err(String::from("Can't dispatch during a reduce."));
        }

        self.is_dispatching = true;
        match self.data.reduce(action.clone()) {
            Ok(_) => {}
            Err(e) => {
                return Err(format!("{}", e));
            }
        }
        self.is_dispatching = false;

        Ok(action)
    }
}

type SubscriptionFunc<T: Reducer> = Box<Fn(&Store<T>)>;

pub struct Subscription<T: Reducer> {
    callback: SubscriptionFunc<T>,
    active: Mutex<bool>,
}

unsafe impl<T: Reducer> Send for Subscription<T> {}
unsafe impl<T: Reducer> Sync for Subscription<T> {}

impl<T: Reducer> Subscription<T> {
    fn new(callback: SubscriptionFunc<T>) -> Subscription<T> {
        Subscription {
            callback: callback,
            active: Mutex::new(true),
        }
    }

    pub fn cancel(&self) {
        let mut active = self.active.lock().unwrap();
        *active = false;
    }

    pub fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}
