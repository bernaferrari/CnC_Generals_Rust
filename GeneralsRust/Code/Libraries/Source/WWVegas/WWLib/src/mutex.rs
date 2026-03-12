// Auto-generated C++ compatibility shim for mutex
use std::sync::{Mutex as StdMutex, MutexGuard};

pub struct Mutex<T> {
    inner: StdMutex<T>,
}

impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: StdMutex::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock().expect("mutex poisoned")
    }
}
