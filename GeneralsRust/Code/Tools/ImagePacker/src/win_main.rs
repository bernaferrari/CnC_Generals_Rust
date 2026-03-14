//! Minimal parity shim for C++ `WinMain.h` global `ApplicationHInstance`.

use std::sync::atomic::{AtomicUsize, Ordering};

pub static APPLICATION_HINSTANCE: AtomicUsize = AtomicUsize::new(0);

pub fn set_application_hinstance(value: usize) {
    APPLICATION_HINSTANCE.store(value, Ordering::SeqCst);
}

pub fn application_hinstance() -> usize {
    APPLICATION_HINSTANCE.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::{application_hinstance, set_application_hinstance};

    #[test]
    fn stores_and_reads_hinstance() {
        set_application_hinstance(1234);
        assert_eq!(application_hinstance(), 1234);
    }
}
