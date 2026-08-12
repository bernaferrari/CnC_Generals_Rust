//! ShellSmokeResult residual claim flags (live module).
//!
//! rustc 1.96 does not allow `include!` / macros in struct field or struct-literal
//! position. Field groups live in `fields_*.rs` and are concatenated into
//! `struct_def.rs`. Assemble helpers fill `&mut ShellSmokeResult` by group.

include!("struct_def.rs");
include!("assemble.rs");
include!("fill_core.rs");
include!("fill_presentation.rs");
include!("fill_waves_72_150.rs");
include!("fill_waves_151_250.rs");
include!("fill_waves_251_350.rs");
include!("fill_waves_351_450.rs");
include!("fill_waves_451_550.rs");
include!("fill_waves_551_650.rs");
include!("fill_waves_651_750.rs");
include!("fill_waves_751_850.rs");
include!("fill_waves_851_941.rs");
include!("fill_claim.rs");
include!("format_detail.rs");
