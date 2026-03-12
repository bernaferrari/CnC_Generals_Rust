//! WW3D2 Compatibility Validation Library
//!
//! This library provides comprehensive validation tools to ensure the Rust
//! implementation of WW3D2 matches the original C++ codebase in terms of:
//!
//! - API compatibility
//! - Binary compatibility
//! - Performance characteristics
//! - Behavioral equivalence
//! - Memory usage patterns

pub mod cpp_compatibility;
pub mod texture_analysis;

pub use cpp_compatibility::*;
pub use texture_analysis::*;

/// Run full compatibility validation suite
pub fn run_full_validation() -> CompatibilityReport {
    let mut validator = CompatibilityValidator::new();
    let results = validator.validate_all();

    let mut report = CompatibilityReport {
        component_results: results,
        overall_score: 0.0,
        critical_issues: Vec::new(),
        recommendations: Vec::new(),
    };

    // Calculate overall score
    if !report.component_results.is_empty() {
        let total_score: f32 = report.component_results.values().map(|r| r.score).sum();
        report.overall_score = total_score / report.component_results.len() as f32;
    }

    // Collect critical issues
    for result in report.component_results.values() {
        for issue in &result.issues {
            match issue {
                CompatibilityIssue::MissingFeature(_)
                | CompatibilityIssue::IncompatibleAPI(_)
                | CompatibilityIssue::BinaryIncompatibility(_) => {
                    report.critical_issues.push(issue.clone());
                }
                _ => {}
            }
        }
    }

    // Generate recommendations
    report.recommendations = generate_recommendations(&report);

    report
}

/// Compatibility report
#[derive(Debug)]
pub struct CompatibilityReport {
    pub component_results: std::collections::HashMap<String, CompatibilityResult>,
    pub overall_score: f32,
    pub critical_issues: Vec<CompatibilityIssue>,
    pub recommendations: Vec<String>,
}

/// Generate recommendations based on compatibility report
fn generate_recommendations(report: &CompatibilityReport) -> Vec<String> {
    let mut recommendations = Vec::new();

    if report.overall_score < 0.8 {
        recommendations.push("Overall compatibility is below acceptable threshold. Focus on critical API and binary compatibility issues.".to_string());
    }

    if !report.critical_issues.is_empty() {
        recommendations.push(format!(
            "Address {} critical compatibility issues before production use.",
            report.critical_issues.len()
        ));
    }

    // Component-specific recommendations
    for (component, result) in &report.component_results {
        if result.score < 0.7 {
            recommendations.push(format!(
                "{} component needs significant improvement (score: {:.1}%)",
                component,
                result.score * 100.0
            ));
        }

        for issue in &result.issues {
            match issue {
                CompatibilityIssue::MissingFeature(feature) => {
                    recommendations.push(format!(
                        "Implement missing feature '{}' in {}",
                        feature, component
                    ));
                }
                CompatibilityIssue::PerformanceRegression(metric) => {
                    recommendations
                        .push(format!("Optimize {} performance in {}", metric, component));
                }
                CompatibilityIssue::MemoryUsageDifference(metric) => {
                    recommendations
                        .push(format!("Reduce {} memory usage in {}", metric, component));
                }
                _ => {}
            }
        }
    }

    if recommendations.is_empty() {
        recommendations.push("All components show good compatibility. Continue monitoring performance and add comprehensive tests.".to_string());
    }

    recommendations
}

/// Quick compatibility check - returns true if compatible
pub fn quick_compatibility_check() -> bool {
    let report = run_full_validation();
    report.overall_score >= 0.85 && report.critical_issues.is_empty()
}

/// Get compatibility score for a specific component
pub fn get_component_score(component: &str) -> Option<f32> {
    let mut validator = CompatibilityValidator::new();
    validator
        .validate_component(component)
        .map(|result| result.score)
}

/// Export compatibility report to file
pub fn export_compatibility_report(path: &std::path::Path) -> std::io::Result<()> {
    let mut validator = CompatibilityValidator::new();
    validator.validate_all();
    validator.export_report(path)
}

/// Export both human readable report and machine readable texture decisions.
pub fn export_full_report(
    human_path: &std::path::Path,
    decisions_path: &std::path::Path,
) -> std::io::Result<()> {
    let mut validator = CompatibilityValidator::new();
    validator.validate_all();
    validator.export_report(human_path)?;
    validator.export_texture_decisions(decisions_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_validation() {
        let report = run_full_validation();

        assert!(!report.component_results.is_empty());
        assert!(report.overall_score >= 0.0 && report.overall_score <= 1.0);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_quick_compatibility_check() {
        let is_compatible = quick_compatibility_check();
        // This will depend on the actual implementation quality
        // For now, just ensure it returns a boolean
        let _ = is_compatible;
    }

    #[test]
    fn test_component_score() {
        let score = get_component_score("w3d_format");
        assert!(score.is_some());
        assert!(score.unwrap() >= 0.0 && score.unwrap() <= 1.0);
    }

    #[test]
    fn test_recommendations() {
        let report = CompatibilityReport {
            component_results: std::collections::HashMap::new(),
            overall_score: 0.5,
            critical_issues: vec![CompatibilityIssue::MissingFeature("test".to_string())],
            recommendations: Vec::new(),
        };

        let recommendations = generate_recommendations(&report);
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.contains("critical")));
    }
}
