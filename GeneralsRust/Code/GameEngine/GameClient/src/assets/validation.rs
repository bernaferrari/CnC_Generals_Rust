//! # Asset Validation and Integrity Checking
//!
//! Comprehensive validation system ensuring asset integrity:
//! - File format validation for all supported types
//! - Checksum verification and corruption detection
//! - Structural integrity checking
//! - Dependency validation
//! - Performance impact analysis
//! - Security scanning for malicious content
//! - Fallback system for missing/corrupted assets
//! - Automatic repair and recovery

use crc32fast::Hasher as CrcHasher;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;

use super::{AssetError, AssetHandle, AssetType};

/// Asset validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Format validation failed for {path}: {errors:?}")]
    FormatValidationFailed { path: String, errors: Vec<String> },
    #[error("Checksum mismatch: {path} - expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("Asset corrupted: {path} - {reason}")]
    AssetCorrupted { path: String, reason: String },
    #[error("Security scan failed: {path} - {threats:?}")]
    SecurityThreatDetected { path: String, threats: Vec<String> },
    #[error("Dependency validation failed: {asset} - missing {dependencies:?}")]
    DependencyValidationFailed {
        asset: String,
        dependencies: Vec<String>,
    },
    #[error("Performance validation failed: {path} - {issues:?}")]
    PerformanceIssues { path: String, issues: Vec<String> },
    #[error("Structural validation failed: {path} - {reason}")]
    StructuralValidationFailed { path: String, reason: String },
    #[error("Repair failed: {path} - {error}")]
    RepairFailed { path: String, error: String },
}

/// Validation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub issue_type: ValidationIssueType,
    pub severity: ValidationSeverity,
    pub message: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
    pub can_auto_fix: bool,
}

/// Types of validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationIssueType {
    FormatCorruption,
    ChecksumMismatch,
    StructuralError,
    SecurityThreat,
    PerformanceIssue,
    DependencyMissing,
    CompatibilityIssue,
    MemoryIssue,
}

/// Asset validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub is_valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub checksum: String,
    pub validation_time: Duration,
    pub file_size: u64,
    pub last_modified: Option<SystemTime>,
    pub repair_suggestions: Vec<RepairSuggestion>,
}

/// Repair suggestion for fixing issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairSuggestion {
    pub issue_type: ValidationIssueType,
    pub action: RepairAction,
    pub description: String,
    pub success_probability: f32,
    pub estimated_time: Duration,
}

/// Available repair actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairAction {
    UseBackup,
    RegenerateFromSource,
    DownloadFromServer,
    UseFallback,
    RepairStructure,
    RecompileAsset,
    UpdateDependencies,
}

/// Fallback asset configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    pub asset_type: AssetType,
    pub fallback_path: PathBuf,
    pub generation_method: FallbackGeneration,
    pub quality_level: f32,
    pub cache_fallbacks: bool,
}

/// Fallback generation methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackGeneration {
    UseStaticAsset,     // Use pre-made fallback
    GenerateProcedural, // Generate on-the-fly
    UseLastKnownGood,   // Use backup/cache
    DownloadOnDemand,   // Fetch from server
}

/// Security scanning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enabled: bool,
    pub scan_for_malware: bool,
    pub check_file_headers: bool,
    pub validate_signatures: bool,
    pub max_file_size: u64,
    pub blocked_extensions: Vec<String>,
    pub scan_embedded_content: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_for_malware: false, // Would require antivirus integration
            check_file_headers: true,
            validate_signatures: true,
            max_file_size: 1024 * 1024 * 1024, // 1GB
            blocked_extensions: vec![
                "exe".to_string(),
                "dll".to_string(),
                "bat".to_string(),
                "com".to_string(),
                "scr".to_string(),
            ],
            scan_embedded_content: true,
        }
    }
}

/// Asset validation statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ValidationStats {
    pub total_validations: u64,
    pub passed_validations: u64,
    pub failed_validations: u64,
    pub issues_found: u64,
    pub auto_repairs: u64,
    pub fallbacks_used: u64,
    pub average_validation_time_ms: f32,
    pub security_threats_blocked: u64,
    pub performance_issues_found: u64,
}

