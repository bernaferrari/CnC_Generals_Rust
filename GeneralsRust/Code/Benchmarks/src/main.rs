//! # C&C Generals Zero Hour - Comprehensive Benchmark Suite
//!
//! This benchmark runner provides comprehensive performance testing across all
//! game engine components with detailed reporting and analysis.

use std::time::{Duration, Instant};
use std::io::{self, Write};
use tokio::runtime::Runtime;

mod benchmark_runner;
mod performance_analysis;
mod reporting;

use benchmark_runner::BenchmarkRunner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏆 C&C Generals Zero Hour - Ultimate Benchmark Suite");
    println!("==================================================");
    println!();

    // Initialize runtime
    let rt = Runtime::new()?;
    
    // Create benchmark runner
    let mut runner = BenchmarkRunner::new()?;
    
    // Display system information
    display_system_info();
    
    // Display available benchmarks
    display_available_benchmarks();
    
    // Run benchmarks based on command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "all" => rt.block_on(run_all_benchmarks(&mut runner))?,
            "engine" => rt.block_on(run_engine_benchmarks(&mut runner))?,
            "graphics" => rt.block_on(run_graphics_benchmarks(&mut runner))?,
            "network" => rt.block_on(run_network_benchmarks(&mut runner))?,
            "audio" => rt.block_on(run_audio_benchmarks(&mut runner))?,
            "math" => rt.block_on(run_math_benchmarks(&mut runner))?,
            "memory" => rt.block_on(run_memory_benchmarks(&mut runner))?,
            "integration" => rt.block_on(run_integration_benchmarks(&mut runner))?,
            "quick" => rt.block_on(run_quick_benchmarks(&mut runner))?,
            "compare" => rt.block_on(run_comparison_benchmarks(&mut runner))?,
            _ => {
                println!("Unknown benchmark suite: {}", args[1]);
                display_usage();
            }
        }
    } else {
        // Interactive mode
        rt.block_on(run_interactive_benchmarks(&mut runner))?;
    }
    
    println!();
    println!("🎯 Benchmark suite completed successfully!");
    println!("📊 Results saved to: ./benchmark_results/");
    
    Ok(())
}

fn display_system_info() {
    use integration::hardware::detect_capabilities;
    
    let caps = detect_capabilities();
    
    println!("💻 System Information:");
    println!("  CPU: {} cores, {} threads", caps.cpu_features.cores, caps.cpu_features.threads);
    println!("  Memory: {:.2} GB total, {:.2} GB available", 
             caps.memory_info.total_mb as f64 / 1024.0,
             caps.memory_info.available_mb as f64 / 1024.0);
    
    print!("  SIMD Support: ");
    let mut simd_features = Vec::new();
    if caps.cpu_features.sse41 { simd_features.push("SSE4.1"); }
    if caps.cpu_features.sse42 { simd_features.push("SSE4.2"); }
    if caps.cpu_features.avx { simd_features.push("AVX"); }
    if caps.cpu_features.avx2 { simd_features.push("AVX2"); }
    if caps.cpu_features.avx512f { simd_features.push("AVX-512"); }
    println!("{}", simd_features.join(", "));
    
    if let Some(ref gpu_info) = caps.gpu_info {
        println!("  GPU: {} ({} MB)", gpu_info.name, gpu_info.memory_mb);
    } else {
        println!("  GPU: Not detected");
    }
    
    println!();
}

fn display_available_benchmarks() {
    println!("📋 Available Benchmark Suites:");
    println!("  all         - Run all benchmarks (comprehensive)");
    println!("  engine      - Core engine benchmarks");
    println!("  graphics    - Graphics and rendering benchmarks"); 
    println!("  network     - Network and multiplayer benchmarks");
    println!("  audio       - Audio system benchmarks");
    println!("  math        - Mathematical operations benchmarks");
    println!("  memory      - Memory management benchmarks");
    println!("  integration - Integration system benchmarks");
    println!("  quick       - Quick performance check");
    println!("  compare     - Compare with baseline performance");
    println!();
}

fn display_usage() {
    println!("Usage: cargo run --bin benchmarks [SUITE]");
    println!("       cargo run --bin benchmarks [SUITE] [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --output-format csv|json|html  Output format for results");
    println!("  --baseline FILE                Compare against baseline");
    println!("  --profile                      Generate performance profiles");
    println!("  --memory-profile               Enable memory profiling");
    println!();
}

