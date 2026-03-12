//! C++ WW3D2 Compatibility Validation
//!
//! This module validates the Rust implementation against the original C++ WW3D2
//! codebase to ensure feature parity, API compatibility, and performance equivalence.

use std::collections::HashMap;
use std::path::Path;
use ww3d_renderer_3d::rendering::texture_metrics;

/// Compatibility validation result
#[derive(Debug, Clone)]
pub struct CompatibilityResult {
    pub component: String,
    pub compatible: bool,
    pub issues: Vec<CompatibilityIssue>,
    pub score: f32, // 0.0 to 1.0, where 1.0 is perfect compatibility
}

/// Compatibility issue types
#[derive(Debug, Clone)]
pub enum CompatibilityIssue {
    MissingFeature(String),
    IncompatibleAPI(String),
    PerformanceRegression(String),
    MemoryUsageDifference(String),
    BinaryIncompatibility(String),
    BehavioralDifference(String),
}

impl std::fmt::Display for CompatibilityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompatibilityIssue::MissingFeature(msg) => write!(f, "Missing Feature: {}", msg),
            CompatibilityIssue::IncompatibleAPI(msg) => write!(f, "Incompatible API: {}", msg),
            CompatibilityIssue::PerformanceRegression(msg) => {
                write!(f, "Performance Regression: {}", msg)
            }
            CompatibilityIssue::MemoryUsageDifference(msg) => {
                write!(f, "Memory Usage Difference: {}", msg)
            }
            CompatibilityIssue::BinaryIncompatibility(msg) => {
                write!(f, "Binary Incompatibility: {}", msg)
            }
            CompatibilityIssue::BehavioralDifference(msg) => {
                write!(f, "Behavioral Difference: {}", msg)
            }
        }
    }
}

/// WW3D2 component specification
#[derive(Debug, Clone)]
pub struct ComponentSpec {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub api_functions: Vec<String>,
    pub data_structures: Vec<String>,
    pub performance_requirements: HashMap<String, f64>,
    pub memory_requirements: HashMap<String, usize>,
}

/// Compatibility validator
pub struct CompatibilityValidator {
    /// Component specifications from C++ WW3D2
    cpp_specs: HashMap<String, ComponentSpec>,
    /// Validation results
    results: HashMap<String, CompatibilityResult>,
}

impl CompatibilityValidator {
    /// Create a new compatibility validator
    pub fn new() -> Self {
        let mut validator = Self {
            cpp_specs: HashMap::new(),
            results: HashMap::new(),
        };

        validator.load_cpp_specifications();
        validator
    }

