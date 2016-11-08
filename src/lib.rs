use std::sync::{Arc, Mutex, RwLock};
use std::thread;
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
                let _ = guard.dispatch(action.clone());
            },
            Err(_) => {
                return Err(String::from("Can't dispatch during a reduce. The internal data is locked."));
            }
        }
        for i in (0 .. self.middlewares.len()).into_iter().rev() {
            let middleware = &self.middlewares[i];
            middleware.after(&self, action.clone());
        }

        let mut i = 0;
        let mut subs_to_remove = vec![];
        {
            let subscriptions = self.subscriptions.read().unwrap();
            for subscription in &(*subscriptions) {
                if subscription.is_active() {
                    let ref cb = subscription.callback;
                    cb(&self);
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

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    pub fn subscribe(&self, callback: Box<Fn(&Store<T>)>) -> Arc<Subscription<T>> {
        let subscription = Arc::new(Subscription::new(callback));
        let s = subscription.clone();
        match self.subscriptions.try_write() {
            Err(_) => {
                let subs = self.subscriptions.clone();
                // TODO: This thread causes a race condition... if you add a new
                // subscription to a store during a dispatch, this subscriber might
                // not be available before the next dispatch is called (the next
                // dispatch might fire before this thread can obtain the write
                // lock on the subscriptions
                thread::spawn(move || {
                    subs.write().unwrap().push(s);
                });
            },
            Ok(mut guard) => guard.push(s),
        }
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
