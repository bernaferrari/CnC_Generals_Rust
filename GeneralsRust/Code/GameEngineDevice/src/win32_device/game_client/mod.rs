//! Win32 Device Game Client Module
//!
//! Contains Windows-specific game client functionality.

pub mod win32_keyboard;
pub mod win32_mouse;
pub mod win32_di_keyboard;
pub mod win32_di_mouse;

pub use win32_keyboard::*;
pub use win32_mouse::*;
pub use win32_di_keyboard::*;
pub use win32_di_mouse::*;