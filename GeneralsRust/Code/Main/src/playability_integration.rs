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
    /// Rows marked FOUND/FOUND_BY_BASENAME whose mapped destination path does not exist.
    pub stale_mapped: u32,
}

impl FileMappingCounts {
    pub fn total_entries(&self) -> u32 {
        self.found
            + self.found_by_basename
            + self.missing
            + self.unknown
            + self.stale_mapped
    }

    /// Covered entries exclude stale mapped paths (claimed found but file missing).
    pub fn covered_entries(&self) -> u32 {
        self.found + self.found_by_basename
    }

    pub fn parity_percent(&self) -> f32 {
        let total = self.total_entries();
        if total == 0 {
            0.0
        } else {
            100.0 * self.covered_entries() as f32 / total as f32
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
    pub input_warnings: Vec<String>,
    /// Mapped destinations marked FOUND* that do not exist on disk.
    pub stale_mapped_paths: Vec<String>,
    pub verify_mapped_paths_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PlayabilityGateConfig {
    pub matrix_path: PathBuf,
    pub missing_files_path: PathBuf,
    pub mismatch_files_path: PathBuf,
    pub include_network: bool,
    /// When true (default), FOUND rows whose mapped path is missing count as stale blockers.
    pub verify_mapped_paths: bool,
    /// Root under which matrix mapped paths are resolved (typically GeneralsRust/Code/GameEngine).
    pub rust_engine_root: Option<PathBuf>,
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
            verify_mapped_paths: true,
            rust_engine_root: Self::discover_rust_engine_root(),
        }
    }

    pub fn with_include_network(mut self, include_network: bool) -> Self {
        self.include_network = include_network;
        self
    }

    pub fn with_verify_mapped_paths(mut self, verify: bool) -> Self {
        self.verify_mapped_paths = verify;
        self
    }

    pub fn with_rust_engine_root(mut self, root: Option<PathBuf>) -> Self {
        self.rust_engine_root = root;
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

    pub fn discover_rust_engine_root() -> Option<PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let candidates = [
            cwd.join("GeneralsRust").join("Code").join("GameEngine"),
            cwd.join("Code").join("GameEngine"),
            cwd.join("..").join("Code").join("GameEngine"),
            cwd.join("..")
                .join("GeneralsRust")
                .join("Code")
                .join("GameEngine"),
            cwd.join("..")
                .join("..")
                .join("GeneralsRust")
                .join("Code")
                .join("GameEngine"),
            cwd.join("..")
                .join("..")
                .join("..")
                .join("GeneralsRust")
                .join("Code")
                .join("GameEngine"),
        ];
        candidates.into_iter().find(|candidate| candidate.is_dir())
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
            covered += subsystem.source.covered_entries();
            covered += subsystem.include.covered_entries();
        }

        if total_entries == 0 {
            return 0.0;
        }

        100.0 * covered as f32 / total_entries as f32
    }

    pub fn stale_mapped_path_count(&self) -> usize {
        self.stale_mapped_paths.len()
    }

    /// Count of matrix rows marked MISSING or UNKNOWN (not covered, not stale-found).
    ///
    /// These are independent of the PORT_MISSING_FILES report: a matrix can claim
    /// incomplete parity without listing Source-missing rows.
    pub fn matrix_missing_entry_count(&self) -> usize {
        self.matrix_by_subsystem
            .iter()
            .map(|subsystem| {
                (subsystem.source.missing
                    + subsystem.include.missing
                    + subsystem.source.unknown
                    + subsystem.include.unknown) as usize
            })
            .sum()
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
        // Stale FOUND mappings are always blockers: file presence without a real destination
        // is not playability evidence.
        // Matrix MISSING rows lower total_parity and are enforced via phase/strict
        // min_parity thresholds (not double-counted here as missing-file blockers).
        known_high_impact + unsectioned + self.stale_mapped_path_count()
    }

    /// Strict gate used by `playability_audit --strict` (no --phase).
    ///
    /// Applies release-candidate thresholds: non-zero min parity, zero unresolved
    /// blockers (including matrix MISSING + stale paths), complete inputs.
    pub fn pass_strict_gate(&self) -> bool {
        self.pass_gate(PlayabilityPhase::PlayabilityReleaseCandidate)
    }

    pub fn strict_gate_description(&self) -> String {
        format!(
            "strict: {}",
            self.gate_description(PlayabilityPhase::PlayabilityReleaseCandidate)
        )
    }

