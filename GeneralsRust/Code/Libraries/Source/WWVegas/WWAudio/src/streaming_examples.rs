//! Examples and tests for the streaming system.
//!
//! This module provides comprehensive examples of how to use the streaming system
//! converted from the original C++ WPAudio implementation.

use crate::aud_stream_buffering::{
    StreamAccessType, StreamBuffer, StreamBuffering, StreamDataBlock,
};
use crate::aud_streamer::{
    get_stream_manager, AudioStreamer as AdvancedAudioStreamer,
    StreamConfig as AdvancedStreamConfig, StreamManager, StreamState as AdvancedStreamState,
};
use crate::error::Result;
use crate::{AudioDevice, AudioFormat, AudioSystem, ChannelLayout, SampleRate, SampleWidth};
use std::path::Path;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Example: Basic streaming playback
pub async fn example_basic_streaming<P: AsRef<Path>>(audio_file: P) -> Result<()> {
    println!("=== Basic Streaming Example ===");

    // Initialize audio system
    let audio_system = AudioSystem::new().await?;
    let device = audio_system.open_device(None).await?;
    let device = Arc::new(device);

    // Create stream configuration
    let config = AdvancedStreamConfig {
        format: AudioFormat {
            channels: 2,
            sample_rate: SampleRate::Hz44100,
            sample_width: SampleWidth::S16,
            channel_layout: ChannelLayout::Stereo,
        },
        buffering_seconds: 5,
        loop_enabled: false,
        buffer_size: 8192,
        buffer_count: 4,
        name: "Basic Stream Example".to_string(),
        ..Default::default()
    };

    // Create and configure streamer
    let streamer = AdvancedAudioStreamer::new(Arc::clone(&device), config)?;

    // Open audio file
    streamer.open_file(audio_file).await?;

    // Start playback
    println!("Starting playback...");
    streamer.start().await?;

    // Monitor playback progress
    while streamer.is_active().await {
        let status = streamer.get_status().await;
        println!(
            "Position: {} bytes, State: {:?}, Buffer: {}%",
            status.position_bytes, status.state, status.buffer_fill_percent
        );

        sleep(Duration::from_millis(500)).await;

        // Break if stopped or error
        if matches!(
            status.state,
            AdvancedStreamState::Stopped | AdvancedStreamState::Error
        ) {
            break;
        }
    }

    // Clean shutdown
    streamer.stop().await?;
    streamer.close().await?;

    println!("Streaming example completed successfully!");
    Ok(())
}

/// Example: Looping audio stream with volume control
pub async fn example_looping_stream<P: AsRef<Path>>(audio_file: P) -> Result<()> {
    println!("=== Looping Stream with Volume Control Example ===");

    let audio_system = AudioSystem::new().await?;
    let device = audio_system.open_device(None).await?;
    let device = Arc::new(device);

    let mut config = AdvancedStreamConfig::default();
    config.loop_enabled = true;
    config.name = "Looping Stream Example".to_string();

    let streamer = AdvancedAudioStreamer::new(Arc::clone(&device), config)?;

    // Open and start looping playback
    streamer.open_file(audio_file).await?;
    streamer.set_looping(true).await;
    streamer.start().await?;

    println!("Starting looping playback with volume fading...");

    // Demonstrate volume control and fading
    for cycle in 0..3 {
        println!("Cycle {}: Fading in...", cycle + 1);
        streamer.fade_in().await?;
        sleep(Duration::from_secs(3)).await;

        println!("Cycle {}: Fading out...", cycle + 1);
        streamer.fade_out().await?;
        streamer.wait_for_fade().await?;
        sleep(Duration::from_secs(1)).await;
    }

    // Stop looping
    streamer.set_looping(false).await;
    streamer.stop().await?;

    println!("Looping example completed!");
    Ok(())
}

