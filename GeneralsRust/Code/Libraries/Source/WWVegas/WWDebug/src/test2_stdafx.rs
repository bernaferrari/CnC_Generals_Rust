#[cfg(windows)]
pub mod windows {
    pub use windows_sys::Win32::Foundation::*;
    pub use windows_sys::Win32::System::SystemServices::*;
    pub use windows_sys::Win32::UI::WindowsAndMessaging::*;
}
