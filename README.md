# redux-rs

![travis-ci](https://travis-ci.org/jaredonline/redux-rs.svg)

An attempt at a uni-directional state flow written in Rust, heavily based in [redux-js](http://redux.js.org/).

## Usage

Here's a simple example of using a store and reducer to make a quick Todo list (you can run this by running `cargo run --example simple` or view the code [here](https://github.com/jaredonline/redux-rs/blob/master/examples/simple.rs)).

```rust
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

	fn reduce(&mut self, action: Self::Action) -> Result<&mut Self, Self::Error> {
		match action {
            TodoAction::Insert(name) => {
                let todo = Todo { name: name, };
                self.push(todo);
            },
		}

        Ok(self)
	}
}

fn main() {
	let store : Store<TodoState> = Store::new(vec![]);
	let action = TodoAction::Insert("Clean the bathroom");
	let _ = store.dispatch(action);

	println!("{:?}", store.get_state());
}
```
