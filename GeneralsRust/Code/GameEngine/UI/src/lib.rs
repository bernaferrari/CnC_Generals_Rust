//! Renderer-independent UI state and C++ transition policies.
//!
//! This crate knows no live simulation, renderer, GameWindow, global engine slot, or lock.
//! Backend adapters supply geometry; the owner drives updates and applies commands synchronously.
#![forbid(unsafe_code)]

mod animation;
pub use animation::{AnimateWindow, AnimateWindowManager, AnimationTarget};

/// Animation types for window transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// No animation
    None,
    /// Slide from left
    SlideLeft,
    /// Slide from right
    SlideRight,
    /// Slide from top
    SlideTop,
    /// Slide from bottom
    SlideBottom,
    /// Slide from right (fast)
    SlideRightFast,
    /// Slide from top (fast)
    SlideTopFast,
    /// Slide from bottom (timed)
    SlideBottomTimed,
    /// Spiral animation
    Spiral,
}

/// 2D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord2D {
    pub x: i32,
    pub y: i32,
}

impl Coord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// 2D float coordinate for animation velocities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2DF {
    pub x: f32,
    pub y: f32,
}

impl Coord2DF {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
