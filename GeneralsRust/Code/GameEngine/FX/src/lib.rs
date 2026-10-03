//! Authored FX rules shared by headless simulation and client execution adapters.
//! C++ authority: GameClient/FXList.cpp and Common/INI/INI.cpp.
//! No runtime state, renderer, audio, RNG, synchronization, or engine dependency.
#![forbid(unsafe_code)]
mod definition;
mod fields;
mod parser;
pub use definition::*;
pub use parser::{FxValueParser, parse_fx_nugget_definition};
mod catalog;
pub use catalog::FxCatalog;
#[cfg(test)]
mod tests;