/// Asset integrity database entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityRecord {
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub checksum_sha256: String,
    pub checksum_crc32: u32,
    pub file_size: u64,
    pub last_validated: SystemTime,
    pub validation_count: u64,
    pub issue_history: Vec<ValidationIssue>,
    pub repair_history: Vec<RepairAttempt>,
    pub performance_metrics: AssetPerformanceMetrics,
}

/// Record of repair attempts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttempt {
    pub timestamp: SystemTime,
    pub action: RepairAction,
    pub success: bool,
    pub error_message: Option<String>,
    pub time_taken: Duration,
}

/// Asset performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPerformanceMetrics {
    pub load_time_avg: Duration,
    pub memory_usage: u64,
    pub cpu_impact: f32,
    pub io_operations: u32,
    pub cache_efficiency: f32,
    pub compression_ratio: f32,
}

impl Default for AssetPerformanceMetrics {
    fn default() -> Self {
        Self {
            load_time_avg: Duration::from_millis(0),
            memory_usage: 0,
            cpu_impact: 0.0,
            io_operations: 0,
            cache_efficiency: 1.0,
            compression_ratio: 1.0,
        }
    }
}

/// Complete Asset Validation System
pub struct AssetValidator {
    // Configuration
    security_config: SecurityConfig,
    fallback_configs: HashMap<AssetType, FallbackConfig>,

    // Integrity database
    integrity_db: Arc<RwLock<HashMap<PathBuf, IntegrityRecord>>>,

    // Format validators
    format_validators: HashMap<AssetType, Box<dyn FormatValidator + Send + Sync>>,

    // Fallback assets cache
    fallback_cache: Arc<RwLock<HashMap<AssetType, Vec<u8>>>>,

    // Last known good assets cache
    last_known_good: Arc<RwLock<HashMap<AssetType, Vec<u8>>>>,

    // Statistics
    stats: Arc<RwLock<ValidationStats>>,

    // Known good checksums database
    known_checksums: Arc<RwLock<HashMap<PathBuf, String>>>,
}

/// Trait for format-specific validators
pub trait FormatValidator {
    fn validate(&self, data: &[u8], path: &Path) -> Result<Vec<ValidationIssue>, ValidationError>;
    fn can_repair(&self, issue: &ValidationIssue) -> bool;
    fn repair(&self, data: &[u8], issue: &ValidationIssue) -> Result<Vec<u8>, ValidationError>;
}