async fn run_interactive_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Interactive Benchmark Mode");
    println!("Select benchmark suite to run:");
    println!();
    
    display_available_benchmarks();
    
    print!("Enter your choice (or 'q' to quit): ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();
    
    match choice {
        "q" | "quit" => return Ok(()),
        "all" => run_all_benchmarks(runner).await?,
        "engine" => run_engine_benchmarks(runner).await?,
        "graphics" => run_graphics_benchmarks(runner).await?,
        "network" => run_network_benchmarks(runner).await?,
        "audio" => run_audio_benchmarks(runner).await?,
        "math" => run_math_benchmarks(runner).await?,
        "memory" => run_memory_benchmarks(runner).await?,
        "integration" => run_integration_benchmarks(runner).await?,
        "quick" => run_quick_benchmarks(runner).await?,
        "compare" => run_comparison_benchmarks(runner).await?,
        _ => println!("Invalid choice: {}", choice),
    }
    
    Ok(())
}

async fn run_all_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Running Complete Benchmark Suite");
    println!("This will take approximately 10-15 minutes...");
    println!();
    
    let start_time = Instant::now();
    
    // Run all benchmark categories
    run_engine_benchmarks(runner).await?;
    run_graphics_benchmarks(runner).await?;
    run_network_benchmarks(runner).await?;
    run_audio_benchmarks(runner).await?;
    run_math_benchmarks(runner).await?;
    run_memory_benchmarks(runner).await?;
    run_integration_benchmarks(runner).await?;
    
    let total_time = start_time.elapsed();
    
    println!("⏱️  Total benchmark time: {:.2}s", total_time.as_secs_f64());
    
    // Generate comprehensive report
    runner.generate_comprehensive_report().await?;
    
    Ok(())
}

async fn run_engine_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Running Engine Benchmarks...");
    
    runner.run_benchmark_suite("engine", vec![
        "game_initialization",
        "frame_update_cycle", 
        "object_creation",
        "object_destruction",
        "entity_component_system",
        "script_execution",
        "data_loading",
        "save_game_operations",
    ]).await?;
    
    Ok(())
}

async fn run_graphics_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Running Graphics Benchmarks...");
    
    runner.run_benchmark_suite("graphics", vec![
        "w3d_rendering",
        "shader_compilation",
        "texture_loading",
        "mesh_processing",
        "particle_systems",
        "terrain_rendering", 
        "ui_rendering",
        "post_processing",
        "shadow_mapping",
        "frustum_culling",
    ]).await?;
    
    Ok(())
}

async fn run_network_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Running Network Benchmarks...");
    
    runner.run_benchmark_suite("network", vec![
        "connection_establishment",
        "message_serialization",
        "message_deserialization", 
        "bandwidth_utilization",
        "latency_measurement",
        "packet_processing",
        "state_synchronization",
        "matchmaking",
    ]).await?;
    
    Ok(())
}

async fn run_audio_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔊 Running Audio Benchmarks...");
    
    runner.run_benchmark_suite("audio", vec![
        "audio_device_initialization",
        "sound_loading",
        "sound_playback",
        "3d_audio_positioning",
        "audio_streaming",
        "compression_decompression",
        "multi_channel_mixing",
        "effects_processing",
    ]).await?;
    
    Ok(())
}

async fn run_math_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🧮 Running Math Benchmarks...");
    
    runner.run_benchmark_suite("math", vec![
        "vector_operations",
        "matrix_operations",
        "quaternion_operations",
        "collision_detection",
        "pathfinding",
        "physics_simulation",
        "spline_interpolation",
        "simd_optimizations",
    ]).await?;
    
    Ok(())
}

async fn run_memory_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("💾 Running Memory Benchmarks...");
    
    runner.run_benchmark_suite("memory", vec![
        "allocation_patterns",
        "memory_pools",
        "garbage_collection",
        "cache_performance",
        "memory_fragmentation",
        "large_object_handling",
        "memory_leak_detection",
        "compression_algorithms",
    ]).await?;
    
    Ok(())
}

async fn run_integration_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Running Integration Benchmarks...");
    
    runner.run_benchmark_suite("integration", vec![
        "system_startup",
        "subsystem_coordination",
        "event_system_performance",
        "resource_management",
        "performance_monitoring",
        "diagnostics_overhead",
        "cross_system_communication",
        "shutdown_cleanup",
    ]).await?;
    
    Ok(())
}

async fn run_quick_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Running Quick Performance Check...");
    
    runner.run_benchmark_suite("quick", vec![
        "basic_performance_check",
        "memory_allocation_speed",
        "graphics_throughput",
        "network_latency",
        "audio_latency",
        "math_operations_speed",
    ]).await?;
    
    Ok(())
}

async fn run_comparison_benchmarks(runner: &mut BenchmarkRunner) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Running Comparison Benchmarks...");
    
    // Load baseline if available
    runner.load_baseline_results().await?;
    
    // Run core benchmarks for comparison
    run_quick_benchmarks(runner).await?;
    
    // Generate comparison report
    runner.generate_comparison_report().await?;
    
    Ok(())
}
