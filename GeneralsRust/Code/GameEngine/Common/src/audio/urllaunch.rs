// FILE: audio/urllaunch.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/URLLaunch.h + Source/Common/Audio/URLLaunch.cpp
//
// PARITY_NOTE: The C++ uses Win32 registry/ShellExecute/CreateProcess to
// launch URLs.  The Rust port uses platform-native commands (open on macOS,
// xdg-open on Linux, registry-based on Windows).  make_escaped_url and
// launch_url signatures are preserved.

pub use crate::common::audio::url_launch::*;
