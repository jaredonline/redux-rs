use std::sync::{Arc, Mutex};

pub trait Reducer {
    type Action;
    type Item;

    fn reduce(&self, Self::Item, Self::Action) -> Self::Item;
    fn init(&self) -> Self::Item;
}

pub struct Store<T: Clone, A: Clone> {
    internal_store: Arc<Mutex<InternalStore<T>>>,
    reducer: Box<Reducer<Action = A, Item = T>>,
    subscriptions: Vec<Arc<Subscription<T, A>>>,
}

unsafe impl<T: Clone, A: Clone> Send for Store<T, A> {}
unsafe impl<T: Clone, A: Clone> Sync for Store<T, A> {}

impl<T: Clone, A: Clone> Store<T, A> {
    pub fn new(reducer: Box<Reducer<Action = A, Item = T>>) -> Store<T, A> {
        let initial_data = reducer.init();

        Store {
            internal_store: Arc::new(Mutex::new(InternalStore {
                data: initial_data,
                is_dispatching: false,
            })),
            reducer: reducer,
            subscriptions: Vec::new(),
        }
    }

    pub fn dispatch(&self, action: A) -> Result<A, String> {
        match self.internal_store.try_lock() {
            Ok(mut guard) => {
                let _ = guard.dispatch(action.clone(), &self.reducer);
            },
            Err(_) => {
                return Err(String::from("Can't dispatch during a reduce. The internal data is locked."));
            }
        }

        for subscription in &self.subscriptions {
            let active = {
                *subscription.active.lock().unwrap()
            };
            if active {
                let ref cb = subscription.callback;
                cb(&self);
            }
        }

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    pub fn subscribe(&mut self, callback: Box<Fn(&Store<T, A>)>) -> Arc<Subscription<T, A>> {
        let subscription = Arc::new(Subscription::new(callback));
        self.subscriptions.push(subscription.clone());
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
}