    /// Load C++ WW3D2 specifications
    fn load_cpp_specifications(&mut self) {
        // Core W3D file format specification
        let w3d_format_spec = ComponentSpec {
            name: "W3D File Format".to_string(),
            version: "1.0".to_string(),
            features: vec![
                "Chunk-based file structure".to_string(),
                "Hierarchical data organization".to_string(),
                "Compressed and uncompressed variants".to_string(),
                "Version compatibility".to_string(),
            ],
            api_functions: vec![
                "LoadW3DFile".to_string(),
                "SaveW3DFile".to_string(),
                "ParseChunk".to_string(),
                "ValidateChunk".to_string(),
            ],
            data_structures: vec![
                "W3dChunkHeader".to_string(),
                "W3dMesh".to_string(),
                "W3dHierarchy".to_string(),
                "W3dAnimation".to_string(),
                "W3dMaterial".to_string(),
            ],
            performance_requirements: [
                ("file_load_time".to_string(), 50.0), // milliseconds
                ("memory_usage_per_mb".to_string(), 1024.0 * 1024.0), // bytes per MB of file data
            ]
            .into(),
            memory_requirements: [
                ("minimum_heap".to_string(), 16 * 1024 * 1024), // 16MB
                ("recommended_heap".to_string(), 64 * 1024 * 1024), // 64MB
            ]
            .into(),
        };

        // WGPU runtime specification replacing the legacy DX8 wrapper
        let wgpu_runtime_spec = ComponentSpec {
            name: "WGPU Runtime".to_string(),
            version: "0.1".to_string(),
            features: vec![
                "Cross-platform GPU abstraction".to_string(),
                "Vertex/Index buffer streaming".to_string(),
                "Bind group based resource binding".to_string(),
                "Modern shader pipeline management".to_string(),
                "Render pass graph orchestration".to_string(),
                "Swap-chain and surface management".to_string(),
            ],
            api_functions: vec![
                "create_buffer".to_string(),
                "create_texture".to_string(),
                "create_render_pipeline".to_string(),
                "begin_render_pass".to_string(),
                "set_bind_group".to_string(),
                "draw_indexed".to_string(),
            ],
            data_structures: vec![
                "WgpuRuntime".to_string(),
                "WgpuBuffer".to_string(),
                "WgpuTexture".to_string(),
                "RenderPassDescriptor".to_string(),
            ],
            performance_requirements: [
                ("draw_call_overhead".to_string(), 0.05), // milliseconds
                (
                    "buffer_upload_bandwidth".to_string(),
                    300.0 * 1024.0 * 1024.0,
                ), // MB/s
            ]
            .into(),
            memory_requirements: [
                ("buffer_pool".to_string(), 16 * 1024 * 1024),  // 16MB
                ("texture_pool".to_string(), 48 * 1024 * 1024), // 48MB
            ]
            .into(),
        };

        let texture_formats_spec = ComponentSpec {
            name: "Texture Format Management".to_string(),
            version: "1.0".to_string(),
            features: vec![
                "Records format decisions".to_string(),
                "Supports legacy fallback rules".to_string(),
                "Captures decompression requirements".to_string(),
            ],
            api_functions: vec![
                "record_decision".to_string(),
                "snapshot_decisions".to_string(),
                "drain_decisions".to_string(),
            ],
            data_structures: vec![
                "TextureDecisionRecord".to_string(),
                "FormatDecision".to_string(),
                "WW3DFormat".to_string(),
            ],
            performance_requirements: [("decompression_ratio".to_string(), 0.25)].into(),
            memory_requirements: [("decision_log".to_string(), 4 * 1024 * 1024)].into(),
        };

        // Asset management specification
        let asset_mgmt_spec = ComponentSpec {
            name: "Asset Management".to_string(),
            version: "2.0".to_string(),
            features: vec![
                "Asset loading and caching".to_string(),
                "Dependency management".to_string(),
                "Memory pooling".to_string(),
                "Streaming support".to_string(),
                "Reference counting".to_string(),
            ],
            api_functions: vec![
                "LoadAsset".to_string(),
                "UnloadAsset".to_string(),
                "GetAsset".to_string(),
                "AddDependency".to_string(),
                "ProcessDependencies".to_string(),
            ],
            data_structures: vec![
                "AssetManager".to_string(),
                "AssetPrototype".to_string(),
                "AssetDependency".to_string(),
                "AssetStatus".to_string(),
            ],
            performance_requirements: [
                ("asset_load_time".to_string(), 10.0), // milliseconds
                ("cache_hit_ratio".to_string(), 0.95),
            ]
            .into(),
            memory_requirements: [
                ("asset_cache".to_string(), 128 * 1024 * 1024), // 128MB
                ("dependency_graph".to_string(), 1024 * 1024),  // 1MB
            ]
            .into(),
        };

        // Rendering pipeline specification
        let render_pipeline_spec = ComponentSpec {
            name: "Rendering Pipeline".to_string(),
            version: "3.0".to_string(),
            features: vec![
                "Forward rendering".to_string(),
                "Deferred rendering support".to_string(),
                "Multi-pass rendering".to_string(),
                "Transparency sorting".to_string(),
                "LOD management".to_string(),
                "Culling systems".to_string(),
            ],
            api_functions: vec![
                "BeginScene".to_string(),
                "EndScene".to_string(),
                "DrawMesh".to_string(),
                "SetMaterial".to_string(),
                "SetLight".to_string(),
                "SetCamera".to_string(),
            ],
            data_structures: vec![
                "RenderObj".to_string(),
                "Material".to_string(),
                "Light".to_string(),
                "Camera".to_string(),
                "Scene".to_string(),
            ],
            performance_requirements: [
                ("frame_time_60fps".to_string(), 16.67), // milliseconds
                ("draw_calls_per_frame".to_string(), 1000.0),
                ("triangles_per_frame".to_string(), 100000.0),
            ]
            .into(),
            memory_requirements: [
                ("render_state".to_string(), 512 * 1024),        // 512KB
                ("material_cache".to_string(), 4 * 1024 * 1024), // 4MB
                ("light_cache".to_string(), 256 * 1024),         // 256KB
            ]
            .into(),
        };

        self.cpp_specs
            .insert("w3d_format".to_string(), w3d_format_spec);
        self.cpp_specs
            .insert("wgpu_runtime".to_string(), wgpu_runtime_spec);
        self.cpp_specs
            .insert("texture_formats".to_string(), texture_formats_spec);
        self.cpp_specs
            .insert("asset_management".to_string(), asset_mgmt_spec);
        self.cpp_specs
            .insert("render_pipeline".to_string(), render_pipeline_spec);
    }

