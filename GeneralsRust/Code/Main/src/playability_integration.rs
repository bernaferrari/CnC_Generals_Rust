//! Playability diagnostics helpers used by the parity and integration gate.
//!
//! The module is intentionally deterministic and non-invasive: it consumes repository
//! tracking artifacts and returns machine-readable summaries.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PlayabilityPhase {
    BaselineLock,
    GameplayParity,
    SaveLoadTerrain,
    UiInputParity,
    PlayabilityReleaseCandidate,
}

impl PlayabilityPhase {
    pub fn from_arg(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "1" | "baseline" | "baseline-lock" | "phase1" => Some(Self::BaselineLock),
            "2" | "gameplay" | "gameplay-parity" | "phase2" => Some(Self::GameplayParity),
            "3" | "saveload" | "save-load" | "save_load" | "phase3" => Some(Self::SaveLoadTerrain),
            "4" | "ui" | "ui-input" | "ui-input-parity" | "phase4" => Some(Self::UiInputParity),
            "5" | "release" | "playable" | "playability-release" | "phase5" => {
                Some(Self::PlayabilityReleaseCandidate)
            }
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaselineLock => "phase1-baseline-lock",
            Self::GameplayParity => "phase2-gameplay-parity",
            Self::SaveLoadTerrain => "phase3-save-load-terrain",
            Self::UiInputParity => "phase4-ui-input-parity",
            Self::PlayabilityReleaseCandidate => "phase5-playability-release",
        }
    }
}

/// File kind in `PORT_FILE_MATRIX.txt` rows.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MappingKind {
    Source,
    Include,
}

impl MappingKind {
    fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix.trim() {
            "Source" => Some(Self::Source),
            "Include" => Some(Self::Include),
            _ => None,
        }
    }
}

/// Mapping status from `PORT_FILE_MATRIX.txt`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MappingStatus {
    Found,
    FoundByBasename,
    Missing,
    Unknown(String),
}

