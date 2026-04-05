//! WthreeDFunctionLexicon Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/Common/System/W3DFunctionLexicon.cpp
//!
//! This module provides functionality for wthree d function lexicon.

use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    ptr,
};

/// WthreeDFunctionLexicon implementation
pub struct WthreeDFunctionLexicon {
    /// Internal data
    data: Vec<u8>,
    /// State flag
    active: bool,
}

impl WthreeDFunctionLexicon {
    /// Create new instance
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            active: false,
        }
    }

    /// Process data
    pub fn process(&mut self, input: &[u8]) -> Result<Vec<u8>, WthreeDFunctionLexiconError> {
        if !self.active {
            return Err(WthreeDFunctionLexiconError::NotActive);
        }

        // PARITY_NOTE: C++ W3DFunctionLexicon.cpp is NOT a data processor.
        // It is a function pointer table (FunctionLexicon subclass) that registers
        // W3D-specific GUI draw/init callbacks (e.g., W3DGadgetPushButtonDraw,
        // W3DLeftHUDDraw, W3DMainMenuDraw, etc.) via loadTable().
        // The init() method calls FunctionLexicon::init() then loads:
        //   - gameWinDrawTable (TABLE_GAME_WIN_DEVICEDRAW)
        //   - layoutInitTable (TABLE_WIN_LAYOUT_DEVICEINIT)
        // This stub's process() API does not correspond to any C++ method.
        // Full port requires: FunctionLexicon base class, GameWindow draw callback system,
        // and all W3DGadget*Draw / W3D*HUDDraw callback implementations.
        self.data.extend_from_slice(input);
        Ok(self.data.clone())
    }

    /// Activate
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Check if active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Clear data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get data size
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

impl Default for WthreeDFunctionLexicon {
    fn default() -> Self {
        Self::new()
    }
}

/// Error types for WthreeDFunctionLexicon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WthreeDFunctionLexiconError {
    /// Not active
    NotActive,
    /// Processing failed
    ProcessingFailed,
    /// Invalid input
    InvalidInput,
    /// Unknown error
    Unknown,
}

impl std::fmt::Display for WthreeDFunctionLexiconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WthreeDFunctionLexiconError::NotActive => write!(f, "Not active"),
            WthreeDFunctionLexiconError::ProcessingFailed => write!(f, "Processing failed"),
            WthreeDFunctionLexiconError::InvalidInput => write!(f, "Invalid input"),
            WthreeDFunctionLexiconError::Unknown => write!(f, "Unknown error"),
        }
    }
}

impl std::error::Error for WthreeDFunctionLexiconError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_function_lexicon_basic() {
        let mut lex = WthreeDFunctionLexicon::new();
        assert!(!lex.is_active());
        lex.activate();
        assert!(lex.is_active());
        let result = lex.process(b"test").unwrap();
        assert_eq!(result, b"test");
        lex.deactivate();
        assert!(!lex.is_active());
    }
}
