use std::sync::{Arc, Mutex, RwLock};

pub trait Reducer {
    type Action;
    type Item;

    fn reduce(&self, Self::Item, Self::Action) -> Self::Item;
    fn init(&self) -> Self::Item;
}

pub trait Middleware<T: Clone, A: Clone> {
    fn before(&self, store: &Store<T, A>, action: A);
    fn after(&self, store: &Store<T, A>, action: A);
}

pub struct Store<T: Clone, A: Clone> {
    internal_store: Mutex<InternalStore<T>>,
    reducer: Box<Reducer<Action = A, Item = T>>,
    subscriptions: RwLock<Vec<Arc<Subscription<T, A>>>>,
    middlewares: Vec<Box<Middleware<T, A>>>,
}

unsafe impl<T: Clone, A: Clone> Send for Store<T, A> {}
unsafe impl<T: Clone, A: Clone> Sync for Store<T, A> {}

impl<T: Clone, A: Clone> Store<T, A> {
    pub fn new(reducer: Box<Reducer<Action = A, Item = T>>, middlewares: Vec<Box<Middleware<T, A>>>) -> Store<T, A> {
        let initial_data = reducer.init();

        Store {
            internal_store: Mutex::new(InternalStore {
                data: initial_data,
                is_dispatching: false,
            }),
            reducer: reducer,
            subscriptions: RwLock::new(Vec::new()),
            middlewares: middlewares,
        }
    }

    pub fn dispatch(&self, action: A) -> Result<A, String> {
        for middleware in &self.middlewares {
            middleware.before(&self, action.clone());
        }
        match self.internal_store.try_lock() {
            Ok(mut guard) => {
                let _ = guard.dispatch(action.clone(), &self.reducer);
            },
            Err(_) => {
                return Err(String::from("Can't dispatch during a reduce. The internal data is locked."));
            }
        }
        for middleware in &self.middlewares {
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

    pub fn subscribe(&self, callback: Box<Fn(&Store<T, A>)>) -> Arc<Subscription<T, A>> {
        let subscription = Arc::new(Subscription::new(callback));
        {
            self.subscriptions.write().unwrap().push(subscription.clone());
        }
        return subscription;
    }
}

struct InternalStore<T: Clone> {
    data: T,
    is_dispatching: bool,
}

impl<T: Clone> InternalStore<T> {
    fn dispatch<A: Clone>(&mut self, action: A, reducer: &Box<Reducer<Action = A, Item = T>>) -> Result<A, String> {
        if self.is_dispatching {
            return Err(String::from("Can't dispatch during a reduce."));
        }

        let data = self.data.clone();
        self.is_dispatching = true;
        self.data = reducer.reduce(data.clone(), action.clone());
        self.is_dispatching = false;

        Ok(action)
    }
}

type SubscriptionFunc<T: Clone, A: Clone> = Box<Fn(&Store<T, A>)>;

pub struct Subscription<T: Clone, A: Clone> {
    callback: SubscriptionFunc<T, A>,
    active: Mutex<bool>,
}

impl<T: Clone, A: Clone> Subscription<T, A> {
    pub fn new(callback: SubscriptionFunc<T, A>) -> Subscription<T, A> {
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
