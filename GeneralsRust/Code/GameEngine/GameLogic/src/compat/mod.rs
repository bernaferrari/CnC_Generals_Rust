pub mod legacy_state;

pub use legacy_state::{
    adapt_legacy_state, legacy_transition, register_classic_state, register_legacy_state,
    ClassicState, LegacyState, LegacyStateAdapter,
};
