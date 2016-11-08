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
        let (subs_to_remove, subs_to_use) = self.get_subscriptions();

        // on every subscription callback loop we gather the indexes of cancelled
        // subscriptions; if we leave a loop and have cancelled subscriptions, we'll
        // try to remove them here
        self.try_to_remove_subscriptions(subs_to_remove);

        // actually run the subscriptions here; after this method is over the subs_to_use
        // vec gets dropped, and all the Arcs of subscriptions get decremented
        for subscription in subs_to_use {
            let cb = &subscription.callback;
            cb(&self, &subscription);
        }

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    pub fn subscribe(&self, callback: SubscriptionFunc<T>) -> Arc<Subscription<T>> {
        let subscription = Arc::new(Subscription::new(callback));
        let s = subscription.clone();
        self.subscriptions.write().unwrap().push(s);
        return subscription;
    }

    fn get_subscriptions(&self) -> (Vec<usize>, Vec<Arc<Subscription<T>>>) {
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

        (subs_to_remove, subs_to_use)
    }

    fn try_to_remove_subscriptions(&self, subs_to_remove: Vec<usize>) {
        if subs_to_remove.len() > 0 {
            match self.subscriptions.try_write() {
                Ok(mut subscriptions) => {
                    for sub_index in subs_to_remove {
                        subscriptions.remove(sub_index);
                    }
                },
                _ => {}
            }
        }
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

type SubscriptionFunc<T: Reducer> = Box<Fn(&Store<T>, &Subscription<T>)>;

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

#[cfg(test)]
impl Reducer for usize {
    type Action = usize;
    type Error = String;

    fn reduce(&mut self, _: Self::Action) -> Result<&mut Self, Self::Error> {
        Ok(self)
    }
}

#[test]
fn get_subscriptions() {
    let store : Store<usize> = Store::new(vec![]);
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(0, remove.len());
        assert_eq!(0, subs.len());
    }

    let sub = store.subscribe(Box::new(|_, _| {}));
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(0, remove.len());
        assert_eq!(1, subs.len());
    }

    sub.cancel();
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(1, remove.len());
        assert_eq!(0, subs.len());
    }
}

#[test]
fn try_remove_subscriptions_easy_lock() {
    let store : Store<usize> = Store::new(vec![]);
    let sub = store.subscribe(Box::new(|_, _| {}));
    sub.cancel();

    let (remove, _) = store.get_subscriptions();
    store.try_to_remove_subscriptions(remove);
    let (_, subs) = store.get_subscriptions();
    assert_eq!(0, subs.len());
}

#[test]
fn try_remove_subscriptions_no_lock() {
    let store : Store<usize> = Store::new(vec![]);
    let sub = store.subscribe(Box::new(|_, _| {}));
    sub.cancel();

    let (remove, _) = store.get_subscriptions();
    {
        let subscriptions = store.subscriptions.write().unwrap();
        store.try_to_remove_subscriptions(remove);
    }
    assert_eq!(1, store.subscriptions.read().unwrap().len());
}