    /// Validate a specific component
    pub fn validate_component(&mut self, component_name: &str) -> Option<&CompatibilityResult> {
        if let Some(spec) = self.cpp_specs.get(component_name) {
            let result = self.perform_validation(component_name, spec);
            self.results.insert(component_name.to_string(), result);
        }

        self.results.get(component_name)
    }

    /// Validate all components
    pub fn validate_all(&mut self) -> HashMap<String, CompatibilityResult> {
        for (name, spec) in &self.cpp_specs {
            let result = self.perform_validation(name, spec);
            self.results.insert(name.clone(), result);
        }

        self.results.clone()
    }

    /// Perform validation for a component
    fn perform_validation(
        &self,
        component_name: &str,
        spec: &ComponentSpec,
    ) -> CompatibilityResult {
        let mut issues = Vec::new();
        let mut score: f32 = 1.0;

        // Check features
        let implemented_features = self.get_implemented_features(component_name);
        for feature in &spec.features {
            if !implemented_features.contains(feature) {
                issues.push(CompatibilityIssue::MissingFeature(feature.clone()));
                score -= 0.1;
            }
        }

        // Check API functions
        let implemented_functions = self.get_implemented_api_functions(component_name);
        for function in &spec.api_functions {
            if !implemented_functions.contains(function) {
                issues.push(CompatibilityIssue::IncompatibleAPI(format!(
                    "Missing function: {}",
                    function
                )));
                score -= 0.15;
            }
        }

        // Check data structures
        let implemented_structs = self.get_implemented_data_structures(component_name);
        for struct_name in &spec.data_structures {
            if !implemented_structs.contains(struct_name) {
                issues.push(CompatibilityIssue::IncompatibleAPI(format!(
                    "Missing struct: {}",
                    struct_name
                )));
                score -= 0.1;
            }
        }

        // Check performance requirements
        let performance_metrics = self.measure_performance(component_name);
        for (metric, required_value) in &spec.performance_requirements {
            if let Some(actual_value) = performance_metrics.get(metric) {
                let ratio = actual_value / required_value;
                if ratio > 1.5 {
                    // 50% slower is concerning
                    issues.push(CompatibilityIssue::PerformanceRegression(format!(
                        "{}: {:.2}x slower than required",
                        metric, ratio
                    )));
                    score -= 0.05;
                }
            }
        }

        // Check memory requirements
        let memory_usage = self.measure_memory_usage(component_name);
        for (metric, required_value) in &spec.memory_requirements {
            if let Some(actual_value) = memory_usage.get(metric) {
                let ratio = *actual_value as f64 / *required_value as f64;
                if ratio > 2.0 {
                    // Using 2x more memory is concerning
                    issues.push(CompatibilityIssue::MemoryUsageDifference(format!(
                        "{}: {:.2}x more memory than required",
                        metric, ratio
                    )));
                    score -= 0.05;
                }
            }
        }

        // Ensure score is within bounds
        score = score.max(0.0).min(1.0);

        CompatibilityResult {
            component: component_name.to_string(),
            compatible: issues.is_empty(),
            issues,
            score,
        }
    }

