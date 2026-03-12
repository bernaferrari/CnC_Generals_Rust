//! Callback hook utilities (ported from WWLib CallbackHook.h).

/// Base trait for callback hooks.
pub trait CallbackHook {
    fn do_callback(&self) -> bool;
}

/// Generic callback wrapper that invokes a function with stored user data.
pub struct Callback<T: Clone> {
    callback: Option<fn(T) -> bool>,
    user_data: T,
}

impl<T: Clone> Callback<T> {
    pub fn new(callback: Option<fn(T) -> bool>, user_data: T) -> Self {
        Self {
            callback,
            user_data,
        }
    }
}

impl<T: Clone> CallbackHook for Callback<T> {
    fn do_callback(&self) -> bool {
        if let Some(cb) = self.callback {
            return cb(self.user_data.clone());
        }
        false
    }
}