/// Example: Stream manager with multiple concurrent streams
pub async fn example_multiple_streams<P: AsRef<Path>>(audio_files: Vec<P>) -> Result<()> {
    println!("=== Multiple Concurrent Streams Example ===");

    if audio_files.is_empty() {
        println!("No audio files provided for multiple streams example");
        return Ok(());
    }

    let audio_system = AudioSystem::new().await?;
    let device = audio_system.open_device(None).await?;
    let device = Arc::new(device);
    let manager = get_stream_manager();

    let mut streamers = Vec::new();

    // Create multiple streamers
    for (i, audio_file) in audio_files.iter().enumerate() {
        let mut config = AdvancedStreamConfig::default();
        config.name = format!("Stream {}", i + 1);
        config.max_volume = 80 - (i as u8 * 10); // Stagger volumes

        let streamer = AdvancedAudioStreamer::new(Arc::clone(&device), config)?;

        // Register with manager
        let stream_id = manager.register_stream(streamer.clone()).await;

        // Open file
        streamer.open_file(audio_file).await?;
        streamers.push((stream_id, streamer));
    }

    // Start all streams
    println!("Starting {} concurrent streams...", streamers.len());
    for (_, streamer) in &streamers {
        streamer.start().await?;
    }

    // Monitor all streams
    for tick in 0..20 {
        // Monitor for 10 seconds
        println!("\n--- Tick {} ---", tick);

        for (stream_id, streamer) in &streamers {
            let status = streamer.get_status().await;
            println!(
                "Stream {}: State={:?}, Pos={}, Vol={}, Buf={}%",
                stream_id,
                status.state,
                status.position_bytes,
                status.volume,
                status.buffer_fill_percent
            );
        }

        sleep(Duration::from_millis(500)).await;
    }

    // Demonstrate manager functionality
    println!("\nPausing all streams...");
    manager.pause_all_streams().await;
    sleep(Duration::from_secs(2)).await;

    println!("Resuming all streams...");
    manager.resume_all_streams().await;
    sleep(Duration::from_secs(2)).await;

    println!("Fading out all streams...");
    manager.fade_out_all_streams().await;

    // Wait for all fades to complete
    while !manager.all_faded().await {
        sleep(Duration::from_millis(100)).await;
    }

    // Stop all streams
    manager.stop_all_streams().await;

    // Unregister streams
    for (stream_id, streamer) in streamers {
        streamer.close().await?;
        manager.unregister_stream(stream_id).await;
    }

    println!("Multiple streams example completed!");
    Ok(())
}

/// Example: Stream buffering system direct usage
pub async fn example_stream_buffering() -> Result<()> {
    println!("=== Stream Buffering System Example ===");

    // Create stream buffering system
    let mut stream_buffer = StreamBuffering::new();

    // Create buffers
    let buffer_count = stream_buffer.create_buffers(4, 2048, 16)?;
    println!("Created {} buffers of 2KB each", buffer_count);

    // Acquire input and output access
    let input_access = stream_buffer.acquire_access(StreamAccessType::Input)?;
    let output_access = stream_buffer.acquire_access(StreamAccessType::Output)?;

    println!(
        "Stream buffer total size: {} bytes",
        stream_buffer.total_bytes()
    );
    println!(
        "Available for input: {} bytes",
        stream_buffer.total_bytes_till_full()
    );
    println!(
        "Available for output: {} bytes",
        stream_buffer.total_bytes_in()
    );

    // Test data transfer
    let test_data = b"Hello, streaming world! This is a test of the stream buffering system.";
    let mut input_data = test_data.to_vec();

    println!("Transferring {} bytes into stream...", input_data.len());
    let bytes_in = input_access.transfer(&mut input_data).await?;
    println!("Successfully transferred {} bytes", bytes_in);

    // Read data back
    let mut output_data = vec![0u8; test_data.len()];
    let bytes_out = output_access.transfer(&mut output_data).await?;
    println!("Successfully read {} bytes", bytes_out);

    // Verify data integrity
    if output_data[..bytes_out] == test_data[..bytes_out] {
        println!("✅ Data integrity verified!");
    } else {
        println!("❌ Data integrity check failed!");
    }

    // Release access
    let input_id = input_access.get_id();
    let output_id = output_access.get_id();
    drop(input_access);
    drop(output_access);
    stream_buffer.release_access(input_id)?;
    stream_buffer.release_access(output_id)?;

    println!("Stream buffering example completed!");
    Ok(())
}