    /// Get implemented features for a component
    fn get_implemented_features(&self, component_name: &str) -> Vec<String> {
        match component_name {
            "w3d_format" => vec![
                "Chunk-based file structure".to_string(),
                "Hierarchical data organization".to_string(),
                "Version compatibility".to_string(),
            ],
            "wgpu_runtime" => vec![
                "DirectX8 API compatibility".to_string(),
                "Vertex buffer management".to_string(),
                "Index buffer management".to_string(),
                "Texture management".to_string(),
                "Render state management".to_string(),
            ],
            "texture_formats" => vec![
                "Records format decisions".to_string(),
                "Supports legacy fallback rules".to_string(),
                "Captures decompression requirements".to_string(),
            ],
            "asset_management" => vec![
                "Asset loading and caching".to_string(),
                "Dependency management".to_string(),
                "Memory pooling".to_string(),
                "Streaming support".to_string(),
            ],
            "render_pipeline" => vec![
                "Forward rendering".to_string(),
                "Multi-pass rendering".to_string(),
                "Transparency sorting".to_string(),
                "LOD management".to_string(),
            ],
            _ => Vec::new(),
        }
    }

    /// Get implemented API functions for a component
    fn get_implemented_api_functions(&self, component_name: &str) -> Vec<String> {
        match component_name {
            "w3d_format" => vec![
                "LoadW3DFile".to_string(),
                "ParseChunk".to_string(),
                "ValidateChunk".to_string(),
            ],
            "wgpu_runtime" => vec![
                "CreateVertexBuffer".to_string(),
                "CreateIndexBuffer".to_string(),
                "CreateTexture".to_string(),
                "SetRenderState".to_string(),
                "DrawPrimitive".to_string(),
            ],
            "texture_formats" => vec![
                "record_decision".to_string(),
                "snapshot_decisions".to_string(),
                "drain_decisions".to_string(),
            ],
            "asset_management" => vec![
                "LoadAsset".to_string(),
                "GetAsset".to_string(),
                "AddDependency".to_string(),
            ],
            "render_pipeline" => vec![
                "BeginScene".to_string(),
                "DrawMesh".to_string(),
                "SetMaterial".to_string(),
                "SetCamera".to_string(),
            ],
            _ => Vec::new(),
        }
    }

    /// Get implemented data structures for a component
    fn get_implemented_data_structures(&self, component_name: &str) -> Vec<String> {
        match component_name {
            "w3d_format" => vec![
                "W3dChunkHeader".to_string(),
                "W3dMesh".to_string(),
                "W3dHierarchy".to_string(),
                "W3dAnimation".to_string(),
            ],
            "wgpu_runtime" => vec![
                "DX8Wrapper".to_string(),
                "DX8VertexBuffer".to_string(),
                "DX8IndexBuffer".to_string(),
            ],
            "texture_formats" => vec![
                "TextureDecisionRecord".to_string(),
                "FormatDecision".to_string(),
                "WW3DFormat".to_string(),
            ],
            "asset_management" => vec![
                "AssetManager".to_string(),
                "AssetPrototype".to_string(),
                "AssetStatus".to_string(),
            ],
            "render_pipeline" => vec![
                "RenderObj".to_string(),
                "Material".to_string(),
                "Camera".to_string(),
            ],
            _ => Vec::new(),
        }
    }

