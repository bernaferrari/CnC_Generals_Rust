// Auto-generated C++ compatibility shim for signal/observer
use std::sync::{Arc, Mutex};

pub struct Signaler {
    listeners: Arc<Mutex<Vec<Box<dyn Fn() + Send + Sync>>>>,
}

impl Signaler {
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn connect<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.listeners
            .lock()
            .expect("signaler poisoned")
            .push(Box::new(f));
    }

    pub fn emit(&self) {
        for f in self.listeners.lock().expect("signaler poisoned").iter() {
            f();
        }
    }
}
