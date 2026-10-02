//! Texture format decision logging for validation and diagnostics.
//!
//! The original DirectX renderer exposed rich diagnostic output around
//! `Get_Valid_Texture_Format` so tools could assert the runtime choices.
//! This module mirrors that behaviour for the WGPU port by recording the
//! format decisions taken whenever a texture is ingested.

use crate::core::ww3dformat::{FormatDecision, WW3DFormat};
use std::cell::RefCell;

/// Recorded texture format decision.
#[cfg_attr(
    feature = "serde_support",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct TextureDecisionRecord {
    pub name: String,
    pub source_format: WW3DFormat,
    pub preferred_format: WW3DFormat,
    pub resolved_format: WW3DFormat,
    pub requires_decompression: bool,
    pub mip_levels: u32,
}

thread_local! {
    /// C++ recorded texture decisions in a plain static buffer on the game thread.
    static DECISIONS: RefCell<Vec<TextureDecisionRecord>> = const { RefCell::new(Vec::new()) };
}

/// Record a texture format decision for later analysis.
pub fn record_decision(name: impl Into<String>, decision: &FormatDecision, mip_levels: u32) {
    let record = TextureDecisionRecord {
        name: name.into(),
        source_format: decision.source_format,
        preferred_format: decision.preferred_format,
        resolved_format: decision.format,
        requires_decompression: decision.requires_decompression,
        mip_levels,
    };

    DECISIONS.with_borrow_mut(|guard| guard.push(record));
}

/// Snapshot the recorded decisions and clear the buffer.
pub fn drain_decisions() -> Vec<TextureDecisionRecord> {
    DECISIONS.with_borrow_mut(|guard| std::mem::take(guard))
}

/// Borrow a copy of the current decision log without clearing it.
pub fn snapshot_decisions() -> Vec<TextureDecisionRecord> {
    DECISIONS.with_borrow(Vec::clone)
}

/// Summary information derived from recorded decisions.
#[cfg_attr(
    feature = "serde_support",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Default)]
pub struct TextureMetricsSummary {
    pub total_textures: usize,
    pub decompressed_textures: usize,
    pub compressed_requests: usize,
    pub average_mip_levels: f32,
}

/// Build a summary of the current decision log without clearing it.
pub fn summarize() -> TextureMetricsSummary {
    let empty = Vec::new();
    let guard = &DECISIONS.with_borrow(Vec::clone);
    let guard: &Vec<TextureDecisionRecord> = if guard.is_empty() { &empty } else { guard };

    if guard.is_empty() {
        return TextureMetricsSummary::default();
    }

    let mut summary = TextureMetricsSummary::default();
    let mut mip_total = 0u64;

    for record in guard.iter() {
        summary.total_textures += 1;
        mip_total += record.mip_levels as u64;
        if record.requires_decompression {
            summary.decompressed_textures += 1;
        }
        if record.source_format.is_block_compressed() {
            summary.compressed_requests += 1;
        }
    }

    summary.average_mip_levels = (mip_total as f32) / (summary.total_textures.max(1) as f32);
    summary
}