    /// Evaluate the shipped strict audit path; returns Err with a human reason on failure.
    pub fn evaluate_strict_gate(&self) -> Result<(), String> {
        if self.has_input_warnings() {
            return Err("audit inputs are incomplete".to_string());
        }
        if self.stale_mapped_path_count() > 0 {
            return Err(format!(
                "{} stale mapped path(s) (FOUND but missing on disk)",
                self.stale_mapped_path_count()
            ));
        }
        if !self.pass_strict_gate() {
            return Err(self.strict_gate_description());
        }
        Ok(())
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
        if target.require_complete_inputs && self.has_input_warnings() {
            return false;
        }
        // Path verification is never optional for gate truthfulness: stale FOUND rows always fail.
        if self.verify_mapped_paths_enabled && self.stale_mapped_path_count() > 0 {
            return false;
        }
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
            "{}: parity {:.1}% >= {:.1}%; unresolved blockers <= {} ({} phase scope); layout mismatches <= {}; stale mapped paths = {}{}",
            phase.as_str(),
            self.total_parity_percent(),
            target.min_parity_percent,
            target.max_unresolved_blockers,
            if target.strict_missing_headers {
                "strict"
            } else {
                "high-impact-only"
            },
            target.max_layout_mismatches,
            self.stale_mapped_path_count(),
            if target.require_complete_inputs {
                "; input tracking must be complete"
            } else {
                ""
            }
        )
    }

    pub fn has_unresolved_blockers(&self) -> bool {
        self.unresolved_high_impact() > 0
    }

    pub fn has_unresolved_blockers_with_headers(&self) -> bool {
        self.total_unresolved_including_headers() > 0
    }

    pub fn has_input_warnings(&self) -> bool {
        !self.input_warnings.is_empty()
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
    require_complete_inputs: bool,
}

fn phase_threshold(phase: PlayabilityPhase) -> PhaseGateConfig {
    // Thresholds are intentionally non-zero where possible so a matrix that only
    // counts "files present" cannot pass with min_parity_percent = 0.0.
    // Stale mapped paths fail independently of these thresholds.
    match phase {
        PlayabilityPhase::BaselineLock => PhaseGateConfig {
            min_parity_percent: 50.0,
            max_unresolved_blockers: 0,
            max_layout_mismatches: 700,
            strict_missing_headers: false,
            require_complete_inputs: false,
        },
        PlayabilityPhase::GameplayParity => PhaseGateConfig {
            min_parity_percent: 80.0,
            max_unresolved_blockers: 12,
            max_layout_mismatches: 600,
            strict_missing_headers: false,
            require_complete_inputs: false,
        },
        PlayabilityPhase::SaveLoadTerrain => PhaseGateConfig {
            min_parity_percent: 85.0,
            max_unresolved_blockers: 6,
            max_layout_mismatches: 420,
            strict_missing_headers: false,
            require_complete_inputs: false,
        },
        PlayabilityPhase::UiInputParity => PhaseGateConfig {
            min_parity_percent: 90.0,
            max_unresolved_blockers: 2,
            max_layout_mismatches: 250,
            strict_missing_headers: true,
            require_complete_inputs: true,
        },
        PlayabilityPhase::PlayabilityReleaseCandidate => PhaseGateConfig {
            min_parity_percent: 99.0,
            max_unresolved_blockers: 0,
            max_layout_mismatches: 0,
            strict_missing_headers: true,
            require_complete_inputs: true,
        },
    }
}

impl fmt::Display for PlayabilityAuditSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.input_warnings.is_empty() {
            writeln!(f, "Input warnings:")?;
            for warning in &self.input_warnings {
                writeln!(f, "  - {}", warning)?;
            }
            writeln!(f)?;
        }
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
            "Stale mapped paths (FOUND but missing on disk): {}",
            self.stale_mapped_path_count()
        )?;
        for path in self.stale_mapped_paths.iter().take(20) {
            writeln!(f, "  stale: {path}")?;
        }
        if self.stale_mapped_path_count() > 20 {
            writeln!(
                f,
                "  ... and {} more",
                self.stale_mapped_path_count() - 20
            )?;
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
        )?;
        if self.verify_mapped_paths_enabled {
            writeln!(f, "Mapped-path verification: enabled")?;
        } else {
            writeln!(f, "Mapped-path verification: disabled")?;
        }
        Ok(())
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

    for stale in &summary.stale_mapped_paths {
        if lines.len() >= max_entries {
            break;
        }
        lines.push(format!("stale mapped path: {stale}"));
    }

    lines.into_iter().take(max_entries).collect()
}