    /// Measure performance metrics
    fn measure_performance(&self, component_name: &str) -> HashMap<String, f64> {
        // This would implement actual performance measurements
        // For now, return dummy values
        match component_name {
            "w3d_format" => [
                ("file_load_time".to_string(), 25.0),
                ("memory_usage_per_mb".to_string(), 900.0 * 1024.0),
            ]
            .into(),
            "wgpu_runtime" => [
                ("draw_call_overhead".to_string(), 0.05),
                (
                    "buffer_upload_bandwidth".to_string(),
                    150.0 * 1024.0 * 1024.0,
                ),
            ]
            .into(),
            "texture_formats" => {
                let summary = texture_metrics::summarize();
                if summary.total_textures == 0 {
                    HashMap::new()
                } else {
                    let ratio =
                        summary.decompressed_textures as f64 / summary.total_textures as f64;
                    [("decompression_ratio".to_string(), ratio)].into()
                }
            }
            "asset_management" => [
                ("asset_load_time".to_string(), 8.0),
                ("cache_hit_ratio".to_string(), 0.97),
            ]
            .into(),
            "render_pipeline" => [
                ("frame_time_60fps".to_string(), 14.0),
                ("draw_calls_per_frame".to_string(), 800.0),
                ("triangles_per_frame".to_string(), 85000.0),
            ]
            .into(),
            _ => HashMap::new(),
        }
    }

    /// Measure memory usage
    fn measure_memory_usage(&self, component_name: &str) -> HashMap<String, usize> {
        // This would implement actual memory measurements
        // For now, return dummy values
        match component_name {
            "w3d_format" => [
                ("minimum_heap".to_string(), 12 * 1024 * 1024),
                ("recommended_heap".to_string(), 48 * 1024 * 1024),
            ]
            .into(),
            "wgpu_runtime" => [
                ("vertex_buffer_pool".to_string(), 6 * 1024 * 1024),
                ("index_buffer_pool".to_string(), 3 * 1024 * 1024),
                ("texture_pool".to_string(), 24 * 1024 * 1024),
            ]
            .into(),
            "texture_formats" => {
                let summary = texture_metrics::summarize();
                let bytes = summary.total_textures
                    * std::mem::size_of::<texture_metrics::TextureDecisionRecord>();
                [("decision_log".to_string(), bytes)].into()
            }
            "asset_management" => [
                ("asset_cache".to_string(), 96 * 1024 * 1024),
                ("dependency_graph".to_string(), 512 * 1024),
            ]
            .into(),
            "render_pipeline" => [
                ("render_state".to_string(), 384 * 1024),
                ("material_cache".to_string(), 3 * 1024 * 1024),
                ("light_cache".to_string(), 192 * 1024),
            ]
            .into(),
            _ => HashMap::new(),
        }
    }

    /// Generate compatibility report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# WW3D2 Rust/C++ Compatibility Report\n\n");

        let mut total_score = 0.0;
        let mut component_count = 0;

        for (component_name, result) in &self.results {
            report.push_str(&format!("## {}\n", component_name));
            report.push_str(&format!(
                "**Compatibility Score:** {:.1}%\n",
                result.score * 100.0
            ));
            report.push_str(&format!(
                "**Status:** {}\n",
                if result.compatible {
                    "✅ Compatible"
                } else {
                    "❌ Issues Found"
                }
            ));

            if !result.issues.is_empty() {
                report.push_str("\n**Issues:**\n");
                for issue in &result.issues {
                    let icon = match issue {
                        CompatibilityIssue::MissingFeature(_) => "❌",
                        CompatibilityIssue::IncompatibleAPI(_) => "⚠️",
                        CompatibilityIssue::PerformanceRegression(_) => "🐌",
                        CompatibilityIssue::MemoryUsageDifference(_) => "💾",
                        CompatibilityIssue::BinaryIncompatibility(_) => "🔗",
                        CompatibilityIssue::BehavioralDifference(_) => "🤖",
                    };
                    report.push_str(&format!("- {} {}\n", icon, issue));
                }
            }

            report.push_str("\n---\n\n");

            total_score += result.score;
            component_count += 1;
        }

