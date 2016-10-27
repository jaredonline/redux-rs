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
    data: Rc<RefCell<T>>,
    reducer: Box<Reducer<Action = A, Item = T>>,
    subscriptions: Vec<Box<Fn(&Store<T, A>)>>,
    is_dispatching: bool,
}

impl<T: Clone, A: Clone> Store<T, A> {
    pub fn new(reducer: Box<Reducer<Action = A, Item = T>>) -> Store<T, A> {
        let initial_data = Rc::new(RefCell::new(reducer.init()));

        Store {
            data: initial_data,
            reducer: reducer,
            subscriptions: Vec::new(),
            is_dispatching: false,
        }
    }

    pub fn dispatch(&self, action: A) -> A {
        if self.is_dispatching {
            panic!("Can't dispatch during a reduce.");
        }

        let new_data = {
            let data_clone = self.data.borrow().clone();
            self.reducer.reduce(data_clone, action.clone())
        };
        {
            let mut d = self.data.borrow_mut();
            *d = new_data;
        }

        for cb in &self.subscriptions {
            cb(&self);
        }

        action
    }

    pub fn get_state(&self) -> T {
        self.data.borrow().clone()
    }

    pub fn subscribe(&mut self, callback: Box<Fn(&Store<T, A>)>) {
        self.subscriptions.push(callback);
    }
}
