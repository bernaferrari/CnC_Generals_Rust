//! Report generation module
//!
//! Generates comprehensive HTML benchmark reports with charts,
//! statistical analysis, and performance insights.

use crate::{BenchmarkReport, Result, BenchmarkCategory};

/// Generate HTML report from benchmark results
pub fn generate_html_report(report: &BenchmarkReport) -> Result<String> {
    let mut html = String::new();

    // HTML header with embedded CSS
    html.push_str(&format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Benchmark Report - {}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            line-height: 1.6;
            color: #333;
            background: #f5f5f5;
            padding: 20px;
        }}
        .container {{ max-width: 1400px; margin: 0 auto; background: white; padding: 40px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }}
        h1 {{ color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 15px; margin-bottom: 30px; }}
        h2 {{ color: #34495e; margin: 30px 0 15px 0; padding: 10px; background: #ecf0f1; border-left: 4px solid #3498db; }}
        h3 {{ color: #7f8c8d; margin: 20px 0 10px 0; }}
        .metadata {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 20px; margin: 20px 0; }}
        .metadata-card {{
            background: #f8f9fa;
            padding: 15px;
            border-radius: 6px;
            border-left: 4px solid #3498db;
        }}
        .metadata-card strong {{ color: #2c3e50; display: block; margin-bottom: 5px; }}
        .metadata-card span {{ color: #7f8c8d; font-size: 0.95em; }}
        table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}
        th {{ background: #34495e; color: white; padding: 12px; text-align: left; font-weight: 600; }}
        td {{ padding: 10px 12px; border-bottom: 1px solid #ecf0f1; }}
        tr:hover {{ background: #f8f9fa; }}
        .benchmark-result {{
            background: #fff;
            margin: 15px 0;
            padding: 20px;
            border-radius: 6px;
            border: 1px solid #e0e0e0;
            box-shadow: 0 1px 3px rgba(0,0,0,0.05);
        }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 15px; margin: 15px 0; }}
        .stat-box {{
            text-align: center;
            padding: 15px;
            background: #ecf0f1;
            border-radius: 6px;
        }}
        .stat-label {{ font-size: 0.85em; color: #7f8c8d; text-transform: uppercase; letter-spacing: 0.5px; }}
        .stat-value {{ font-size: 1.5em; color: #2c3e50; font-weight: 600; margin-top: 5px; }}
        .category-badge {{
            display: inline-block;
            padding: 4px 10px;
            border-radius: 12px;
            font-size: 0.85em;
            font-weight: 600;
            margin: 5px 5px 5px 0;
        }}
        .badge-cpu {{ background: #3498db; color: white; }}
        .badge-gpu {{ background: #e74c3c; color: white; }}
        .badge-memory {{ background: #9b59b6; color: white; }}
        .badge-ai {{ background: #1abc9c; color: white; }}
        .badge-compression {{ background: #f39c12; color: white; }}
        .badge-network {{ background: #16a085; color: white; }}
        .badge-graphics {{ background: #e67e22; color: white; }}
        .performance-score {{
            font-size: 3em;
            color: #27ae60;
            font-weight: bold;
            text-align: center;
            padding: 30px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border-radius: 8px;
            margin: 20px 0;
        }}
        .recommendation {{
            background: #fff3cd;
            border-left: 4px solid #ffc107;
            padding: 15px;
            margin: 10px 0;
            border-radius: 4px;
        }}
        footer {{
            margin-top: 40px;
            padding-top: 20px;
            border-top: 2px solid #ecf0f1;
            text-align: center;
            color: #7f8c8d;
            font-size: 0.9em;
        }}
        .chart-placeholder {{
            background: #f8f9fa;
            border: 2px dashed #dee2e6;
            padding: 40px;
            text-align: center;
            color: #6c757d;
            border-radius: 6px;
            margin: 20px 0;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Benchmark Report</h1>
        <p>Session ID: <code>{}</code></p>
        <p>Generated: {}</p>
"#,
        report.session_id,
        report.session_id,
        report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // System Information
    html.push_str(r#"
        <h2>System Information</h2>
        <div class="metadata">
"#);

    html.push_str(&format!(r#"
            <div class="metadata-card">
                <strong>Operating System</strong>
                <span>{}</span>
            </div>
            <div class="metadata-card">
                <strong>CPU</strong>
                <span>{} ({} cores / {} threads)</span>
            </div>
            <div class="metadata-card">
                <strong>CPU Frequency</strong>
                <span>{} MHz</span>
            </div>
            <div class="metadata-card">
                <strong>Total Memory</strong>
                <span>{} GB</span>
            </div>
        </div>
"#,
        report.system_info.os,
        report.system_info.cpu.name,
        report.system_info.cpu.cores,
        report.system_info.cpu.threads,
        report.system_info.cpu.base_frequency,
        report.system_info.memory.total / (1024 * 1024 * 1024),
    ));

    // Overall Performance Score
    html.push_str(&format!(r#"
        <h2>Overall Performance Score</h2>
        <div class="performance-score">
            {:.1}/100
        </div>
"#, report.summary.overall_performance_score));

    // Summary by Category
    html.push_str(r#"
        <h2>Summary by Category</h2>
        <table>
            <thead>
                <tr>
                    <th>Category</th>
                    <th>Benchmarks</th>
                    <th>Average Score</th>
                    <th>Best Result</th>
                    <th>Worst Result</th>
                </tr>
            </thead>
            <tbody>
"#);

    let categories = [
        BenchmarkCategory::Cpu,
        BenchmarkCategory::Gpu,
        BenchmarkCategory::Memory,
        BenchmarkCategory::Ai,
        BenchmarkCategory::Compression,
        BenchmarkCategory::Network,
    ];

    for cat in categories {
        if let Some(summary) = report.summary.categories.get(&cat) {
            let badge_class = match cat {
                BenchmarkCategory::Cpu => "badge-cpu",
                BenchmarkCategory::Gpu => "badge-gpu",
                BenchmarkCategory::Memory => "badge-memory",
                BenchmarkCategory::Ai => "badge-ai",
                BenchmarkCategory::Compression => "badge-compression",
                BenchmarkCategory::Network => "badge-network",
                _ => "badge-graphics",
            };

            html.push_str(&format!(r#"
                <tr>
                    <td><span class="category-badge {}">{}</span></td>
                    <td>{}</td>
                    <td>{:.2}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>
"#,
                badge_class,
                cat.name(),
                summary.benchmark_count,
                summary.average_score,
                summary.best_result.as_ref().unwrap_or(&"N/A".to_string()),
                summary.worst_result.as_ref().unwrap_or(&"N/A".to_string()),
            ));
        }
    }

    html.push_str(r#"
            </tbody>
        </table>
"#);

    // Detailed Results
    html.push_str(r#"
        <h2>Detailed Benchmark Results</h2>
"#);

    for result in &report.results {
        let stats = result.statistics();

        html.push_str(&format!(r#"
        <div class="benchmark-result">
            <h3>{}</h3>
            <p>{}</p>

            <div class="stats">
                <div class="stat-box">
                    <div class="stat-label">Mean</div>
                    <div class="stat-value">{:.2}</div>
                </div>
                <div class="stat-box">
                    <div class="stat-label">Median</div>
                    <div class="stat-value">{:.2}</div>
                </div>
                <div class="stat-box">
                    <div class="stat-label">Min</div>
                    <div class="stat-value">{:.2}</div>
                </div>
                <div class="stat-box">
                    <div class="stat-label">Max</div>
                    <div class="stat-value">{:.2}</div>
                </div>
                <div class="stat-box">
                    <div class="stat-label">Std Dev</div>
                    <div class="stat-value">{:.2}</div>
                </div>
                <div class="stat-box">
                    <div class="stat-label">Samples</div>
                    <div class="stat-value">{}</div>
                </div>
            </div>
"#,
            result.name,
            result.metadata.get("description").unwrap_or(&"No description".to_string()),
            stats.mean,
            stats.median,
            stats.min,
            stats.max,
            stats.std_dev,
            stats.count,
        ));

        if !stats.outliers.is_empty() {
            html.push_str(&format!(r#"
            <p><strong>Outliers detected:</strong> {} measurements</p>
"#, stats.outliers.len()));
        }

        html.push_str("        </div>\n");
    }

    // Recommendations
    html.push_str(r#"
        <h2>Performance Recommendations</h2>
"#);

    for recommendation in &report.summary.recommendations {
        html.push_str(&format!(r#"
        <div class="recommendation">
            <strong>💡 Recommendation:</strong> {}
        </div>
"#, recommendation));
    }

    // Footer
    html.push_str(&format!(r#"
        <footer>
            <p>GeneralsRust Benchmark Suite v1.0</p>
            <p>Report generated at {} with {} total benchmarks</p>
            <p>Session: {}</p>
        </footer>
    </div>
</body>
</html>
"#,
        report.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
        report.summary.total_benchmarks,
        report.session_id,
    ));

    Ok(html)
}