impl AssetValidator {
    /// Create new asset validator
    pub fn new() -> Self {
        let mut fallback_configs = HashMap::new();

        // Setup default fallback configurations
        fallback_configs.insert(
            AssetType::Texture,
            FallbackConfig {
                asset_type: AssetType::Texture,
                fallback_path: PathBuf::from("fallback/missing_texture.png"),
                generation_method: FallbackGeneration::GenerateProcedural,
                quality_level: 0.5,
                cache_fallbacks: true,
            },
        );

        fallback_configs.insert(
            AssetType::Model,
            FallbackConfig {
                asset_type: AssetType::Model,
                fallback_path: PathBuf::from("fallback/missing_model.w3d"),
                generation_method: FallbackGeneration::UseStaticAsset,
                quality_level: 0.3,
                cache_fallbacks: true,
            },
        );

        fallback_configs.insert(
            AssetType::Audio,
            FallbackConfig {
                asset_type: AssetType::Audio,
                fallback_path: PathBuf::from("fallback/silence.wav"),
                generation_method: FallbackGeneration::UseStaticAsset,
                quality_level: 0.1,
                cache_fallbacks: false,
            },
        );

        let mut format_validators: HashMap<AssetType, Box<dyn FormatValidator + Send + Sync>> =
            HashMap::new();

        // Register format validators
        format_validators.insert(AssetType::Texture, Box::new(TextureValidator));
        format_validators.insert(AssetType::Model, Box::new(W3DValidator));
        format_validators.insert(AssetType::Audio, Box::new(AudioValidator));

        Self {
            security_config: SecurityConfig::default(),
            fallback_configs,
            integrity_db: Arc::new(RwLock::new(HashMap::new())),
            format_validators,
            fallback_cache: Arc::new(RwLock::new(HashMap::new())),
            last_known_good: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ValidationStats::default())),
            known_checksums: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Validate asset integrity and format
    pub async fn validate_asset(
        &self,
        path: &Path,
        data: &[u8],
        asset_type: AssetType,
    ) -> Result<ValidationResult, ValidationError> {
        let start_time = Instant::now();
        let mut issues = Vec::new();

        log::debug!("Validating asset: {} ({:?})", path.display(), asset_type);

        // Basic integrity checks
        if data.is_empty() {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::StructuralError,
                severity: ValidationSeverity::Critical,
                message: "Asset file is empty".to_string(),
                location: Some("file_size".to_string()),
                suggestion: Some("Check if file was corrupted during transfer".to_string()),
                can_auto_fix: false,
            });
        }

        // Security scanning
        if self.security_config.enabled {
            let security_issues = self.perform_security_scan(path, data).await?;
            issues.extend(security_issues);
        }

        // Checksum validation
        let checksum = self.calculate_checksum_sha256(data);
        if let Some(expected_checksum) = self.get_known_checksum(path) {
            if checksum != expected_checksum {
                issues.push(ValidationIssue {
                    issue_type: ValidationIssueType::ChecksumMismatch,
                    severity: ValidationSeverity::Error,
                    message: format!(
                        "Checksum mismatch: expected {}, got {}",
                        expected_checksum, checksum
                    ),
                    location: None,
                    suggestion: Some("Re-download or restore from backup".to_string()),
                    can_auto_fix: true,
                });
            }
        }

        // Format-specific validation
        if let Some(validator) = self.format_validators.get(&asset_type) {
            match validator.validate(data, path) {
                Ok(format_issues) => issues.extend(format_issues),
                Err(e) => {
                    issues.push(ValidationIssue {
                        issue_type: ValidationIssueType::FormatCorruption,
                        severity: ValidationSeverity::Critical,
                        message: format!("Format validation failed: {}", e),
                        location: None,
                        suggestion: Some("Asset may be corrupted or in wrong format".to_string()),
                        can_auto_fix: false,
                    });
                }
            }
        }

        // Performance validation
        let performance_issues = self.validate_performance(path, data, asset_type).await;
        issues.extend(performance_issues);

        // Generate repair suggestions
        let repair_suggestions = self.generate_repair_suggestions(&issues, asset_type);

        let validation_time = start_time.elapsed();
        let is_valid = !issues
            .iter()
            .any(|i| i.severity >= ValidationSeverity::Error);

        let result = ValidationResult {
            path: path.to_path_buf(),
            asset_type,
            is_valid,
            issues,
            checksum,
            validation_time,
            file_size: data.len() as u64,
            last_modified: tokio::fs::metadata(path)
                .await
                .ok()
                .and_then(|m| m.modified().ok()),
            repair_suggestions,
        };

        // Update integrity database
        self.update_integrity_record(path, &result, asset_type)
            .await;

        if is_valid {
            self.store_last_known_good(asset_type, data);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.total_validations += 1;
            if is_valid {
                stats.passed_validations += 1;
            } else {
                stats.failed_validations += 1;
            }
            stats.issues_found += result.issues.len() as u64;

            // Update average validation time
            let total_time =
                stats.average_validation_time_ms * (stats.total_validations - 1) as f32;
            stats.average_validation_time_ms =
                (total_time + validation_time.as_millis() as f32) / stats.total_validations as f32;
        }

        log::debug!(
            "Validation complete: {} ({} issues, {} ms)",
            path.display(),
            result.issues.len(),
            validation_time.as_millis()
        );

        Ok(result)
    }

    /// Perform security scanning
    async fn perform_security_scan(
        &self,
        path: &Path,
        data: &[u8],
    ) -> Result<Vec<ValidationIssue>, ValidationError> {
        let mut issues = Vec::new();

        // Check file size limits
        if data.len() as u64 > self.security_config.max_file_size {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::SecurityThreat,
                severity: ValidationSeverity::Warning,
                message: format!("File size ({} bytes) exceeds security limit", data.len()),
                location: Some("file_size".to_string()),
                suggestion: Some("Consider compressing or splitting large assets".to_string()),
                can_auto_fix: false,
            });
        }

        // Check blocked extensions
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            if self
                .security_config
                .blocked_extensions
                .contains(&extension.to_lowercase())
            {
                issues.push(ValidationIssue {
                    issue_type: ValidationIssueType::SecurityThreat,
                    severity: ValidationSeverity::Critical,
                    message: format!("File extension '{}' is blocked for security", extension),
                    location: Some("file_extension".to_string()),
                    suggestion: Some("Use allowed file formats only".to_string()),
                    can_auto_fix: false,
                });
            }
        }

        // Check file headers
        if self.security_config.check_file_headers {
            let header_issues = self.validate_file_headers(data, path).await;
            issues.extend(header_issues);
        }

        // Scan for embedded content
        if self.security_config.scan_embedded_content {
            let embedded_issues = self.scan_embedded_content(data).await;
            issues.extend(embedded_issues);
        }

        Ok(issues)
    }

    /// Validate file headers match expected format
    async fn validate_file_headers(&self, data: &[u8], path: &Path) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if data.len() < 8 {
            return issues;
        }

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let header = &data[0..8.min(data.len())];

        let expected_signatures: Vec<&[u8]> = match extension.to_lowercase().as_str() {
            "png" => vec![b"\x89PNG\r\n\x1A\n"],
            "jpg" | "jpeg" => vec![b"\xFF\xD8\xFF"],
            "wav" => vec![b"RIFF"],
            "ogg" => vec![b"OggS"],
            "w3d" => vec![b"W3D\0"],
            _ => return issues,
        };

        let header_matches = expected_signatures
            .iter()
            .any(|sig| header.len() >= sig.len() && header[0..sig.len()] == **sig);

        if !header_matches {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::SecurityThreat,
                severity: ValidationSeverity::Warning,
                message: format!("File header doesn't match extension .{}", extension),
                location: Some("file_header".to_string()),
                suggestion: Some("Verify file format and extension match".to_string()),
                can_auto_fix: false,
            });
        }

        issues
    }

    /// Scan for potentially dangerous embedded content
    async fn scan_embedded_content(&self, data: &[u8]) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Look for suspicious patterns
        let suspicious_patterns: &[&[u8]] = &[
            b"<script",
            b"javascript:",
            b"data:text/html",
            b"eval(",
            b"setTimeout(",
            b"setInterval(",
        ];

        for pattern in suspicious_patterns {
            if data.windows(pattern.len()).any(|window| window == *pattern) {
                issues.push(ValidationIssue {
                    issue_type: ValidationIssueType::SecurityThreat,
                    severity: ValidationSeverity::Error,
                    message: format!(
                        "Suspicious embedded content detected: {:?}",
                        String::from_utf8_lossy(pattern)
                    ),
                    location: Some("embedded_content".to_string()),
                    suggestion: Some("Remove or sanitize embedded scripts".to_string()),
                    can_auto_fix: false,
                });
            }
        }

        issues
    }

    /// Validate asset performance characteristics
    async fn validate_performance(
        &self,
        path: &Path,
        data: &[u8],
        asset_type: AssetType,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check file size vs. expected size for asset type
        let expected_max_size = match asset_type {
            AssetType::Texture => 16 * 1024 * 1024, // 16MB
            AssetType::Audio => 32 * 1024 * 1024,   // 32MB
            AssetType::Model => 8 * 1024 * 1024,    // 8MB
            _ => 64 * 1024 * 1024,                  // 64MB default
        };

        if data.len() as u64 > expected_max_size {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::PerformanceIssue,
                severity: ValidationSeverity::Warning,
                message: format!(
                    "Asset size ({} MB) may impact performance",
                    data.len() / (1024 * 1024)
                ),
                location: Some("file_size".to_string()),
                suggestion: Some("Consider compression or optimization".to_string()),
                can_auto_fix: true,
            });
        }

        // Check for compression opportunities
        let compression_ratio = self.estimate_compression_ratio(data).await;
        if compression_ratio > 2.0 && !path.to_string_lossy().contains("compressed") {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::PerformanceIssue,
                severity: ValidationSeverity::Info,
                message: format!(
                    "Asset could be compressed (estimated {:.1}x reduction)",
                    compression_ratio
                ),
                location: Some("compression".to_string()),
                suggestion: Some("Apply lossless compression".to_string()),
                can_auto_fix: true,
            });
        }

        issues
    }

    /// Estimate potential compression ratio
    async fn estimate_compression_ratio(&self, data: &[u8]) -> f32 {
        if data.len() < 1024 {
            return 1.0;
        }

        // Simple entropy-based estimation
        let mut byte_counts = [0u32; 256];
        for &byte in data {
            byte_counts[byte as usize] += 1;
        }

        // Calculate entropy
        let len = data.len() as f32;
        let entropy: f32 = byte_counts
            .iter()
            .filter(|&&count| count > 0)
            .map(|&count| {
                let p = count as f32 / len;
                -p * p.log2()
            })
            .sum();

        // Estimate compression ratio based on entropy
        // Lower entropy = better compression potential
        let max_entropy = 8.0;
        let compression_potential = (max_entropy - entropy) / max_entropy;
        1.0 + (compression_potential * 10.0).max(0.0)
    }

    /// Generate repair suggestions for issues
    fn generate_repair_suggestions(
        &self,
        issues: &[ValidationIssue],
        asset_type: AssetType,
    ) -> Vec<RepairSuggestion> {
        let mut suggestions = Vec::new();

        for issue in issues {
            if issue.can_auto_fix {
                let suggestion = match issue.issue_type {
                    ValidationIssueType::ChecksumMismatch => RepairSuggestion {
                        issue_type: issue.issue_type,
                        action: RepairAction::DownloadFromServer,
                        description: "Re-download asset from original source".to_string(),
                        success_probability: 0.9,
                        estimated_time: Duration::from_secs(30),
                    },
                    ValidationIssueType::PerformanceIssue => RepairSuggestion {
                        issue_type: issue.issue_type,
                        action: RepairAction::RecompileAsset,
                        description: "Optimize asset for better performance".to_string(),
                        success_probability: 0.8,
                        estimated_time: Duration::from_secs(60),
                    },
                    ValidationIssueType::StructuralError => RepairSuggestion {
                        issue_type: issue.issue_type,
                        action: RepairAction::RepairStructure,
                        description: "Attempt to repair asset structure".to_string(),
                        success_probability: 0.6,
                        estimated_time: Duration::from_secs(10),
                    },
                    _ => continue,
                };
                suggestions.push(suggestion);
            }
        }

        // Add fallback suggestion as last resort
        if !issues.is_empty() && self.fallback_configs.contains_key(&asset_type) {
            suggestions.push(RepairSuggestion {
                issue_type: ValidationIssueType::FormatCorruption,
                action: RepairAction::UseFallback,
                description: "Use fallback asset as temporary replacement".to_string(),
                success_probability: 1.0,
                estimated_time: Duration::from_millis(100),
            });
        }

        suggestions
    }

    /// Get fallback asset data
    pub async fn get_fallback_asset(
        &self,
        asset_type: AssetType,
    ) -> Result<Vec<u8>, ValidationError> {
        // Check cache first
        {
            let cache = self
                .fallback_cache
                .read()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(cached_data) = cache.get(&asset_type) {
                return Ok(cached_data.clone());
            }
        }

        // Generate or load fallback
        let fallback_data = if let Some(config) = self.fallback_configs.get(&asset_type) {
            match config.generation_method {
                FallbackGeneration::UseStaticAsset => {
                    self.load_static_fallback(&config.fallback_path).await?
                }
                FallbackGeneration::GenerateProcedural => {
                    self.generate_procedural_fallback(asset_type, config.quality_level)
                        .await?
                }
                FallbackGeneration::UseLastKnownGood => {
                    self.get_last_known_good(asset_type).await?
                }
                FallbackGeneration::DownloadOnDemand => {
                    self.download_fallback_asset(asset_type).await?
                }
            }
        } else {
            return Err(ValidationError::RepairFailed {
                path: "fallback".to_string(),
                error: format!("No fallback configuration for {:?}", asset_type),
            });
        };

        // Cache if configured to do so
        if let Some(config) = self.fallback_configs.get(&asset_type) {
            if config.cache_fallbacks {
                self.fallback_cache
                    .write()
                    .unwrap()
                    .insert(asset_type, fallback_data.clone());
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.fallbacks_used += 1;
        }

        log::info!(
            "Generated fallback asset for {:?} ({} bytes)",
            asset_type,
            fallback_data.len()
        );
        Ok(fallback_data)
    }

    /// Load static fallback asset from file
    async fn load_static_fallback(&self, path: &Path) -> Result<Vec<u8>, ValidationError> {
        tokio::fs::read(path)
            .await
            .map_err(|e| ValidationError::RepairFailed {
                path: path.to_string_lossy().to_string(),
                error: e.to_string(),
            })
    }

    /// Generate procedural fallback asset
    async fn generate_procedural_fallback(
        &self,
        asset_type: AssetType,
        quality: f32,
    ) -> Result<Vec<u8>, ValidationError> {
        match asset_type {
            AssetType::Texture => self.generate_fallback_texture(quality).await,
            AssetType::Audio => self.generate_fallback_audio(quality).await,
            AssetType::Model => self.generate_fallback_model(quality).await,
            _ => Err(ValidationError::RepairFailed {
                path: "procedural".to_string(),
                error: format!("Procedural generation not supported for {:?}", asset_type),
            }),
        }
    }

    /// Generate fallback texture
    async fn generate_fallback_texture(&self, quality: f32) -> Result<Vec<u8>, ValidationError> {
        // Generate a simple checkerboard pattern
        let size = (64.0 * quality).max(16.0) as u32;
        let mut data = Vec::with_capacity((size * size * 4) as usize);

        for y in 0..size {
            for x in 0..size {
                let checker = ((x / 8) + (y / 8)) % 2;
                let color = if checker == 0 { 255u8 } else { 128u8 };
                data.extend_from_slice(&[color, 0, color, 255]); // Magenta checkerboard
            }
        }

        // In a real implementation, this would generate a proper PNG/TGA file
        Ok(data)
    }

    /// Generate fallback audio
    async fn generate_fallback_audio(&self, quality: f32) -> Result<Vec<u8>, ValidationError> {
        // Generate silence or simple tone
        let sample_rate = (22050.0 * quality).max(8000.0) as u32;
        let duration_samples = sample_rate; // 1 second

        let mut wav_data = Vec::new();

        // WAV header (44 bytes)
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&(36 + duration_samples * 2).to_le_bytes()); // File size - 8
        wav_data.extend_from_slice(b"WAVE");
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes()); // Fmt chunk size
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // Mono
        wav_data.extend_from_slice(&sample_rate.to_le_bytes()); // Sample rate
        wav_data.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // Byte rate
        wav_data.extend_from_slice(&2u16.to_le_bytes()); // Block align
        wav_data.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&(duration_samples * 2).to_le_bytes()); // Data size

        // Generate silence
        for _ in 0..duration_samples {
            wav_data.extend_from_slice(&0i16.to_le_bytes());
        }

        Ok(wav_data)
    }

    /// Generate fallback model
    async fn generate_fallback_model(&self, _quality: f32) -> Result<Vec<u8>, ValidationError> {
        // Minimal W3D header so loader can parse an empty model.
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(b"W3D\0");
        data.extend_from_slice(&1u32.to_le_bytes());
        Ok(data)
    }

    /// Get last known good version of asset
    async fn get_last_known_good(&self, asset_type: AssetType) -> Result<Vec<u8>, ValidationError> {
        if let Some(data) = self
            .last_known_good
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&asset_type)
        {
            return Ok(data.clone());
        }

        Err(ValidationError::RepairFailed {
            path: format!("last_known_good:{:?}", asset_type),
            error: "No cached last-known-good asset".to_string(),
        })
    }

    /// Download fallback asset from server
    async fn download_fallback_asset(
        &self,
        asset_type: AssetType,
    ) -> Result<Vec<u8>, ValidationError> {
        let config = self.fallback_configs.get(&asset_type).ok_or_else(|| {
            ValidationError::RepairFailed {
                path: "fallback".to_string(),
                error: format!("No fallback configuration for {:?}", asset_type),
            }
        })?;

        let target = config.fallback_path.to_string_lossy();
        if let Ok(url) = Url::parse(&target) {
            let response =
                reqwest::get(url.clone())
                    .await
                    .map_err(|error| ValidationError::RepairFailed {
                        path: url.to_string(),
                        error: error.to_string(),
                    })?;

            if !response.status().is_success() {
                return Err(ValidationError::RepairFailed {
                    path: url.to_string(),
                    error: format!("HTTP {}", response.status()),
                });
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|error| ValidationError::RepairFailed {
                    path: url.to_string(),
                    error: error.to_string(),
                })?;
            Ok(bytes.to_vec())
        } else {
            tokio::fs::read(&config.fallback_path)
                .await
                .map_err(|error| ValidationError::RepairFailed {
                    path: config.fallback_path.to_string_lossy().to_string(),
                    error: error.to_string(),
                })
        }
    }

    /// Calculate SHA-256 checksum
    fn calculate_checksum_sha256(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Calculate CRC32 checksum
    fn calculate_checksum_crc32(&self, data: &[u8]) -> u32 {
        let mut hasher = CrcHasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// Get known checksum for asset
    fn get_known_checksum(&self, path: &Path) -> Option<String> {
        self.known_checksums
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned()
    }

    /// Update integrity database record
    async fn update_integrity_record(
        &self,
        path: &Path,
        result: &ValidationResult,
        asset_type: AssetType,
    ) {
        let mut db = self.integrity_db.write().unwrap_or_else(|e| e.into_inner());

        let record = db
            .entry(path.to_path_buf())
            .or_insert_with(|| IntegrityRecord {
                path: path.to_path_buf(),
                asset_type,
                checksum_sha256: String::new(),
                checksum_crc32: 0,
                file_size: 0,
                last_validated: SystemTime::UNIX_EPOCH,
                validation_count: 0,
                issue_history: Vec::new(),
                repair_history: Vec::new(),
                performance_metrics: AssetPerformanceMetrics::default(),
            });

        record.checksum_sha256 = result.checksum.clone();
        record.file_size = result.file_size;
        record.last_validated = SystemTime::now();
        record.validation_count += 1;

        // Keep limited history
        record.issue_history.extend(result.issues.iter().cloned());
        if record.issue_history.len() > 100 {
            record.issue_history.drain(0..50); // Keep last 50
        }
    }

    /// Get validation statistics
    pub fn get_stats(&self) -> ValidationStats {
        self.stats.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Get integrity record for asset
    pub fn get_integrity_record(&self, path: &Path) -> Option<IntegrityRecord> {
        self.integrity_db
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned()
    }

    /// Add known good checksum
    pub fn add_known_checksum(&self, path: PathBuf, checksum: String) {
        self.known_checksums
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path, checksum);
    }

    fn store_last_known_good(&self, asset_type: AssetType, data: &[u8]) {
        self.last_known_good
            .write()
            .unwrap()
            .insert(asset_type, data.to_vec());
    }
}

// Format-specific validators
struct TextureValidator;
struct W3DValidator;
struct AudioValidator;

impl FormatValidator for TextureValidator {
    fn validate(&self, data: &[u8], _path: &Path) -> Result<Vec<ValidationIssue>, ValidationError> {
        let mut issues = Vec::new();

        if data.len() < 8 {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::StructuralError,
                severity: ValidationSeverity::Critical,
                message: "Texture file too small".to_string(),
                location: Some("file_size".to_string()),
                suggestion: Some("File may be truncated or corrupted".to_string()),
                can_auto_fix: false,
            });
        }

        // Add more texture-specific validation logic here

        Ok(issues)
    }

    fn can_repair(&self, issue: &ValidationIssue) -> bool {
        matches!(issue.issue_type, ValidationIssueType::PerformanceIssue)
    }

    fn repair(&self, data: &[u8], _issue: &ValidationIssue) -> Result<Vec<u8>, ValidationError> {
        // Simple repair - return original data
        // Real implementation would perform actual repairs
        Ok(data.to_vec())
    }
}

