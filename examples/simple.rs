extern crate redux;
use redux::{Store, Reducer};
use std::default::Default;

#[derive(Clone, Debug)]
struct Todo {
	name: &'static str,
}

#[derive(Clone, Debug)]
struct TodoState {
	todos: Vec<Todo>,
}

impl TodoState {
    fn new() -> TodoState {
        TodoState {
            todos: vec![],
        }
    }

	fn push(&mut self, todo: Todo) {
		self.todos.push(todo);
	}
}

#[derive(Clone)]
enum TodoAction {
	Insert(&'static str),
}

impl Default for TodoState {
    fn default() -> Self {
        TodoState::new()
    }
}

impl Reducer for TodoState {
	type Action = TodoAction;
	type Error = String;

	fn reduce(&mut self, action: Self::Action) -> Result<Self, Self::Error> {
		match action {
            TodoAction::Insert(name) => {
                let todo = Todo { name: name, };
                self.push(todo);
                Ok(self.clone())
            },
		}
	}
}

fn main() {
	let store : Store<TodoState> = Store::new(vec![]);
	let action = TodoAction::Insert("Clean the bathroom");
	let _ = store.dispatch(action);

	println!("{:?}", store.get_state());
}