#[derive(Debug, Clone)]
struct ParsedMatrixRow {
    subsystem: String,
    kind: MappingKind,
    status: MappingStatus,
    mapped_path: Option<String>,
}

fn parse_matrix_row(line: &str) -> Option<ParsedMatrixRow> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let parts = trimmed.splitn(5, " | ").map(str::trim).collect::<Vec<_>>();
    if parts.len() < 4 {
        return None;
    }

    let kind = MappingKind::from_prefix(parts[0])?;
    let path = parts[1]
        .trim_start_matches("Source/")
        .trim_start_matches("Include/");
    let mapped_raw = parts[2].trim();
    let mapped_path = if mapped_raw.is_empty() || mapped_raw == "-" {
        None
    } else {
        Some(mapped_raw.to_string())
    };
    let status = MappingStatus::from_value(parts[3]);
    let subsystem = path.split('/').next().unwrap_or("unknown").to_string();
    Some(ParsedMatrixRow {
        subsystem,
        kind,
        status,
        mapped_path,
    })
}

/// Returns true when a FOUND* mapped destination exists under `rust_engine_root`.
pub fn mapped_path_exists(rust_engine_root: &std::path::Path, mapped_rel: &str) -> bool {
    let cleaned = mapped_rel
        .trim()
        .trim_start_matches("./")
        .replace('\\', "/");
    if cleaned.is_empty() || cleaned == "-" {
        return false;
    }
    let candidate = rust_engine_root.join(&cleaned);
    candidate.is_file()
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
    subsystem: String,
    kind: MappingKind,
    status: MappingStatus,
    is_stale: bool,
) {
    let entry = target
        .entry(subsystem)
        .or_insert_with(|| (FileMappingCounts::default(), FileMappingCounts::default()));

    let counts = match kind {
        MappingKind::Source => &mut entry.0,
        MappingKind::Include => &mut entry.1,
    };

    if is_stale {
        counts.stale_mapped += 1;
        return;
    }

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
    let mut input_warnings = Vec::new();
    let matrix_data = read_tracking_file_or_empty(
        &config.matrix_path,
        "PORT_FILE_MATRIX.txt",
        &mut input_warnings,
    )?;
    let missing_data = read_tracking_file_or_empty(
        &config.missing_files_path,
        "PORT_MISSING_FILES_BY_SUBSYSTEM.txt",
        &mut input_warnings,
    )?;
    let mismatch_data = read_tracking_file_or_empty(
        &config.mismatch_files_path,
        "PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt",
        &mut input_warnings,
    )?;

    if matrix_data.trim().is_empty() {
        input_warnings.push(format!(
            "{} has no matrix rows; parity percentages are informational only",
            config.matrix_path.display()
        ));
    }
    if missing_data.trim().is_empty() {
        input_warnings.push(format!(
            "{} has no missing-file rows; blocker counts are informational only",
            config.missing_files_path.display()
        ));
    }

    if config.verify_mapped_paths && config.rust_engine_root.is_none() {
        input_warnings.push(
            "mapped-path verification enabled but GeneralsRust/Code/GameEngine root was not found; \
             stale-path checks are skipped until the root is available"
                .to_string(),
        );
    }

    let verify_paths = config.verify_mapped_paths && config.rust_engine_root.is_some();
    let rust_root = config.rust_engine_root.clone();

    let mut map = HashMap::<String, (FileMappingCounts, FileMappingCounts)>::new();
    let mut stale_mapped_paths = Vec::new();
    for line in matrix_data.lines() {
        if let Some(row) = parse_matrix_row(line) {
            if !config.include_network && row.subsystem == "GameNetwork" {
                continue;
            }
            let mut is_stale = false;
            if verify_paths {
                if matches!(
                    row.status,
                    MappingStatus::Found | MappingStatus::FoundByBasename
                ) {
                    match row.mapped_path.as_deref() {
                        Some(mapped) => {
                            let root = rust_root.as_ref().expect("verified above");
                            if !mapped_path_exists(root, mapped) {
                                is_stale = true;
                                stale_mapped_paths.push(mapped.to_string());
                            }
                        }
                        None => {
                            is_stale = true;
                            stale_mapped_paths.push(format!(
                                "<empty mapped path for {} {:?}>",
                                row.subsystem, row.kind
                            ));
                        }
                    }
                }
            }
            increment_counts(
                &mut map,
                row.subsystem,
                row.kind,
                row.status,
                is_stale,
            );
        }
    }
    stale_mapped_paths.sort();
    stale_mapped_paths.dedup();

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

    if !mismatch_data.trim().is_empty() {
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
        input_warnings,
        stale_mapped_paths,
        verify_mapped_paths_enabled: verify_paths,
    })
}