impl MappingStatus {
    fn from_value(value: &str) -> Self {
        match value.trim() {
            "FOUND" => Self::Found,
            "FOUND_BY_BASENAME" => Self::FoundByBasename,
            "MISSING" => Self::Missing,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileMappingCounts {
    pub found: u32,
    pub found_by_basename: u32,
    pub missing: u32,
    pub unknown: u32,
}

impl FileMappingCounts {
    pub fn total_entries(&self) -> u32 {
        self.found + self.found_by_basename + self.missing + self.unknown
    }

    pub fn parity_percent(&self) -> f32 {
        let total = self.total_entries();
        if total == 0 {
            0.0
        } else {
            100.0 * (self.found + self.found_by_basename) as f32 / total as f32
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubsystemMatrixSummary {
    pub subsystem: String,
    pub source: FileMappingCounts,
    pub include: FileMappingCounts,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutMismatchCounts {
    pub source: usize,
    pub include: usize,
}

impl LayoutMismatchCounts {
    pub fn total(&self) -> usize {
        self.source + self.include
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlayabilityAuditSummary {
    pub matrix_by_subsystem: Vec<SubsystemMatrixSummary>,
    pub total_missing_reports: HashMap<String, usize>,
    pub mismatch_reports_by_subsystem: HashMap<String, LayoutMismatchCounts>,
    pub source_missing_count: usize,
    pub include_missing_count: usize,
    pub network_deferred: bool,
}

#[derive(Debug, Clone)]
pub struct PlayabilityGateConfig {
    pub matrix_path: PathBuf,
    pub missing_files_path: PathBuf,
    pub mismatch_files_path: PathBuf,
    pub include_network: bool,
}

impl Default for PlayabilityGateConfig {
    fn default() -> Self {
        Self::new(
            Self::default_matrix_path(),
            Self::default_missing_files_path(),
            Self::default_mismatch_files_path(),
        )
    }
}

impl PlayabilityGateConfig {
    pub fn new(
        matrix_path: PathBuf,
        missing_files_path: PathBuf,
        mismatch_files_path: PathBuf,
    ) -> Self {
        Self {
            matrix_path,
            missing_files_path,
            mismatch_files_path,
            include_network: false,
        }
    }

    pub fn with_include_network(mut self, include_network: bool) -> Self {
        self.include_network = include_network;
        self
    }

    pub fn default_matrix_path() -> PathBuf {
        Self::discover_tracking_file("PORT_FILE_MATRIX.txt")
            .unwrap_or_else(|| PathBuf::from("PORT_FILE_MATRIX.txt"))
    }

    pub fn default_missing_files_path() -> PathBuf {
        Self::discover_tracking_file("PORT_MISSING_FILES_BY_SUBSYSTEM.txt")
            .unwrap_or_else(|| PathBuf::from("PORT_MISSING_FILES_BY_SUBSYSTEM.txt"))
    }

    pub fn default_mismatch_files_path() -> PathBuf {
        Self::discover_tracking_file("PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt")
            .unwrap_or_else(|| PathBuf::from("PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt"))
    }

    fn discover_tracking_file(file_name: &str) -> Option<PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let candidates = [
            cwd.join(file_name),
            cwd.join("..").join(file_name),
            cwd.join("..").join("..").join(file_name),
            cwd.join("..").join("..").join("..").join(file_name),
        ];
        candidates.into_iter().find(|candidate| candidate.is_file())
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl PlayabilityAuditSummary {
    pub fn total_parity_percent(&self) -> f32 {
        let mut total_entries = 0u32;
        let mut covered = 0u32;

        for subsystem in &self.matrix_by_subsystem {
            let source_total = subsystem.source.total_entries();
            let include_total = subsystem.include.total_entries();
            total_entries += source_total + include_total;
            covered += subsystem.source.found + subsystem.source.found_by_basename;
            covered += subsystem.include.found + subsystem.include.found_by_basename;
        }

        if total_entries == 0 {
            return 0.0;
        }

        100.0 * covered as f32 / total_entries as f32
    }

    pub fn unresolved_high_impact(&self) -> usize {
        let known_high_impact = self
            .total_missing_reports
            .iter()
            .filter_map(|(subsystem, count)| {
                is_high_impact_subsystem(subsystem, !self.network_deferred).then_some(*count)
            })
            .sum::<usize>();

        // Keep malformed/unsectioned rows visible while ignoring non-impact buckets (e.g. Precompiled).
        let known_all = self.total_missing_reports.values().sum::<usize>();
        let unsectioned = self.source_missing_count.saturating_sub(known_all);
        known_high_impact + unsectioned
    }

    pub fn total_unresolved_including_headers(&self) -> usize {
        self.unresolved_high_impact() + self.include_missing_count
    }

    pub fn unresolved_layout_mismatches(&self) -> usize {
        self.mismatch_reports_by_subsystem
            .iter()
            .filter(|(subsystem, _)| (**subsystem != "GameNetwork") || !self.network_deferred)
            .map(|(_, counts)| counts.source)
            .sum()
    }

    pub fn informational_include_layout_mismatches(&self) -> usize {
        self.mismatch_reports_by_subsystem
            .iter()
            .filter(|(subsystem, _)| (**subsystem != "GameNetwork") || !self.network_deferred)
            .map(|(_, counts)| counts.include)
            .sum()
    }

    pub fn high_impact_sections(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .total_missing_reports
            .iter()
            .filter_map(|(section, count)| (count > &0).then(|| section.clone()))
            .filter(|section| is_high_impact_subsystem(section, !self.network_deferred))
            .collect();

        for (section, count) in &self.mismatch_reports_by_subsystem {
            if count.total() > 0 && is_high_impact_subsystem(section, !self.network_deferred) {
                keys.push(section.clone());
            }
        }

        keys.sort();
        keys.dedup();
        keys
    }

    pub fn pass_gate(&self, phase: PlayabilityPhase) -> bool {
        let target = phase_threshold(phase);
        let unresolved = if target.strict_missing_headers {
            self.total_unresolved_including_headers()
        } else {
            self.unresolved_high_impact()
        };
        unresolved <= target.max_unresolved_blockers
            && self.unresolved_layout_mismatches() <= target.max_layout_mismatches
            && self.total_parity_percent() >= target.min_parity_percent
    }

    pub fn gate_description(&self, phase: PlayabilityPhase) -> String {
        let target = phase_threshold(phase);
        format!(
            "{}: parity {:.1}% >= {:.1}%; unresolved blockers <= {} ({} phase scope); layout mismatches <= {}",
            phase.as_str(),
            self.total_parity_percent(),
            target.min_parity_percent,
            target.max_unresolved_blockers,
            if target.strict_missing_headers { "strict" } else { "high-impact-only" },
            target.max_layout_mismatches
        )
    }

    pub fn has_unresolved_blockers(&self) -> bool {
        self.unresolved_high_impact() > 0
    }

    pub fn has_unresolved_blockers_with_headers(&self) -> bool {
        self.total_unresolved_including_headers() > 0
    }
}

fn is_high_impact_subsystem(name: &str, include_network: bool) -> bool {
    matches!(name, "GameLogic" | "GameClient" | "Common")
        || (include_network && name == "GameNetwork")
}

#[derive(Debug, Clone, Copy)]
struct PhaseGateConfig {
    min_parity_percent: f32,
    max_unresolved_blockers: usize,
    max_layout_mismatches: usize,
    strict_missing_headers: bool,
}

fn phase_threshold(phase: PlayabilityPhase) -> PhaseGateConfig {
    match phase {
        PlayabilityPhase::BaselineLock => PhaseGateConfig {
            min_parity_percent: 0.0,
            max_unresolved_blockers: 0,
            max_layout_mismatches: 700,
            strict_missing_headers: false,
        },
        PlayabilityPhase::GameplayParity => PhaseGateConfig {
            min_parity_percent: 0.0,
            max_unresolved_blockers: 12,
            max_layout_mismatches: 600,
            strict_missing_headers: false,
        },
        PlayabilityPhase::SaveLoadTerrain => PhaseGateConfig {
            min_parity_percent: 0.0,
            max_unresolved_blockers: 6,
            max_layout_mismatches: 420,
            strict_missing_headers: false,
        },
        PlayabilityPhase::UiInputParity => PhaseGateConfig {
            min_parity_percent: 0.0,
            max_unresolved_blockers: 2,
            max_layout_mismatches: 250,
            strict_missing_headers: true,
        },
        PlayabilityPhase::PlayabilityReleaseCandidate => PhaseGateConfig {
            min_parity_percent: 0.0,
            max_unresolved_blockers: 0,
            max_layout_mismatches: 0,
            strict_missing_headers: true,
        },
    }
}

impl fmt::Display for PlayabilityAuditSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== File mapping matrix summary ===")?;
        for subsystem in &self.matrix_by_subsystem {
            writeln!(
                f,
                "{}: source={} total (found: {:>3}, by_name: {:>3}, missing: {:>3}), include={} total (found: {:>3}, by_name: {:>3}, missing: {:>3})",
                subsystem.subsystem,
                subsystem.source.total_entries(),
                subsystem.source.found,
                subsystem.source.found_by_basename,
                subsystem.source.missing,
                subsystem.include.total_entries(),
                subsystem.include.found,
                subsystem.include.found_by_basename,
                subsystem.include.missing,
            )?;
            writeln!(
                f,
                "    parity: source {:.1}% include {:.1}%",
                subsystem.source.parity_percent(),
                subsystem.include.parity_percent(),
            )?;
        }

        writeln!(f, "Source missing files: {}", self.source_missing_count)?;
        writeln!(f, "Include missing files: {}", self.include_missing_count)?;
        let source_mismatch_total = self.unresolved_layout_mismatches();
        let include_mismatch_total = self.informational_include_layout_mismatches();
        if source_mismatch_total > 0 || include_mismatch_total > 0 {
            writeln!(f, "Path-layout mismatch entries: {}", source_mismatch_total)?;
            if include_mismatch_total > 0 {
                writeln!(
                    f,
                    "Header-layout mismatch entries (informational): {}",
                    include_mismatch_total
                )?;
            }
            for (subsystem, counts) in self.mismatch_reports_by_subsystem.iter() {
                if counts.source > 0 {
                    writeln!(f, "  {}: {}", subsystem, counts.source)?;
                }
            }
        }
        writeln!(
            f,
            "High-impact unresolved blockers: {}",
            self.unresolved_high_impact()
        )?;
        writeln!(
            f,
            "Total matrix parity: {:.1}%",
            self.total_parity_percent()
        )
    }
}

pub fn current_unresolved_blocker_examples(
    summary: &PlayabilityAuditSummary,
    max_entries: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    for (section, count) in summary.total_missing_reports.iter() {
        if lines.len() >= max_entries {
            break;
        }

        if *count > 0 {
            lines.push(format!(
                "{}: {} missing path-layout entries",
                section, count
            ));
        }
    }

    if lines.len() < max_entries {
        if summary.source_missing_count > 0 {
            lines.push(format!(
                "Source list has {} unmapped path entries",
                summary.source_missing_count
            ));
        }
        if summary.include_missing_count > 0 {
            lines.push(format!(
                "Include list has {} unmapped header entries",
                summary.include_missing_count
            ));
        }
    }

    if summary.unresolved_layout_mismatches() > 0 && lines.len() < max_entries {
        lines.push(format!(
            "Layout mismatch entries: {}",
            summary.unresolved_layout_mismatches()
        ));
    }

    lines.into_iter().take(max_entries).collect()
}

fn parse_matrix_row(line: &str) -> Option<(String, MappingKind, MappingStatus)> {
    let parts = line.splitn(5, " | ").map(str::trim).collect::<Vec<_>>();
    if parts.len() < 4 {
        return None;
    }

    let kind = MappingKind::from_prefix(parts[0])?;
    let path = parts[1]
        .trim_start_matches("Source/")
        .trim_start_matches("Include/");
    let status = MappingStatus::from_value(parts[3]);
    let subsystem = path.split('/').next().unwrap_or("unknown").to_string();
    Some((subsystem, kind, status))
}

fn parse_mismatch_row(line: &str) -> Option<(String, MappingKind)> {
    let parts = line.split(" | ").map(str::trim).collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }

    let kind = MappingKind::from_prefix(parts[0])?;
    let expected_field = parts[2].trim_start_matches("expected ");
    let mut expected_parts = expected_field.split('/').map(str::trim);
    let expected_root = expected_parts.next()?;
    if expected_root.is_empty() {
        return None;
    }
    Some((expected_root.to_string(), kind))
}

fn is_tracking_section_header(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']')
}

fn increment_counts(
    target: &mut HashMap<String, (FileMappingCounts, FileMappingCounts)>,
    row: (String, MappingKind, MappingStatus),
) {
    let (subsystem, kind, status) = row;
    let entry = target
        .entry(subsystem)
        .or_insert_with(|| (FileMappingCounts::default(), FileMappingCounts::default()));

    let counts = match kind {
        MappingKind::Source => &mut entry.0,
        MappingKind::Include => &mut entry.1,
    };

    match status {
        MappingStatus::Found => counts.found += 1,
        MappingStatus::FoundByBasename => counts.found_by_basename += 1,
        MappingStatus::Missing => counts.missing += 1,
        MappingStatus::Unknown(_) => counts.unknown += 1,
    }
}

/// Build a full parity report from PORT tracking files.
pub fn build_playability_audit(
    config: &PlayabilityGateConfig,
) -> Result<PlayabilityAuditSummary, ParseError> {
    let matrix_data = fs::read_to_string(&config.matrix_path).map_err(|err| ParseError {
        message: format!("failed to read {}: {err}", config.matrix_path.display()),
    })?;

    let missing_data =
        fs::read_to_string(&config.missing_files_path).map_err(|err| ParseError {
            message: format!(
                "failed to read {}: {err}",
                config.missing_files_path.display()
            ),
        })?;

    let mut map = HashMap::<String, (FileMappingCounts, FileMappingCounts)>::new();
    for line in matrix_data.lines() {
        if let Some(row) = parse_matrix_row(line) {
            if !config.include_network && row.0 == "GameNetwork" {
                continue;
            }
            increment_counts(&mut map, row);
        }
    }

    let mut by_subsystem = map
        .into_iter()
        .map(|(subsystem, (source, include))| SubsystemMatrixSummary {
            subsystem,
            source,
            include,
        })
        .collect::<Vec<_>>();
    by_subsystem.sort_by(|a, b| a.subsystem.cmp(&b.subsystem));

    let mut total_missing_reports = HashMap::<String, usize>::new();
    let mut source_missing_count = 0usize;
    let mut include_missing_count = 0usize;
    let mut mismatch_reports_by_subsystem = HashMap::<String, LayoutMismatchCounts>::new();
    let mut current_section = String::new();
    let mut in_source_missing_section = false;
    let mut in_include_missing_section = false;

    for line in missing_data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_tracking_section_header(trimmed) {
            current_section = trimmed.trim_matches(&['[', ']'][..]).to_string();
            in_source_missing_section = false;
            in_include_missing_section = false;
            continue;
        }
        if trimmed.starts_with("Source missing:") {
            in_source_missing_section = true;
            in_include_missing_section = false;
            continue;
        }
        if trimmed.starts_with("Include missing") {
            in_include_missing_section = true;
            in_source_missing_section = false;
            continue;
        }

        let is_source_entry = in_source_missing_section && trimmed.starts_with("Source/");
        let is_include_entry = in_include_missing_section && trimmed.starts_with("Include/");
        if !is_source_entry && !is_include_entry {
            continue;
        }
        let track_section_for_blockers = current_section.is_empty()
            || is_high_impact_subsystem(&current_section, config.include_network);
        if !track_section_for_blockers {
            continue;
        }

        if is_source_entry {
            source_missing_count += 1;
            if !current_section.is_empty() {
                *total_missing_reports
                    .entry(current_section.clone())
                    .or_insert(0) += 1;
            }
        }
        if is_include_entry {
            include_missing_count += 1;
        }
    }

    if let Ok(mismatch_data) = fs::read_to_string(&config.mismatch_files_path) {
        let mut section = String::new();
        for line in mismatch_data.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if is_tracking_section_header(trimmed) {
                section = trimmed.trim_matches(&['[', ']'][..]).to_string();
                continue;
            }
            if section.is_empty() {
                continue;
            }
            if !config.include_network && section == "GameNetwork" {
                continue;
            }
            if let Some((_, kind)) = parse_mismatch_row(trimmed) {
                let counts = mismatch_reports_by_subsystem
                    .entry(section.clone())
                    .or_default();
                match kind {
                    MappingKind::Source => counts.source += 1,
                    MappingKind::Include => counts.include += 1,
                }
            }
        }
    }

    Ok(PlayabilityAuditSummary {
        matrix_by_subsystem: by_subsystem,
        total_missing_reports,
        mismatch_reports_by_subsystem,
        source_missing_count,
        include_missing_count,
        network_deferred: !config.include_network,
    })
}

/// Convenience check for phase-1 gating: no source/section blockers remain.
pub fn has_unresolved_blockers(summary: &PlayabilityAuditSummary) -> bool {
    summary.unresolved_high_impact() > 0
}

/// Convenience check that also includes header list mismatches.
pub fn has_unresolved_blockers_with_headers(summary: &PlayabilityAuditSummary) -> bool {
    summary.total_unresolved_including_headers() > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SAMPLE_CONFIG_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn sample_config(
        matrix: &str,
        missing: &str,
        mismatch: &str,
    ) -> Result<PlayabilityAuditSummary, ParseError> {
        let temp_dir = std::env::temp_dir();
        let unique = SAMPLE_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let matrix_path = temp_dir.join(format!("playability_matrix_sample_{pid}_{unique}.txt"));
        let missing_path = temp_dir.join(format!("playability_missing_sample_{pid}_{unique}.txt"));
        let mismatch_path =
            temp_dir.join(format!("playability_mismatch_sample_{pid}_{unique}.txt"));
        fs::write(&mismatch_path, mismatch).map_err(|err| ParseError {
            message: format!("write mismatch: {err}"),
        })?;

        fs::write(&matrix_path, matrix).map_err(|err| ParseError {
            message: format!("write matrix: {err}"),
        })?;
        fs::write(&missing_path, missing).map_err(|err| ParseError {
            message: format!("write missing: {err}"),
        })?;

        let config = PlayabilityGateConfig::new(matrix_path, missing_path, mismatch_path);
        build_playability_audit(&config)
    }

    #[test]
    fn test_parse_playability_matrix() {
        let matrix = "Source | Source/GameLogic/Foo.cpp | GameLogic/GameLogic/src/foo.rs | FOUND\nSource | Source/GameClient/Foo.cpp | GameClient/src/bar.rs | FOUND_BY_BASENAME\nInclude | Include/GameLogic/Foo.h | game_logic.rs | MISSING\n";
        let missing = "[Common]\nSource missing:\nSource/Common/Unused.cpp\n[GameLogic]\nInclude missing (no Rust header equivalents):\nInclude/GameLogic/Legacy.h\n";

        let summary = sample_config(matrix, missing, "").expect("audit parse");
        assert_eq!(summary.matrix_by_subsystem.len(), 2);
        assert_eq!(summary.source_missing_count, 1);
        assert_eq!(summary.include_missing_count, 1);
        assert!(summary.unresolved_high_impact() > 0);
        assert!((summary.total_parity_percent() - 66.66667).abs() < 0.1);
    }

    #[test]
    fn test_has_no_unresolved_blockers_is_boolean() {
        let matrix = "Source | Source/GameLogic/Foo.cpp | GameLogic/GameLogic/src/foo.rs | FOUND\nSource | Source/GameNetwork/Bar.cpp | GameNetwork/src/bar.rs | MISSING | GameNetwork/GameNetwork/src/net/bar.rs\n";
        let summary = sample_config(matrix, "[GameLogic]\n", "").expect("audit parse");
        assert!(!has_unresolved_blockers(&summary));
    }

    #[test]
    fn test_playability_phase_gate_tracks_mismatch_inputs() {
        let matrix = "Source | Source/GameLogic/Foo.cpp | GameLogic/GameLogic/src/foo.rs | FOUND\n";
        let missing = "[GameClient]\n";
        let mismatch = "[GameLogic]\nInclude | Include/GameLogic/Foo.h | expected game_logic/foo.rs | found Foo.rs\n";

        let summary = sample_config(matrix, missing, mismatch).expect("audit parse");
        assert_eq!(summary.mismatch_reports_by_subsystem.len(), 1);
        assert_eq!(summary.informational_include_layout_mismatches(), 1);
        assert!(summary.pass_gate(PlayabilityPhase::BaselineLock));
    }

    #[test]
    fn test_layout_mismatch_separates_from_unresolved_blockers() {
        let matrix = "Source | Source/GameLogic/Foo.cpp | GameLogic/GameLogic/src/foo.rs | FOUND\n";
        let missing = "[GameLogic]\n";
        let mismatch = "[GameLogic]\nInclude | Include/GameLogic/Foo.h | expected game_logic/foo.rs | found Foo.rs\n";

        let summary = sample_config(matrix, missing, mismatch).expect("audit parse");
        assert_eq!(summary.unresolved_high_impact(), 0);
        assert_eq!(summary.unresolved_layout_mismatches(), 0);
        assert_eq!(summary.informational_include_layout_mismatches(), 1);
        assert_eq!(summary.total_unresolved_including_headers(), 0);
        assert!(summary.pass_gate(PlayabilityPhase::BaselineLock));
    }

    #[test]
    fn test_network_is_deferred_by_default() {
        let matrix = "Source | Source/GameNetwork/Foo.cpp | GameNetwork/src/foo.rs | MISSING | GameNetwork/GameNetwork/src/foo.rs\n";
        let missing = "[GameNetwork]\nSource missing:\nSource/GameNetwork/Foo.cpp\nInclude missing (no Rust header equivalents):\nInclude/GameNetwork/Foo.h\n";
        let mismatch = "[GameNetwork]\nSource | Source/GameNetwork/Foo.cpp | expected game_network/foo.rs | found GameNetwork/GameNetwork/src/foo.rs\n";
        let summary = sample_config(matrix, missing, mismatch).expect("audit parse");
        assert_eq!(summary.matrix_by_subsystem.len(), 0);
        assert_eq!(summary.source_missing_count, 0);
        assert_eq!(summary.include_missing_count, 0);
        assert_eq!(summary.unresolved_layout_mismatches(), 0);
        assert_eq!(summary.unresolved_high_impact(), 0);
    }

    #[test]
    fn test_network_can_be_included_explicitly() {
        let temp_dir = std::env::temp_dir();
        let matrix_path = temp_dir.join("playability_matrix_network_include.txt");
        let missing_path = temp_dir.join("playability_missing_network_include.txt");
        let mismatch_path = temp_dir.join("playability_mismatch_network_include.txt");

        fs::write(
            &matrix_path,
            "Source | Source/GameNetwork/Foo.cpp | GameNetwork/src/foo.rs | MISSING | GameNetwork/GameNetwork/src/foo.rs\n",
        )
        .expect("write matrix");
        fs::write(
            &missing_path,
            "[GameNetwork]\nSource missing:\nSource/GameNetwork/Foo.cpp\nInclude missing (no Rust header equivalents):\nInclude/GameNetwork/Foo.h\n",
        )
        .expect("write missing");
        fs::write(
            &mismatch_path,
            "[GameNetwork]\nInclude | Include/GameNetwork/Foo.h | expected game_network/foo.rs | found GameNetwork/GameNetwork/src/foo.rs\n",
        )
        .expect("write mismatch");

        let config = PlayabilityGateConfig::new(matrix_path, missing_path, mismatch_path)
            .with_include_network(true);
        let summary = build_playability_audit(&config).expect("audit parse");
        assert_eq!(summary.matrix_by_subsystem.len(), 1);
        assert_eq!(summary.source_missing_count, 1);
        assert_eq!(summary.include_missing_count, 1);
        assert_eq!(summary.unresolved_layout_mismatches(), 0);
        assert_eq!(summary.informational_include_layout_mismatches(), 1);
        assert_eq!(summary.unresolved_high_impact(), 1);
    }

    #[test]
    fn test_precompiled_missing_entries_are_informational_only() {
        let matrix = "Source | Source/Precompiled/PreRTS.cpp | Precompiled/pre_rts.rs | MISSING\nInclude | Include/Precompiled/PreRTS.h | precompiled/pre_rts.rs | MISSING\n";
        let missing = "[Precompiled]\nSource missing:\nSource/Precompiled/PreRTS.cpp\nInclude missing (no Rust header equivalents):\nInclude/Precompiled/PreRTS.h\n";

        let summary = sample_config(matrix, missing, "").expect("audit parse");
        assert_eq!(summary.source_missing_count, 0);
        assert_eq!(summary.include_missing_count, 0);
        assert_eq!(summary.unresolved_high_impact(), 0);
        assert_eq!(summary.total_unresolved_including_headers(), 0);
        assert!(summary.pass_gate(PlayabilityPhase::PlayabilityReleaseCandidate));
    }
}
