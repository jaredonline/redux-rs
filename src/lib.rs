use std::sync::{Arc, Mutex, RwLock};
use std::default::Default;
use std::fmt::Display;

/// The `Reducer` trait is meant to be applied to the object that contains your
/// applications state. Because each application will have their own custom state
/// to track, we don't provide a sort of state object in redux-rs.
///
/// redux-rs expects a 1:1:1 mapping between your Store, your State and your Reducer
///
/// ## Types
///
/// `Reducer` requires you provide two types:
///  - `Action` is the type of action your `Reducer` reduces
///  - `Error` the type of error this `Reducer` can return
///
/// ## Required traits
///
/// `Reducer` requires your type implements `Clone` and `Default`.
///
/// ## Example
/// 
/// Here's an example that provides a state object, implements Reducer on it and
/// creates the store:
///
/// ```
/// # #[allow(dead_code)]
/// use redux::{Reducer, Store};
///
/// #[derive(Clone, Default)]
/// struct MyState {
///     foo: usize,
///     bar: usize,
/// }
///
/// impl Reducer for MyState {
///     type Action = String;
///     type Error = String;
///
///     fn reduce(&mut self, action: Self::Action) -> Result<Self, Self::Error> {
///         Ok(self.clone())
///     }
/// }
///
/// fn main() {
///     let store : Store<MyState> = Store::new(vec![]);
/// }
/// ```
pub trait Reducer: Clone + Default {
    /// The type of action that this reducer can accept, probably an enum
    type Action: Clone;

    /// The type of error this reducer can return in the `Result`
    type Error: Display;

    /// Reduce a given state based upon an action. This won't be called externally
    /// because your application will never have a reference to the state object
    /// directly. Instead, it'll be called with you call `store.dispatch`.
    fn reduce(&mut self, Self::Action) -> Result<Self, Self::Error>;
}

fn build_next<T: 'static + Reducer>(next: DispatchFunc<T>, middleware: Box<Middleware<T>>) -> DispatchFunc<T> {
    Box::new(move |store, action| {
        middleware.dispatch(store, action, &next)
    })
}

/// The `Store` is the main access point for your application. As soon as you
/// initialize your `Store` it will start your state in the default state and
/// allow you to start dispatching events to it.
///
/// ## Example
///
/// ```
/// # #[allow(dead_code)]
/// use redux::{Reducer, Store};
///
/// #[derive(Clone, Debug)]
/// struct Todo {
/// 	name: &'static str,
/// }
/// 
/// #[derive(Clone, Debug)]
/// struct TodoState {
/// 	todos: Vec<Todo>,
/// }
/// 
/// impl TodoState {
///     fn new() -> TodoState {
///         TodoState {
///             todos: vec![],
///         }
///     }
/// 
/// 	fn push(&mut self, todo: Todo) {
/// 		self.todos.push(todo);
/// 	}
/// }
/// 
/// #[derive(Clone)]
/// enum TodoAction {
/// 	Insert(&'static str),
/// }
/// 
/// impl Default for TodoState {
///     fn default() -> Self {
///         TodoState::new()
///     }
/// }
/// 
/// impl Reducer for TodoState {
/// 	type Action = TodoAction;
/// 	type Error = String;
/// 
/// 	fn reduce(&mut self, action: Self::Action) -> Result<Self, Self::Error> {
/// 		match action {
///             TodoAction::Insert(name) => {
///                 let todo = Todo { name: name, };
///                 self.push(todo);
///             },
/// 		}
/// 
///         Ok(self.clone())
/// 	}
/// }
/// 
/// fn main() {
/// 	let store : Store<TodoState> = Store::new(vec![]);
/// 	let action = TodoAction::Insert("Clean the bathroom");
/// 	let _ = store.dispatch(action);
/// 
/// 	println!("{:?}", store.get_state());
/// }
/// ```
pub struct Store<T: Reducer> {
    internal_store: Arc<Mutex<InternalStore<T>>>,
    subscriptions: Arc<RwLock<Vec<Arc<Subscription<T>>>>>,
    dispatch_chain: DispatchFunc<T>,
}

// Would love to get rid of these someday
unsafe impl<T: Reducer> Send for Store<T> {}
unsafe impl<T: Reducer> Sync for Store<T> {}

