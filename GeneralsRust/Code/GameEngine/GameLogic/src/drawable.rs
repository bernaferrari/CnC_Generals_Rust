//! Drawable system - GameLogic side drawable interface
//!
//! This is the GameLogic-side interface for drawable objects. The actual rendering
//! implementation is in GameClient. This trait provides the interface that GameLogic
//! objects use to interact with their visual representation.
//!
//! Reference: C++ Drawable.h and Drawable.cpp in GameClient
//!
//! Architecture:
//! - GameLogic/Object owns a reference to its Drawable (via ObjectID)
//! - GameClient/Drawable contains the actual rendering data and modules
//! - This trait bridges the two, allowing GameLogic to control visibility, state, etc.

use crate::common::*;

/// Drawable trait for objects that can be rendered
/// This is the minimal interface that GameLogic code uses to interact with drawables
pub trait Drawable {
    /// Render the drawable at a specific position and rotation
    /// Note: This is typically called by the GameClient rendering system
    fn draw(&mut self, transform: Option<&Matrix3D>);

    /// Check if the drawable is currently visible
    fn is_visible(&self) -> bool;

    /// Set the drawable visibility state
    /// When hidden, the drawable will not be rendered or updated
    fn set_visible(&mut self, visible: bool);

    /// Get current world transform
    fn get_transform(&self) -> Matrix3D;
}
