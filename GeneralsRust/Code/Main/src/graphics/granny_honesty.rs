//! Granny decoder honesty for the live mesh path.
//!
//! The Device crate has a simplified `GR2R` test-blob parser
//! (`wthree_d_granny.rs`). That is **not** the RAD Granny SDK and does not
//! load retail `.gr2` files. Live units use W3D / HLOD clips selected by
//! model-condition bits and retail hide-show suffixes (`_d` / `_rd` / rubble /
//! `_die`).

/// No in-tree RAD Granny SDK / retail `.gr2` decoder.
pub const GRANNY_DECODER_AVAILABLE: bool = false;

pub fn granny_decoder_available() -> bool {
    GRANNY_DECODER_AVAILABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granny_decoder_is_documented_unavailable() {
        assert!(!granny_decoder_available());
        assert!(!GRANNY_DECODER_AVAILABLE);
    }
}