impl<T: 'static + Reducer> Store<T> {
    /// Initialize a new `Store`. 
    pub fn new(middlewares: Vec<Box<Middleware<T>>>) -> Store<T> {
        let initial_data = T::default();
        let internal = Arc::new(Mutex::new(InternalStore {
            data: initial_data,
            is_dispatching: false,
        }));
        let is = internal.clone();
        let mut next : DispatchFunc<T> = Box::new(move |_, action| {
            match is.try_lock() {
                Ok(mut guard) => {
                    guard.dispatch(action.clone())
                },
                Err(_) => {
                    Err(String::from("Can't dispatch during a reduce. The internal data is locked."))
                }
            }
        });
        for middleware in middlewares {
            next = build_next(next, middleware);
        }

        Store {
            internal_store: internal,
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            dispatch_chain: next,
        }
    }

    /// Dispatch an event to the stores, returning an `Result`. Only one dispatch
    /// can be happening at a time.
    pub fn dispatch(&self, action: T::Action) -> Result<T::Action, String> {
        let ref dispatch = self.dispatch_chain;
        match dispatch(&self, action.clone()) {
            Err(e) => return Err(format!("Error during dispatch: {}", e)),
            _ => {}
        }

        // snapshot the active subscriptions here before calling them. This both
        // emulates the Redux.js way of doing them *and* frees up the lock so
        // that a subscription can cause another subscription; also use this
        // loop to grab the ones that are safe to remove and try to remove them
        // after this
        let (subs_to_remove, subs_to_use) = self.get_subscriptions();

        // on every subscription callback loop we gather the indexes of cancelled
        // subscriptions; if we leave a loop and have cancelled subscriptions, we'll
        // try to remove them here
        self.try_to_remove_subscriptions(subs_to_remove);

        // actually run the subscriptions here; after this method is over the subs_to_use
        // vec gets dropped, and all the Arcs of subscriptions get decremented
        for subscription in subs_to_use {
            let cb = &subscription.callback;
            cb(&self, &subscription);
        }

        Ok(action)
    }

    /// Returns a `Clone` of the store's state. If called during a dispatch, this
    /// will block until the dispatch is over.
    pub fn get_state(&self) -> T {
        self.internal_store.lock().unwrap().data.clone()
    }

    /// Create a new subscription to this store. Subscriptions are called for every
    /// dispatch made. 
    /// 
    /// ## Nested subscriptions
    /// 
    /// Its possible to subscribe to a store from within a currently called 
    /// subscription:
    /// 
    /// ```
    /// # #[allow(dead_code)]
    /// # use redux::{Reducer, Store};
    /// #
    /// # #[derive(Clone, Default)]
    /// # struct Foo {}
    /// # impl Reducer for Foo {
    /// #     type Action = usize;
    /// #     type Error = String;
    /// #     
    /// #     fn reduce(&mut self, _: Self::Action) -> Result<Self, Self::Error> {
    /// #         Ok(self.clone())
    /// #     }
    /// # }
    /// #
    /// # let store : Store<Foo> = Store::new(vec![]);
    /// store.subscribe(Box::new(|store, _| {
    ///     store.subscribe(Box::new(|_, _| { }));
    /// }));
    /// ```
    ///
    /// The nested subscription won't be called until the next dispatch.
    ///
    /// ## Snapshotting subscriptions
    /// 
    /// Subscriptions are snap-shotted immediately after the reducer and middlewares
    /// finish and before the subscriptions are called, so any subscriptions made
    /// during a subscription callback won't be fired until the next dispatch
    ///
    /// ## Return value
    /// 
    /// This method returns a `Subscription` wrapped in an `Arc` because both
    /// the caller of the method and the internal list of subscriptions need
    /// a reference to it
    pub fn subscribe(&self, callback: SubscriptionFunc<T>) -> Arc<Subscription<T>> {
        let subscription = Arc::new(Subscription::new(callback));
        let s = subscription.clone();
        self.subscriptions.write().unwrap().push(s);
        return subscription;
    }

    fn get_subscriptions(&self) -> (Vec<usize>, Vec<Arc<Subscription<T>>>) {
        let mut i = 0;
        let mut subs_to_remove = vec![];
        let mut subs_to_use = vec![];
        {
            let subscriptions = self.subscriptions.read().unwrap();
            for subscription in &(*subscriptions) {
                if subscription.is_active() {
                    subs_to_use.push(subscription.clone());
                } else {
                    subs_to_remove.push(i);
                }
                i += 1;
            }
        }

        (subs_to_remove, subs_to_use)
    }

    fn try_to_remove_subscriptions(&self, subs_to_remove: Vec<usize>) {
        if subs_to_remove.len() > 0 {
            match self.subscriptions.try_write() {
                Ok(mut subscriptions) => {
                    for sub_index in subs_to_remove {
                        subscriptions.remove(sub_index);
                    }
                },
                _ => {}
            }
        }
    }
}

struct InternalStore<T: Reducer> {
    data: T,
    is_dispatching: bool,
}

