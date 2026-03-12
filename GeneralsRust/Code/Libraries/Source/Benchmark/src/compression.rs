//! Compression benchmarking module

use crate::{BenchmarkConfig, BenchmarkResult, BenchmarkCategory, Measurement, MeasurementUnit, Result};
use std::time::Instant;

/// Compression benchmarks
pub struct CompressionBenchmarks {
    config: BenchmarkConfig,
}

impl CompressionBenchmarks {
    pub fn new(config: &BenchmarkConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
    
    pub async fn run_all(&mut self) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();
        
        // Test different compression algorithms with various data types
        results.extend(self.benchmark_text_compression().await?);
        results.extend(self.benchmark_binary_compression().await?);
        results.extend(self.benchmark_repetitive_data_compression().await?);
        results.extend(self.benchmark_auto_compression().await?);
        
        Ok(results)
    }
    
    async fn benchmark_text_compression(&self) -> Result<Vec<BenchmarkResult>> {
        #[cfg(feature = "compression")]
        {
            use generals_compression::{compress, decompress, CompressionType};
            
            let test_data = "The quick brown fox jumps over the lazy dog. ".repeat(1000);
            let data_bytes = test_data.as_bytes();
            
            let mut results = Vec::new();
            
            for compression_type in &[
                CompressionType::RefPack,
                CompressionType::BTree,
                CompressionType::Huffman,
                CompressionType::LZH,
                CompressionType::ZLib(6),
            ] {
                let mut result = BenchmarkResult::new(
                    format!("Text Compression - {:?}", compression_type),
                    BenchmarkCategory::Compression,
                );
                
                // Compression speed benchmark
                let start = Instant::now();
                for _ in 0..self.config.measurement_iterations {
                    let _ = compress(data_bytes, *compression_type)?;
                }
                let compress_time = start.elapsed().as_nanos() as f64 / self.config.measurement_iterations as f64;
                result.add_measurement(Measurement::new(compress_time, MeasurementUnit::Nanoseconds));
                
                // Compression ratio benchmark
                let compressed = compress(data_bytes, *compression_type)?;
                let ratio = compressed.len() as f64 / data_bytes.len() as f64;
                result.add_measurement(Measurement::new(ratio, MeasurementUnit::Ratio)
                    .with_metadata("metric".to_string(), "compression_ratio".to_string()));
                
                // Decompression speed benchmark
                let start = Instant::now();
                for _ in 0..self.config.measurement_iterations {
                    let _ = decompress(&compressed)?;
                }
                let decompress_time = start.elapsed().as_nanos() as f64 / self.config.measurement_iterations as f64;
                result.add_measurement(Measurement::new(decompress_time, MeasurementUnit::Nanoseconds)
                    .with_metadata("operation".to_string(), "decompression".to_string()));
                
                results.push(result);
            }
            
            Ok(results)
        }
        #[cfg(not(feature = "compression"))]
        Ok(vec![])
    }
    
    async fn benchmark_binary_compression(&self) -> Result<Vec<BenchmarkResult>> {
        #[cfg(feature = "compression")]
        {
            use generals_compression::{compress, CompressionType};
            
            // Generate pseudo-random binary data
            let mut test_data = vec![0u8; 65536];
            for (i, byte) in test_data.iter_mut().enumerate() {
                *byte = ((i * 17 + 42) % 256) as u8;
            }
            
            let mut result = BenchmarkResult::new(
                "Binary Data Compression".to_string(),
                BenchmarkCategory::Compression,
            );
            
            // Test auto-compression algorithm selection
            let start = Instant::now();
            let compressed = generals_compression::compress_auto(&test_data)?;
            let duration = start.elapsed();
            
            result.add_measurement(Measurement::new(duration.as_nanos() as f64, MeasurementUnit::Nanoseconds));
            result.add_measurement(Measurement::new(
                compressed.len() as f64 / test_data.len() as f64,
                MeasurementUnit::Ratio,
            ).with_metadata("metric".to_string(), "auto_compression_ratio".to_string()));
            
            Ok(vec![result])
        }
        #[cfg(not(feature = "compression"))]
        Ok(vec![])
    }
    
    async fn benchmark_repetitive_data_compression(&self) -> Result<Vec<BenchmarkResult>> {
        #[cfg(feature = "compression")]
        {
            use generals_compression::{compress, CompressionType};
            
            // Highly repetitive data should compress very well
            let test_data = vec![0xAA; 32768];
            
            let mut result = BenchmarkResult::new(
                "Repetitive Data Compression".to_string(),
                BenchmarkCategory::Compression,
            );
            
            let start = Instant::now();
            let compressed = compress(&test_data, CompressionType::BTree)?;
            let duration = start.elapsed();
            
            result.add_measurement(Measurement::new(duration.as_nanos() as f64, MeasurementUnit::Nanoseconds));
            result.add_measurement(Measurement::new(
                compressed.len() as f64 / test_data.len() as f64,
                MeasurementUnit::Ratio,
            ).with_metadata("data_type".to_string(), "repetitive".to_string()));
            
            Ok(vec![result])
        }
        #[cfg(not(feature = "compression"))]
        Ok(vec![])
    }
    
    async fn benchmark_auto_compression(&self) -> Result<Vec<BenchmarkResult>> {
        #[cfg(feature = "compression")]
        {
            let mut result = BenchmarkResult::new(
                "Auto Compression Algorithm Selection".to_string(),
                BenchmarkCategory::Compression,
            );
            
            // Test the automatic compression algorithm selection
            let test_cases = vec![
                ("text", "Hello World! ".repeat(1000).into_bytes()),
                ("binary", (0..4096).map(|i| (i % 256) as u8).collect()),
                ("repetitive", vec![0x42; 8192]),
            ];
            
            for (name, data) in test_cases {
                let start = Instant::now();
                let compressed = generals_compression::compress_auto(&data)?;
                let duration = start.elapsed();
                
                result.add_measurement(Measurement::new(duration.as_nanos() as f64, MeasurementUnit::Nanoseconds)
                    .with_metadata("data_type".to_string(), name.to_string()));
                result.add_measurement(Measurement::new(
                    compressed.len() as f64 / data.len() as f64,
                    MeasurementUnit::Ratio,
                ).with_metadata("data_type".to_string(), format!("{}_ratio", name)));
            }
            
            Ok(vec![result])
        }
        #[cfg(not(feature = "compression"))]
        Ok(vec![])
    }
}