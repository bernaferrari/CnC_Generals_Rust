//! W3D Asset Loader Module
//!
//! This module provides the main asset loading interface, re-exporting
//! streaming and other loading functionality.

// The C++-style `StreamingW3dLoader` scaffolding was removed: it had no callers
// anywhere in the workspace (the game loads W3D assets synchronously through
// `AssetManager`), and its loader-thread plumbing only existed to serve it.
