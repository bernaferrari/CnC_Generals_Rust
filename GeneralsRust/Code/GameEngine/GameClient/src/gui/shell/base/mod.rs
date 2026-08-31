//! # Shell Menu System
//!
//! This module provides the shell menu system for managing UI screens and menu navigation.
//! It implements a stack-based approach for screen transitions with proper initialization,
//! shutdown, and animation support.
//!
//! ## Features
//! - Stack-based screen management with push/pop operations
//! - Proper initialization and shutdown lifecycle for screens
//! - Animation system integration for smooth transitions
//! - Menu scheme support for theming
//! - Background management for different game states
//! - Support for various menu types (main menu, options, save/load, etc.)
//!
//! ## Architecture
//!
//! The shell system operates on a stack model where:
//! 1. Screens are pushed onto a stack when navigating forward
//! 2. Screens are popped from the stack when going back
//! 3. Each screen has proper init/shutdown lifecycle management
//! 4. Animations can be applied during transitions
//!
//! ## Usage
//! ```rust
//! use crate::gui::shell::{Shell, WindowLayout};
//!
//! let mut shell = Shell::new();
//! shell.init()?;
//!
//! // Push a new screen
//! shell.push("Menus/MainMenu.wnd", false)?;
//!
//! // Update the shell (call this every frame)
//! shell.update()?;
//!
//! // Pop current screen
//! shell.pop()?;
//! ```

use super::super::game_window::WIN_COLOR_UNDEFINED;
use super::super::game_window::{GameWindow, WindowStatus};
use super::super::ime_manager::get_ime_manager;
use super::super::window_manager::{
    WindowLayout as ManagerWindowLayout, with_window_manager, with_window_manager_ref,
};
use crate::message_stream::{GameMessageType, get_message_stream};
use crate::system::SubsystemInterface;
use game_engine::common::ini::get_global_data;
use game_engine::common::random_value::init_random_with_seed;
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::{GAME_NONE, GAME_SHELL};
use std::cell::{Cell, RefCell};
use std::cmp::min;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use thiserror::Error;

// Live `base` shell module. `include!` keeps one logical module so field
// privacy and the public API stay identical to the former monolith. Fragment
// order matches the original `base.rs` layout; `tests.rs` carries its own
// `#[cfg(test)] mod tests` wrapper.

include!("types.rs");
include!("scheme.rs");
include!("animate_window.rs");
include!("shell_lifecycle.rs");
include!("shell_ops.rs");
include!("tests.rs");
include!("residual.rs");
