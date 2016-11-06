extern crate redux;
use redux::{Store, Reducer};

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

struct TodoReducer {}
impl Reducer for TodoReducer {
	type Action = TodoAction;
	type Item = TodoState;

	fn reduce(&self, data: Self::Item, action: Self::Action) -> Self::Item {
		match action {
            TodoAction::Insert(name) => {
                let mut data = data;
                let todo = Todo { name: name, };
                data.push(todo);
                data
            },
		}
	}

    fn init(&self) -> Self::Item {
        TodoState::new()
    }
}

fn main() {
    let reducer = Box::new(TodoReducer{});
	let store = Store::new(reducer, vec![]);
	let action = TodoAction::Insert("Clean the bathroom");
	let _ = store.dispatch(action);

	println!("{:?}", store.get_state());
}
