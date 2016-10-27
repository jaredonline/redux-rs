use std::sync::{Arc, Mutex};

// state = data store
// action = object that triggers a change
// reducer = state + action = new state
//
// let reducer = Reducer::new(|| {})
// let store = Store::new(reducer);
// let action = Action { name: "FOO", data: ... }
// store.dispatch(action);
//
pub trait Reducer {
    type Action;
    type Item;

    fn reduce(&self, Self::Item, Self::Action) -> Self::Item;
    fn init(&self) -> Self::Item;
}

pub type ReducerBox<T: Clone, A: Clone> = Box<Reducer<Action = A, Item = T>>;

pub struct Store<T: Clone, A: Clone> {
    internal_store: Arc<Mutex<InternalStore<T>>>,
    reducer: ReducerBox<T, A>,
    subscriptions: Vec<Box<Fn(&Store<T, A>)>>,
}

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
                guard.dispatch(action.clone(), &self.reducer);
            },
            Err(e) => {
                return Err(String::from("Can't dispatch during a reduce. The internal data is locked."));
            }
        }

        for cb in &self.subscriptions {
            cb(&self);
        }

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    pub fn subscribe(&mut self, callback: Box<Fn(&Store<T, A>)>) {
        self.subscriptions.push(callback);
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