impl FormatValidator for W3DValidator {
    fn validate(&self, data: &[u8], _path: &Path) -> Result<Vec<ValidationIssue>, ValidationError> {
        let mut issues = Vec::new();

        if data.len() < 8 || &data[0..4] != b"W3D\0" {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::FormatCorruption,
                severity: ValidationSeverity::Critical,
                message: "Invalid W3D signature".to_string(),
                location: Some("header".to_string()),
                suggestion: Some("File is not a valid W3D model".to_string()),
                can_auto_fix: false,
            });
        }

        Ok(issues)
    }

    fn can_repair(&self, _issue: &ValidationIssue) -> bool {
        false
    }

    fn repair(&self, data: &[u8], _issue: &ValidationIssue) -> Result<Vec<u8>, ValidationError> {
        Ok(data.to_vec())
    }
}

impl FormatValidator for AudioValidator {
    fn validate(&self, data: &[u8], _path: &Path) -> Result<Vec<ValidationIssue>, ValidationError> {
        let mut issues = Vec::new();

        if data.len() < 44 {
            issues.push(ValidationIssue {
                issue_type: ValidationIssueType::StructuralError,
                severity: ValidationSeverity::Error,
                message: "Audio file too small for valid header".to_string(),
                location: Some("file_size".to_string()),
                suggestion: Some("File may be corrupted".to_string()),
                can_auto_fix: false,
            });
        }

        Ok(issues)
    }

