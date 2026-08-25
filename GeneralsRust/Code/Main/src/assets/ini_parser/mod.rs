////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! INI object-definition parsing, split by source-owned format domains.
//!
//! The sibling modules retain the original declaration and parse order:
//! authored draw/value records live in `ini_parser_types`, object/domain
//! behavior in `ini_parser_objects`, and the stateful reader in
//! `ini_parser_parser`. Re-exports keep the pre-split public API stable.

use anyhow::Result;
use log::{debug, trace};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

mod objects;
mod parser;
mod types;

#[cfg(test)]
mod tests;

pub use objects::*;
pub use parser::*;
pub use types::*;
