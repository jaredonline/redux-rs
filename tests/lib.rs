extern crate redux;

use redux::{Reducer, Store};

use std::collections::HashMap;
use std::sync::{Mutex, Arc};

#[derive(Clone)]
struct Todo {
    name: String,
    id: usize,
}

#[derive(Clone)]
struct TodoStore {
    todos: HashMap<usize, Todo>,
    vec: Vec<usize>,
    ticket: usize,
}

impl TodoStore {
    pub fn new() -> TodoStore {
        TodoStore {
            todos: HashMap::new(),
            vec: Vec::new(),
            ticket: 0,
        }
    }

    pub fn ticket(&mut self) -> usize {
        self.ticket += 1;
        self.ticket
    }

    pub fn push(&mut self, todo: Todo) {
        let ticket = todo.id;
        self.todos.insert(ticket, todo);
        self.vec.push(ticket);
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }
}

struct TodoReducer { }

#[derive(Clone)]
enum TodoAction {
    NewTodo { name: String }
}

impl Reducer for TodoReducer {
    type Action = TodoAction;
    type Item   = TodoStore;
    
    fn reduce(&self, data: Self::Item, action: Self::Action) -> Self::Item {
        match action {
            TodoAction::NewTodo { name } => {
                let mut data = data;
                let todo = Todo { name: name, id: data.ticket(), };
                data.push(todo);
                data
            },
            // _ => {}
        }
    }

    fn init(&self) -> Self::Item {
        TodoStore::new()
    }
}

#[test]
fn todo_list() {
    struct PingbackTester {
        counter: usize
    }
    let pingbacker = Arc::new(Mutex::new(PingbackTester { counter: 0 }));

    let reducer = Box::new(TodoReducer {});
    let mut store = Store::new(reducer);
    let pbacker = pingbacker.clone();
    store.subscribe(Box::new(move |store| {
        let mut pingbacker = pingbacker.lock().unwrap();
        let _ = store.get_state();
        pingbacker.counter += 1;
    }));
    
    let action = TodoAction::NewTodo {name: String::from("Grocery Shopping")};
    store.dispatch(action);
    assert_eq!(1, store.get_state().len());
    assert_eq!(1, pbacker.lock().unwrap().counter);
}

#[test]
fn dispatch_from_a_listener() {
    let reducer = Box::new(TodoReducer {});
    let mut store = Store::new(reducer);
    store.subscribe(Box::new(move |store| {
        if store.get_state().len() < 2 {
            let action = TodoAction::NewTodo {name: String::from("Finish that new todo")};
            store.dispatch(action);
        }
    }));
    
    let action = TodoAction::NewTodo {name: String::from("Grocery Shopping")};
    store.dispatch(action);
    assert_eq!(2, store.get_state().len());
}
