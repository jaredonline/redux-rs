use std::collections::HashMap;
use std::sync::Arc;

pub enum ActionData {
    Int(usize),
    Array(Vec<ActionData>),
    None,
}
pub struct Action {
    name: &'static str,
    data: ActionData,
}
impl Action {
    fn new(name: &'static str, data: ActionData) -> Action {
        Action {
            name: name,
            data: data,
        }
    }
}

pub trait State {
    fn reduce(&self, &Action);
}

pub struct Dispatcher {
    states: Vec<Box<State>>,
}

impl Dispatcher {
    fn dispatch(&self, action: &Action) {
        for state in &self.states {
            state.reduce(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(PartialEq, Debug, Clone)]
    struct NoState {
    }
    impl State for NoState {
        fn reduce(&self, _: &Action) {
        }
    }

    #[test]
    fn it_works() {
        let state = NoState{};
        let action = Action::new("no op", ActionData::None);
        assert_eq!(state, state.reduce(&action));
    }

    #[derive(PartialEq, Debug, Clone)]
    struct CounterState {
        count: usize,
    }
    impl State for CounterState {
        fn reduce(&self, action: &Action) {
            match action.name {
                "ADD" => {
                    match action.data {
                        ActionData::Int(i) => {
                            self.count += i;
                        },
                        _ => {}
                    }
                    state
                },
                _ => { }
            }
        }
    }

    #[test]
    fn counter() {
        let start_state = CounterState {
            count: 0,
        };
        let action = Action::new("ADD", ActionData::Int(1));
        let end_state = start_state.reduce(&action);
        let result = CounterState {
            count: 1,
        };
        assert_eq!(end_state, result);

        let action2 = Action::new("ADD", ActionData::Int(3));
        let end_state2 = end_state.reduce(&action2);
        let result2 = CounterState {
            count: 4,
        };
        assert_eq!(end_state2, result2);
    }
}