        if component_count > 0 {
            let average_score = total_score / component_count as f32;
            report.push_str(&format!(
                "## Overall Compatibility: {:.1}%\n",
                average_score * 100.0
            ));

            if average_score >= 0.95 {
                report.push_str("🎉 **Excellent compatibility!** The Rust implementation is highly compatible with C++ WW3D2.\n");
            } else if average_score >= 0.85 {
                report.push_str("✅ **Good compatibility!** Minor issues may need attention.\n");
            } else if average_score >= 0.70 {
                report
                    .push_str("⚠️ **Fair compatibility.** Several issues need to be addressed.\n");
            } else {
                report.push_str("❌ **Poor compatibility.** Significant work needed to match C++ implementation.\n");
            }
        }

        report
    }

    /// Export compatibility report to file
    pub fn export_report(&self, path: &Path) -> std::io::Result<()> {
        let report = self.generate_report();
        std::fs::write(path, report)
    }

    /// Export texture decision log to a JSON file alongside the human readable report.
    pub fn export_texture_decisions<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let decisions = texture_metrics::snapshot_decisions();
        let summary = texture_metrics::summarize();
        #[cfg(feature = "serde_support")]
        {
            let payload = serde_json::json!({
                "summary": {
                    "total_textures": summary.total_textures,
                    "decompressed_textures": summary.decompressed_textures,
                    "compressed_requests": summary.compressed_requests,
                    "average_mip_levels": summary.average_mip_levels,
                },
                "decisions": decisions,
            });
            std::fs::write(path, serde_json::to_vec_pretty(&payload)?)
        }
        #[cfg(not(feature = "serde_support"))]
        {
            let mut buffer = String::new();
            buffer.push_str("summary:\n");
            buffer.push_str(&format!(
                "  total_textures: {}\n  decompressed_textures: {}\n  compressed_requests: {}\n  average_mip_levels: {:.2}\n",
                summary.total_textures,
                summary.decompressed_textures,
                summary.compressed_requests,
                summary.average_mip_levels
            ));
            buffer.push_str("decisions:\n");
            for record in decisions {
                buffer.push_str(&format!(
                    "  - name: {}\n    source: {:?}\n    preferred: {:?}\n    resolved: {:?}\n    requires_decompression: {}\n    mip_levels: {}\n",
                    record.name,
                    record.source_format,
                    record.preferred_format,
                    record.resolved_format,
                    record.requires_decompression,
                    record.mip_levels
                ));
            }
            std::fs::write(path, buffer)
        }
    }
}

/// Binary compatibility checker
pub struct BinaryCompatibilityChecker {
    /// Struct size comparisons
    struct_sizes: HashMap<String, (usize, usize)>, // (rust_size, cpp_size)
    /// Struct alignment comparisons
    struct_alignments: HashMap<String, (usize, usize)>, // (rust_align, cpp_align)
}

impl BinaryCompatibilityChecker {
    pub fn new() -> Self {
        Self {
            struct_sizes: HashMap::new(),
            struct_alignments: HashMap::new(),
        }
    }

    /// Add struct size comparison
    pub fn add_struct_comparison(&mut self, name: &str, rust_size: usize, cpp_size: usize) {
        self.struct_sizes
            .insert(name.to_string(), (rust_size, cpp_size));
    }

    /// Add struct alignment comparison
    pub fn add_alignment_comparison(&mut self, name: &str, rust_align: usize, cpp_align: usize) {
        self.struct_alignments
            .insert(name.to_string(), (rust_align, cpp_align));
    }

    /// Check binary compatibility
    pub fn check_compatibility(&self) -> Vec<CompatibilityIssue> {
        let mut issues = Vec::new();

        for (name, (rust_size, cpp_size)) in &self.struct_sizes {
            if rust_size != cpp_size {
                issues.push(CompatibilityIssue::BinaryIncompatibility(format!(
                    "Struct {} size mismatch: Rust={} bytes, C++={} bytes",
                    name, rust_size, cpp_size
                )));
            }
        }

        for (name, (rust_align, cpp_align)) in &self.struct_alignments {
            if rust_align != cpp_align {
                issues.push(CompatibilityIssue::BinaryIncompatibility(format!(
                    "Struct {} alignment mismatch: Rust={} bytes, C++={} bytes",
                    name, rust_align, cpp_align
                )));
            }
        }

        issues
    }
}