impl<T: Reducer> InternalStore<T> {
    fn dispatch(&mut self, action: T::Action) -> Result<T, String> {
        if self.is_dispatching {
            return Err(String::from("Can't dispatch during a reduce."));
        }

        self.is_dispatching = true;
        match self.data.reduce(action.clone()) {
            Ok(_) => {}
            Err(e) => {
                return Err(format!("{}", e));
            }
        }
        self.is_dispatching = false;

        Ok(self.data.clone())
    }
}

type SubscriptionFunc<T: Reducer> = Box<Fn(&Store<T>, &Subscription<T>)>;

/// Represents a subscription to a `Store` which can be cancelled.
pub struct Subscription<T: Reducer> {
    callback: SubscriptionFunc<T>,
    active: Mutex<bool>,
}

unsafe impl<T: Reducer> Send for Subscription<T> {}
unsafe impl<T: Reducer> Sync for Subscription<T> {}

impl<T: Reducer> Subscription<T> {
    fn new(callback: SubscriptionFunc<T>) -> Subscription<T> {
        Subscription {
            callback: callback,
            active: Mutex::new(true),
        }
    }

    /// Cancels a subscription which means it will no longer be called on a 
    /// dispatch and it will be removed from the internal list of subscriptions
    /// at the next available time.
    ///
    /// A cancelled subscription cannot be re-instated
    pub fn cancel(&self) {
        let mut active = self.active.lock().unwrap();
        *active = false;
    }

    /// Returns whether or not a subscription has been cancelled.
    pub fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}

pub type DispatchFunc<T: Reducer> = Box<Fn(&Store<T>, T::Action) -> Result<T, String>>;

/// A decent approximation of a redux-js middleware wrapper. This lets you have
/// wrap calls to dispatch, performing actions right before and right after a
/// call. Each call to dispatch in a Store will loop the middlewares, calling
/// before, then call the dispatch, then loop the middlewares in reverse order
/// calling after.
///
/// ## Example:
///
/// ```
/// # #[allow(dead_code)]
/// # use redux::{Store, Reducer, Middleware, DispatchFunc};
/// #
/// # #[derive(Clone, Debug)]
/// # enum FooAction {}
/// #
/// # #[derive(Clone, Default, Debug)]
/// # struct Foo {}
/// # impl Reducer for Foo {
/// #   type Action = FooAction;
/// #   type Error = String;
/// #
/// #   fn reduce(&mut self, _: Self::Action) -> Result<Self, Self::Error> {
/// #       Ok(self.clone())
/// #   }
/// # }
///
/// struct Logger{}
/// impl Middleware<Foo> for Logger {
///     fn dispatch(&self, store: &Store<Foo>, action: FooAction, next: &DispatchFunc<Foo>) -> Result<Foo, String> {
///         println!("Called action: {:?}", action);
///         println!("State before action: {:?}", store.get_state());
///         let result = next(store, action);
///         println!("State after action: {:?}", store.get_state());
///
///         result
///     }
/// }
///
/// let logger = Box::new(Logger{});
/// let store : Store<Foo> = Store::new(vec![logger]);
/// ```
pub trait Middleware<T: Reducer> {
    fn dispatch(&self, store: &Store<T>, action: T::Action, next: &DispatchFunc<T>) -> Result<T, String>;
}

#[cfg(test)]
impl Reducer for usize {
    type Action = usize;
    type Error = String;

    fn reduce(&mut self, _: Self::Action) -> Result<Self, Self::Error> {
        Ok(self.clone())
    }
}

#[test]
fn get_subscriptions() {
    let store : Store<usize> = Store::new(vec![]);
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(0, remove.len());
        assert_eq!(0, subs.len());
    }

    let sub = store.subscribe(Box::new(|_, _| {}));
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(0, remove.len());
        assert_eq!(1, subs.len());
    }

    sub.cancel();
    {
        let (remove, subs) = store.get_subscriptions();
        assert_eq!(1, remove.len());
        assert_eq!(0, subs.len());
    }
}

#[test]
fn try_remove_subscriptions_easy_lock() {
    let store : Store<usize> = Store::new(vec![]);
    let sub = store.subscribe(Box::new(|_, _| {}));
    sub.cancel();

    let (remove, _) = store.get_subscriptions();
    store.try_to_remove_subscriptions(remove);
    let (_, subs) = store.get_subscriptions();
    assert_eq!(0, subs.len());
    assert_eq!(0, store.subscriptions.read().unwrap().len());
}

#[test]
fn try_remove_subscriptions_no_lock() {
    let store : Store<usize> = Store::new(vec![]);
    let sub = store.subscribe(Box::new(|_, _| {}));
    sub.cancel();

    let (remove, _) = store.get_subscriptions();
    {
        let subscriptions = store.subscriptions.write().unwrap();
        store.try_to_remove_subscriptions(remove);
    }
    assert_eq!(1, store.subscriptions.read().unwrap().len());
}