    fn can_repair(&self, _issue: &ValidationIssue) -> bool {
        false
    }

    fn repair(&self, data: &[u8], _issue: &ValidationIssue) -> Result<Vec<u8>, ValidationError> {
        Ok(data.to_vec())
    }
}

impl From<ValidationError> for AssetError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::ChecksumMismatch {
                path,
                expected,
                actual,
            } => AssetError::Corrupted {
                path,
                expected_size: expected.len() as u64,
                actual_size: actual.len() as u64,
            },
            ValidationError::AssetCorrupted { path, reason } => {
                AssetError::ValidationFailed { path, reason }
            }
            _ => AssetError::ValidationFailed {
                path: "validation".to_string(),
                reason: err.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_checksum_calculation() {
        let validator = AssetValidator::new();
        let data = b"test data";
        let checksum = validator.calculate_checksum_sha256(data);
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex characters
    }

    #[tokio::test]
    async fn test_fallback_texture_generation() {
        let validator = AssetValidator::new();
        let texture_data = validator.generate_fallback_texture(1.0).await.unwrap();
        assert!(!texture_data.is_empty());
    }

    #[test]
    fn test_validation_issue_creation() {
        let issue = ValidationIssue {
            issue_type: ValidationIssueType::ChecksumMismatch,
            severity: ValidationSeverity::Error,
            message: "Test issue".to_string(),
            location: None,
            suggestion: None,
            can_auto_fix: true,
        };

        assert_eq!(issue.severity, ValidationSeverity::Error);
        assert!(issue.can_auto_fix);
    }
}
