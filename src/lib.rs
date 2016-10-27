use std::cell::RefCell;
use std::rc::Rc;

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

pub struct Store<T: Clone, A: Clone> {
    internal_store: Rc<RefCell<InternalStore<T>>>,
    reducer: Box<Reducer<Action = A, Item = T>>,
    subscriptions: Vec<Box<Fn(&Store<T, A>)>>,
}

struct InternalStore<T: Clone> {
    data: T,
    is_dispatching: bool,
}

impl<T: Clone, A: Clone> Store<T, A> {
    pub fn new(reducer: Box<Reducer<Action = A, Item = T>>) -> Store<T, A> {
        let initial_data = reducer.init();

        Store {
            internal_store: Rc::new(RefCell::new(InternalStore {
                data: initial_data,
                is_dispatching: false,
            })),
            reducer: reducer,
            subscriptions: Vec::new(),
        }
    }

    pub fn dispatch(&self, action: A) -> Result<A, String> {
        let new_data = {
            let internal_store = self.internal_store.borrow();
            if self.internal_store.borrow().is_dispatching {
                return Err(String::from("Can't dispatch during a reduce."));
            }
            self.reducer.reduce(internal_store.data.clone(), action.clone())
        };

        {
            let mut d = self.internal_store.borrow_mut();
            d.is_dispatching = true;
            d.data = new_data;
            d.is_dispatching = false;
        }

        for cb in &self.subscriptions {
            cb(&self);
        }

        Ok(action)
    }

    pub fn get_state(&self) -> T {
        self.internal_store.borrow().data.clone()
    }

    pub fn subscribe(&mut self, callback: Box<Fn(&Store<T, A>)>) {
        self.subscriptions.push(callback);
    }
}