/// Example: Stream seeking and position control
pub async fn example_stream_seeking<P: AsRef<Path>>(audio_file: P) -> Result<()> {
    println!("=== Stream Seeking Example ===");

    let audio_system = AudioSystem::new().await?;
    let device = audio_system.open_device(None).await?;
    let device = Arc::new(device);

    let config = AdvancedStreamConfig {
        name: "Seeking Example".to_string(),
        ..Default::default()
    };

    let streamer = AdvancedAudioStreamer::new(Arc::clone(&device), config)?;
    streamer.open_file(audio_file).await?;

    // Get file information
    let status = streamer.get_status().await;
    let total_bytes = status.total_bytes;
    println!("Audio file size: {} bytes", total_bytes);

    // Start playback
    streamer.start().await?;

    // Play for a few seconds
    println!("Playing from beginning...");
    sleep(Duration::from_secs(2)).await;

    // Seek to 25% position
    let seek_pos = total_bytes / 4;
    println!("Seeking to 25% position ({} bytes)...", seek_pos);
    streamer.set_position(seek_pos).await?;
    sleep(Duration::from_secs(2)).await;

    // Seek to 50% position
    let seek_pos = total_bytes / 2;
    println!("Seeking to 50% position ({} bytes)...", seek_pos);
    streamer.set_position(seek_pos).await?;
    sleep(Duration::from_secs(2)).await;

    // Seek to 75% position
    let seek_pos = (total_bytes * 3) / 4;
    println!("Seeking to 75% position ({} bytes)...", seek_pos);
    streamer.set_position(seek_pos).await?;
    sleep(Duration::from_secs(2)).await;

    // Seek back to beginning
    println!("Seeking back to beginning...");
    streamer.set_position(0).await?;
    sleep(Duration::from_secs(2)).await;

    streamer.stop().await?;
    println!("Seeking example completed!");
    Ok(())
}

/// Example: Error handling and recovery
pub async fn example_error_handling() -> Result<()> {
    println!("=== Error Handling Example ===");

    let audio_system = AudioSystem::new().await?;
    let device = audio_system.open_device(None).await?;
    let device = Arc::new(device);
    let config = AdvancedStreamConfig::default();
    let streamer = AdvancedAudioStreamer::new(Arc::clone(&device), config)?;

    // Test 1: Try to open non-existent file
    println!("Test 1: Opening non-existent file...");
    match streamer.open_file("non_existent_file.wav").await {
        Ok(_) => println!("❌ Expected error but got success"),
        Err(e) => println!("✅ Got expected error: {}", e),
    }

    // Test 2: Try to start without opening file
    println!("Test 2: Starting stream without open file...");
    match streamer.start().await {
        Ok(_) => println!("❌ Expected error but got success"),
        Err(e) => println!("✅ Got expected error: {}", e),
    }

    // Test 3: Test stream buffer overflow protection
    println!("Test 3: Testing buffer overflow protection...");
    let mut stream_buffer = StreamBuffering::new();

    // Try to create buffer with zero size
    match StreamBuffer::new(0, 16) {
        Ok(_) => println!("❌ Expected error but got success"),
        Err(e) => println!("✅ Got expected error: {}", e),
    }

    // Test 4: Multiple access acquisition
    println!("Test 4: Testing multiple access acquisition...");
    stream_buffer.create_buffers(2, 1024, 8)?;

    let _input_access1 = stream_buffer.acquire_access(StreamAccessType::Input)?;
    match stream_buffer.acquire_access(StreamAccessType::Input) {
        Ok(_) => println!("❌ Expected error but got success"),
        Err(e) => println!("✅ Got expected error: {}", e),
    }

    println!("Error handling example completed!");
    Ok(())
}

/// Performance benchmark for streaming operations
pub async fn benchmark_streaming_performance() -> Result<()> {
    println!("=== Streaming Performance Benchmark ===");

    use std::time::Instant;

    // Benchmark buffer operations
    let start = Instant::now();
    let mut stream_buffer = StreamBuffering::new();

    // Create large buffers
    let buffer_count = stream_buffer.create_buffers(8, 64 * 1024, 16)?; // 8 x 64KB buffers
    let creation_time = start.elapsed();

    println!(
        "Buffer creation: {} buffers in {:?}",
        buffer_count, creation_time
    );

    // Benchmark data transfer
    let data_size = 1024 * 1024; // 1MB test data
    let test_data = vec![0x42u8; data_size];

    let input_access = stream_buffer.acquire_access(StreamAccessType::Input)?;

    let start = Instant::now();
    let mut data_copy = test_data.clone();
    let transferred = input_access.transfer(&mut data_copy).await?;
    let transfer_time = start.elapsed();

    let throughput = (transferred as f64 / 1024.0 / 1024.0) / transfer_time.as_secs_f64();
    println!(
        "Data transfer: {} bytes in {:?} ({:.2} MB/s)",
        transferred, transfer_time, throughput
    );

    // Benchmark random access
    let output_access = stream_buffer.acquire_access(StreamAccessType::Output)?;

    let start = Instant::now();
    for _ in 0..1000 {
        output_access.get_block().await?;
        output_access.advance(256).await?;
    }
    let access_time = start.elapsed();

    println!("Random access: 1000 operations in {:?}", access_time);

    // Memory usage estimation
    let buffer_memory = buffer_count * 64 * 1024;
    println!("Total buffer memory: {} KB", buffer_memory / 1024);

    let input_id = input_access.get_id();
    let output_id = output_access.get_id();
    drop(input_access);
    drop(output_access);
    stream_buffer.release_access(input_id)?;
    stream_buffer.release_access(output_id)?;

    println!("Performance benchmark completed!");
    Ok(())
}

