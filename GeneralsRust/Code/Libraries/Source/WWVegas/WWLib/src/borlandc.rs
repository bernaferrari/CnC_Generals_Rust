//! Borland C++ compiler compatibility shim mirroring WWLib `borlandc.h`.
//!
//! The original C++ header contains Borland C++ specific overrides to match
//! C++ standards. However, as noted in the original header:
//!
//! > "Funny, but there are no required overrides to make Borland C match C++
//! > standards. This is because Borland C more closely matches the C++ standard
//! > than the other compilers."
//!
//! In Rust, no Borland-specific compatibility is needed. This module exists
//! solely for source-level parity with the C++ codebase.
//!
//! # C++ Source
//! Original implementation in `GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/borlandc.h`

#[cfg(test)]
mod tests {
    #[test]
    fn test_borlandc_module_exists() {
        // Module exists for parity; no runtime behavior required
        assert!(true);
    }
}