fn read_tracking_file_or_empty(
    path: &PathBuf,
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<String, ParseError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            warnings.push(format!(
                "{} not found at {}; continuing with empty input",
                label,
                path.display()
            ));
            Ok(String::new())
        }
        Err(err) => Err(ParseError {
            message: format!("failed to read {}: {err}", path.display()),
        }),
    }
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

        // Fixture rows use synthetic mapped paths; disable existence checks for pure parse tests.
        let config = PlayabilityGateConfig::new(matrix_path, missing_path, mismatch_path)
            .with_verify_mapped_paths(false);
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
    fn test_stale_mapped_path_fails_gate_and_lowers_parity() {
        let temp_dir = std::env::temp_dir();
        let unique = SAMPLE_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let rust_root = temp_dir.join(format!("playability_rust_root_{pid}_{unique}"));
        fs::create_dir_all(rust_root.join("GameLogic").join("src")).expect("mkdir");
        // Present file for one FOUND row; second FOUND row points at a missing path.
        fs::write(
            rust_root.join("GameLogic").join("src").join("real.rs"),
            "// present\n",
        )
        .expect("write real");

        let matrix = "\
Source | Source/GameLogic/Real.cpp | GameLogic/src/real.rs | FOUND\n\
Source | Source/GameLogic/Missing.cpp | GameLogic/src/ai/modules/missile_ai_update.rs | FOUND\n\
";
        let matrix_path = temp_dir.join(format!("playability_stale_matrix_{pid}_{unique}.txt"));
        let missing_path = temp_dir.join(format!("playability_stale_missing_{pid}_{unique}.txt"));
        let mismatch_path = temp_dir.join(format!("playability_stale_mismatch_{pid}_{unique}.txt"));
        fs::write(&matrix_path, matrix).expect("write matrix");
        fs::write(&missing_path, "").expect("write missing");
        fs::write(&mismatch_path, "").expect("write mismatch");

        let config = PlayabilityGateConfig::new(matrix_path, missing_path, mismatch_path)
            .with_verify_mapped_paths(true)
            .with_rust_engine_root(Some(rust_root));
        let summary = build_playability_audit(&config).expect("audit");

        assert_eq!(summary.stale_mapped_path_count(), 1);
        assert!(summary
            .stale_mapped_paths
            .iter()
            .any(|p| p.contains("missile_ai_update.rs")));
        assert_eq!(summary.unresolved_high_impact(), 1);
        // One covered of two total → 50% parity; stale is not counted as covered.
        assert!((summary.total_parity_percent() - 50.0).abs() < 0.1);
        // Baseline requires 50% parity but zero blockers including stale paths.
        assert!(!summary.pass_gate(PlayabilityPhase::BaselineLock));
        assert!(!summary.pass_gate(PlayabilityPhase::PlayabilityReleaseCandidate));
    }

    #[test]
    fn test_phase_thresholds_reject_zero_parity_even_without_missing_reports() {
        // Empty matrix → 0% parity. min_parity is no longer 0.0 for baseline.
        let summary = sample_config("", "", "").expect("audit parse");
        assert_eq!(summary.total_parity_percent(), 0.0);
        assert!(!summary.pass_gate(PlayabilityPhase::BaselineLock));
        assert!(!summary.pass_gate(PlayabilityPhase::GameplayParity));
    }

    #[test]
    fn test_strict_gate_rejects_incomplete_matrix_parity_with_empty_missing_report() {
        // Spot-check the dishonest path the skeptic flagged:
        // - matrix has FOUND + MISSING (40% parity)
        // - missing-files report has no Source-missing rows
        // - no stale paths (verify disabled for synthetic destinations)
        // Strict must NOT claim zero blockers / success.
        let matrix = "\
Source | Source/GameLogic/A.cpp | GameLogic/src/a.rs | FOUND\n\
Source | Source/GameLogic/B.cpp | GameLogic/src/b.rs | FOUND\n\
Source | Source/GameLogic/C.cpp | - | MISSING\n\
Source | Source/GameLogic/D.cpp | - | MISSING\n\
Source | Source/GameLogic/E.cpp | - | MISSING\n\
";
        // Empty section headers only — no Source missing lines.
        let missing = "[GameLogic]\nSource missing:\nInclude missing:\n";
        let summary = sample_config(matrix, missing, "").expect("audit parse");

        assert!(
            (summary.total_parity_percent() - 40.0).abs() < 0.1,
            "expected ~40% parity, got {}",
            summary.total_parity_percent()
        );
        assert_eq!(summary.matrix_missing_entry_count(), 3);
        // Missing-files report is empty of high-impact Source rows.
        assert_eq!(summary.source_missing_count, 0);
        // Stale not involved.
        assert_eq!(summary.stale_mapped_path_count(), 0);

        // Shipped strict path (same as playability_audit --strict without --phase).
        assert!(!summary.pass_strict_gate());
        assert!(!summary.pass_gate(PlayabilityPhase::PlayabilityReleaseCandidate));
        let strict_err = summary
            .evaluate_strict_gate()
            .expect_err("strict gate must fail on 40% matrix parity");
        assert!(
            strict_err.contains("strict") || strict_err.contains("parity") || strict_err.contains("phase5"),
            "error should describe parity failure: {strict_err}"
        );
    }

    #[test]
    fn test_strict_gate_accepts_complete_found_matrix() {
        let matrix = "\
Source | Source/GameLogic/A.cpp | GameLogic/src/a.rs | FOUND\n\
Source | Source/GameLogic/B.cpp | GameLogic/src/b.rs | FOUND\n\
Include | Include/GameLogic/A.h | GameLogic/src/a.rs | FOUND\n\
";
        let missing = "[GameLogic]\nSource missing:\nInclude missing:\n";
        let summary = sample_config(matrix, missing, "").expect("audit parse");
        assert_eq!(summary.total_parity_percent(), 100.0);
        assert_eq!(summary.matrix_missing_entry_count(), 0);
        assert!(summary.pass_strict_gate());
        assert!(summary.evaluate_strict_gate().is_ok());
    }

    #[test]
    fn test_mapped_path_exists_helper() {
        let temp_dir = std::env::temp_dir();
        let unique = SAMPLE_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let rust_root = temp_dir.join(format!("playability_exists_{pid}_{unique}"));
        let nested = rust_root.join("GameLogic").join("src");
        fs::create_dir_all(&nested).expect("mkdir");
        let file = nested.join("exists.rs");
        fs::write(&file, "ok\n").expect("write");
        assert!(mapped_path_exists(&rust_root, "GameLogic/src/exists.rs"));
        assert!(!mapped_path_exists(
            &rust_root,
            "GameLogic/src/does_not_exist.rs"
        ));
        assert!(!mapped_path_exists(&rust_root, "-"));
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
        // Precompiled misses are not blockers, but 0% parity no longer passes release.
        assert!(!summary.pass_gate(PlayabilityPhase::PlayabilityReleaseCandidate));
        assert_eq!(summary.total_parity_percent(), 0.0);
    }

    #[test]
    fn test_missing_tracking_inputs_are_reported_as_warnings_not_errors() {
        let temp_dir = std::env::temp_dir();
        let unique = SAMPLE_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let matrix_path = temp_dir.join(format!("playability_missing_matrix_{pid}_{unique}.txt"));
        let missing_path = temp_dir.join(format!("playability_missing_missing_{pid}_{unique}.txt"));
        let mismatch_path =
            temp_dir.join(format!("playability_missing_mismatch_{pid}_{unique}.txt"));

        let config = PlayabilityGateConfig::new(matrix_path, missing_path, mismatch_path)
            .with_verify_mapped_paths(false);
        let summary =
            build_playability_audit(&config).expect("audit should tolerate missing inputs");

        assert!(summary.matrix_by_subsystem.is_empty());
        assert!(summary.total_missing_reports.is_empty());
        assert!(summary.has_input_warnings());
        assert!(summary.input_warnings.len() >= 3);
    }
}