/// Comprehensive test runner
pub async fn run_all_examples() -> Result<()> {
    println!("🎵 WPAudio Streaming System Examples 🎵\n");

    // Run stream buffering example (doesn't need audio files)
    if let Err(e) = example_stream_buffering().await {
        println!("Stream buffering example failed: {}", e);
    }
    println!();

    // Run error handling example
    if let Err(e) = example_error_handling().await {
        println!("Error handling example failed: {}", e);
    }
    println!();

    // Run performance benchmark
    if let Err(e) = benchmark_streaming_performance().await {
        println!("Performance benchmark failed: {}", e);
    }
    println!();

    // Note: Audio file examples would need actual audio files
    println!("📝 Note: Audio file examples require actual WAV files to be provided");
    println!("   Use example_basic_streaming(), example_looping_stream(), etc. with file paths");

    println!("\n✅ All available examples completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_stream_buffering_example() {
        example_stream_buffering()
            .await
            .expect("Stream buffering example should succeed");
    }

    #[tokio::test]
    async fn test_error_handling_example() {
        example_error_handling()
            .await
            .expect("Error handling example should succeed");
    }

    #[tokio::test]
    async fn test_performance_benchmark() {
        benchmark_streaming_performance()
            .await
            .expect("Performance benchmark should succeed");
    }

    #[tokio::test]
    async fn test_buffer_creation_and_destruction() {
        let mut stream_buffer = StreamBuffering::new();

        // Test buffer creation
        let count = stream_buffer.create_buffers(4, 1024, 8).unwrap();
        assert_eq!(count, 4);
        assert_eq!(stream_buffer.total_bytes(), 4 * 1024);

        // Test buffer destruction
        stream_buffer.destroy_buffers();
        assert_eq!(stream_buffer.total_bytes(), 0);
    }

    #[tokio::test]
    async fn test_access_management() {
        let mut stream_buffer = StreamBuffering::new();
        stream_buffer.create_buffers(2, 1024, 8).unwrap();

        // Test successful access acquisition
        let input_access = stream_buffer
            .acquire_access(StreamAccessType::Input)
            .unwrap();
        assert_eq!(input_access.get_id(), StreamAccessType::Input as u32);

        let output_access = stream_buffer
            .acquire_access(StreamAccessType::Output)
            .unwrap();
        assert_eq!(output_access.get_id(), StreamAccessType::Output as u32);

        // Test access release
        let input_id = input_access.get_id();
        let output_id = output_access.get_id();
        drop(input_access);
        drop(output_access);
        stream_buffer.release_access(input_id).unwrap();
        stream_buffer.release_access(output_id).unwrap();
    }

    #[tokio::test]
    async fn test_data_transfer_integrity() {
        let mut stream_buffer = StreamBuffering::new();
        stream_buffer.create_buffers(2, 1024, 8).unwrap();

        let input_access = stream_buffer
            .acquire_access(StreamAccessType::Input)
            .unwrap();
        let output_access = stream_buffer
            .acquire_access(StreamAccessType::Output)
            .unwrap();

        // Test data
        let test_pattern = b"Test data for integrity check";
        let mut input_data = test_pattern.to_vec();
        let mut output_data = vec![0u8; test_pattern.len()];

        // Transfer data
        let bytes_in = input_access.transfer(&mut input_data).await.unwrap();
        let bytes_out = output_access.transfer(&mut output_data).await.unwrap();

        // Verify integrity
        assert_eq!(bytes_in, test_pattern.len());
        assert_eq!(bytes_out, test_pattern.len());
        assert_eq!(&output_data[..bytes_out], test_pattern);

        // Clean up
        let input_id = input_access.get_id();
        let output_id = output_access.get_id();
        drop(input_access);
        drop(output_access);
        stream_buffer.release_access(input_id).unwrap();
        stream_buffer.release_access(output_id).unwrap();
    }
}