/// Performance regression detector
pub struct PerformanceRegressionDetector {
    /// Baseline performance metrics
    baseline_metrics: HashMap<String, f64>,
    /// Current performance metrics
    current_metrics: HashMap<String, f64>,
    /// Performance thresholds
    thresholds: HashMap<String, f64>,
}

impl PerformanceRegressionDetector {
    pub fn new() -> Self {
        Self {
            baseline_metrics: HashMap::new(),
            current_metrics: HashMap::new(),
            thresholds: [
                ("frame_time".to_string(), 0.1),    // 10% regression threshold
                ("memory_usage".to_string(), 0.05), // 5% regression threshold
                ("load_time".to_string(), 0.2),     // 20% regression threshold
            ]
            .into(),
        }
    }

    /// Set baseline metric
    pub fn set_baseline(&mut self, metric: &str, value: f64) {
        self.baseline_metrics.insert(metric.to_string(), value);
    }

    /// Set current metric
    pub fn set_current(&mut self, metric: &str, value: f64) {
        self.current_metrics.insert(metric.to_string(), value);
    }

    /// Detect regressions
    pub fn detect_regressions(&self) -> Vec<CompatibilityIssue> {
        let mut issues = Vec::new();

        for (metric, &baseline) in &self.baseline_metrics {
            if let Some(&current) = self.current_metrics.get(metric) {
                if let Some(&threshold) = self.thresholds.get(metric) {
                    let regression = (current - baseline) / baseline;
                    if regression > threshold {
                        issues.push(CompatibilityIssue::PerformanceRegression(format!(
                            "{} regression: {:.1}% (baseline: {:.2}, current: {:.2})",
                            metric,
                            regression * 100.0,
                            baseline,
                            current
                        )));
                    }
                }
            }
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatibility_validator_creation() {
        let validator = CompatibilityValidator::new();
        assert!(!validator.cpp_specs.is_empty());
        assert!(validator.cpp_specs.contains_key("w3d_format"));
        assert!(validator.cpp_specs.contains_key("wgpu_runtime"));
    }

    #[test]
    fn test_component_validation() {
        let mut validator = CompatibilityValidator::new();

        let result = validator.validate_component("w3d_format");
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(!result.component.is_empty());
        // Score should be between 0.0 and 1.0
        assert!(result.score >= 0.0 && result.score <= 1.0);
    }

    #[test]
    fn test_binary_compatibility_checker() {
        let mut checker = BinaryCompatibilityChecker::new();

        checker.add_struct_comparison("W3dVector", 12, 12); // Same size
        checker.add_struct_comparison("W3dMatrix", 64, 48); // Different size

        let issues = checker.check_compatibility();
        assert_eq!(issues.len(), 1); // Should detect one size mismatch

        match &issues[0] {
            CompatibilityIssue::BinaryIncompatibility(msg) => {
                assert!(msg.contains("W3dMatrix"));
                assert!(msg.contains("size mismatch"));
            }
            _ => panic!("Expected binary incompatibility issue"),
        }
    }

    #[test]
    fn test_performance_regression_detector() {
        let mut detector = PerformanceRegressionDetector::new();

        detector.set_baseline("frame_time", 16.67); // 60 FPS
        detector.set_current("frame_time", 20.0); // ~50 FPS, 20% regression

        let regressions = detector.detect_regressions();
        assert!(!regressions.is_empty());

        match &regressions[0] {
            CompatibilityIssue::PerformanceRegression(msg) => {
                assert!(msg.contains("frame_time"));
                assert!(msg.contains("20.0%"));
            }
            _ => panic!("Expected performance regression issue"),
        }
    }

    #[test]
    fn test_report_generation() {
        let mut validator = CompatibilityValidator::new();
        validator.validate_all();

        let report = validator.generate_report();
        assert!(!report.is_empty());
        assert!(report.contains("# WW3D2 Rust/C++ Compatibility Report"));
        assert!(report.contains("## w3d_format"));
        assert!(report.contains("Overall Compatibility"));
    }
}
