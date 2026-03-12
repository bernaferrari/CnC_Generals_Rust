use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ww3d_renderer_3d::rendering::texture_metrics::{
    self, TextureDecisionRecord, TextureMetricsSummary,
};

#[derive(Debug, Error)]
pub enum TextureAnalysisError {
    #[error("failed to read log: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse log: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureDecisionLog {
    pub summary: TextureMetricsSummary,
    pub decisions: Vec<TextureDecisionRecord>,
}

impl TextureDecisionLog {
    pub fn current_snapshot() -> Self {
        Self {
            summary: texture_metrics::summarize(),
            decisions: texture_metrics::snapshot_decisions(),
        }
    }
}

pub fn load_log<P: AsRef<Path>>(path: P) -> Result<TextureDecisionLog, TextureAnalysisError> {
    let data = fs::read(path)?;
    let log = serde_json::from_slice::<TextureDecisionLog>(&data)?;
    Ok(log)
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TextureDecisionDiff {
    pub missing_in_rhs: Vec<TextureDecisionRecord>,
    pub additional_in_rhs: Vec<TextureDecisionRecord>,
    pub decompression_delta: i32,
}

pub fn diff_logs(lhs: &TextureDecisionLog, rhs: &TextureDecisionLog) -> TextureDecisionDiff {
    let mut diff = TextureDecisionDiff::default();

    let lhs_set: HashMap<_, _> = lhs
        .decisions
        .iter()
        .map(|d| ((d.name.clone(), d.resolved_format), d))
        .collect();
    let rhs_set: HashMap<_, _> = rhs
        .decisions
        .iter()
        .map(|d| ((d.name.clone(), d.resolved_format), d))
        .collect();

    for (key, record) in lhs_set.iter() {
        if !rhs_set.contains_key(key) {
            diff.missing_in_rhs.push((*record).clone());
        }
    }

    for (key, record) in rhs_set.iter() {
        if !lhs_set.contains_key(key) {
            diff.additional_in_rhs.push((*record).clone());
        }
    }

    diff.decompression_delta =
        rhs.summary.decompressed_textures as i32 - lhs.summary.decompressed_textures as i32;

    diff
}

pub fn print_summary(log: &TextureDecisionLog) {
    println!("Texture Decisions Summary:");
    println!("  Total textures: {}", log.summary.total_textures);
    println!(
        "  Decompressed (CPU fallback): {}",
        log.summary.decompressed_textures
    );
    println!("  Compressed requests: {}", log.summary.compressed_requests);
    println!(
        "  Average mip levels: {:.2}",
        log.summary.average_mip_levels
    );

    if !log.decisions.is_empty() {
        let mut formats: BTreeSet<String> = BTreeSet::new();
        for record in &log.decisions {
            formats.insert(format!("{:?}", record.resolved_format));
        }
        println!("  Formats observed: {:?}", formats);
    }
}

pub fn print_diff(diff: &TextureDecisionDiff) {
    println!("Texture Decision Diff:");
    println!("  Δ Decompressed textures: {}", diff.decompression_delta);
    if !diff.missing_in_rhs.is_empty() {
        println!("  Missing in RHS ({} entries):", diff.missing_in_rhs.len());
        for record in &diff.missing_in_rhs {
            println!(
                "    - {} -> {:?} (pref {:?})",
                record.name, record.resolved_format, record.preferred_format
            );
        }
    }
    if !diff.additional_in_rhs.is_empty() {
        println!(
            "  Additional in RHS ({} entries):",
            diff.additional_in_rhs.len()
        );
        for record in &diff.additional_in_rhs {
            println!(
                "    + {} -> {:?} (pref {:?})",
                record.name, record.resolved_format, record.preferred_format
            );
        }
    }
}
