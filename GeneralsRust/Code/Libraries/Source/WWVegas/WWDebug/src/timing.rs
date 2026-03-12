//! Timing Module
//!
//! Provides high-precision timing functionality equivalent to the C++ ProfileGetTime
//! and CPU frequency measurement functions.

use crate::{ProfileError, ProfileResult};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// High-precision timer for profiling
pub struct ProfileTimer;

impl ProfileTimer {
    /// Get CPU cycle count (equivalent to RDTSC instruction)
    ///
    /// On modern systems where direct RDTSC access may not be available,
    /// we use high-precision system timers and convert to cycle equivalents.
    #[cfg(target_arch = "x86_64")]
    pub fn get_cpu_cycles() -> ProfileResult<u64> {
        use std::arch::x86_64::_rdtsc;

        unsafe { Ok(_rdtsc()) }
    }

    /// Get CPU cycle count using system timer (fallback for non-x86_64 or when RDTSC unavailable)
    #[cfg(not(target_arch = "x86_64"))]
    pub fn get_cpu_cycles() -> ProfileResult<u64> {
        // Use a process-relative monotonic clock so successive reads increase.
        static START: OnceLock<Instant> = OnceLock::new();
        let start = START.get_or_init(Instant::now);
        let nanos = start.elapsed().as_nanos() as u64;

        // Convert to cycle-equivalent based on estimated frequency
        // This is an approximation since we don't have direct cycle access
        const ESTIMATED_FREQ: u64 = 3_000_000_000; // 3 GHz estimation
        Ok((nanos * ESTIMATED_FREQ) / 1_000_000_000)
    }

    /// Measure CPU frequency by timing known operations
    /// Equivalent to the GetClockCyclesFast function in the C++ version
    pub fn measure_cpu_frequency() -> ProfileResult<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            Self::measure_cpu_frequency_x86_64()
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            Self::estimate_cpu_frequency()
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn measure_cpu_frequency_x86_64() -> ProfileResult<u64> {
        use std::arch::x86_64::_rdtsc;
        use std::thread;

        // Measure cycle count 3 times for 20 msec each, then take the 2 closest
        let mut measurements = [0u64; 3];

        for i in 0..3 {
            // Wait for end of current tick to get stable timing
            thread::sleep(Duration::from_millis(2));

            let start_time = Instant::now();
            let start_cycles = unsafe { _rdtsc() };

            // Wait 20ms
            thread::sleep(Duration::from_millis(20));

            let end_cycles = unsafe { _rdtsc() };
            let elapsed = start_time.elapsed();

            // Calculate cycles per second
            let cycle_diff = end_cycles.saturating_sub(start_cycles);
            let cycles_per_sec = (cycle_diff as f64 / elapsed.as_secs_f64()) as u64;

            measurements[i] = cycles_per_sec;
        }

        // Find the two closest measurements and average them
        let diff_01 = (measurements[1] as i64 - measurements[0] as i64).abs() as u64;
        let diff_02 = (measurements[2] as i64 - measurements[0] as i64).abs() as u64;
        let diff_12 = (measurements[2] as i64 - measurements[1] as i64).abs() as u64;

        let avg = if diff_01 < diff_02 && diff_01 < diff_12 {
            (measurements[0] + measurements[1]) / 2
        } else if diff_02 < diff_12 {
            (measurements[0] + measurements[2]) / 2
        } else {
            (measurements[1] + measurements[2]) / 2
        };

        // Round to the nearest MHz for consistency with C++ version
        Ok(((avg + 500_000) / 1_000_000) * 1_000_000)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn estimate_cpu_frequency() -> ProfileResult<u64> {
        // For non-x86_64 platforms, use CPU info if available
        // or return a reasonable default

        let freq = {
            #[cfg(target_os = "linux")]
            {
                if let Ok(freq) = Self::read_cpu_freq_from_proc() {
                    freq
                } else {
                    Self::estimate_frequency_by_timing()?
                }
            }

            #[cfg(target_os = "macos")]
            {
                if let Ok(freq) = Self::read_cpu_freq_sysctl() {
                    freq
                } else {
                    Self::estimate_frequency_by_timing()?
                }
            }

            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                Self::estimate_frequency_by_timing()?
            }
        };

        Ok(((freq + 500_000) / 1_000_000) * 1_000_000)
    }

    #[cfg(target_os = "linux")]
    fn read_cpu_freq_from_proc() -> ProfileResult<u64> {
        use std::fs;

        // Try to read from /proc/cpuinfo
        if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
            for line in contents.lines() {
                if line.starts_with("cpu MHz") {
                    if let Some(freq_str) = line.split(':').nth(1) {
                        if let Ok(freq_mhz) = freq_str.trim().parse::<f64>() {
                            return Ok((freq_mhz * 1_000_000.0) as u64);
                        }
                    }
                }
            }
        }

        // Try scaling frequency
        if let Ok(contents) =
            fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        {
            if let Ok(freq_khz) = contents.trim().parse::<u64>() {
                return Ok(freq_khz * 1000);
            }
        }

        Err(ProfileError::ClockError)
    }

    #[cfg(target_os = "macos")]
    fn read_cpu_freq_sysctl() -> ProfileResult<u64> {
        use std::process::Command;

        // Use sysctl to get CPU frequency
        if let Ok(output) = Command::new("sysctl")
            .args(&["-n", "hw.cpufrequency"])
            .output()
        {
            if let Ok(freq_str) = String::from_utf8(output.stdout) {
                if let Ok(freq) = freq_str.trim().parse::<u64>() {
                    return Ok(freq);
                }
            }
        }

        // Try alternative sysctl
        if let Ok(output) = Command::new("sysctl")
            .args(&["-n", "hw.cpufrequency_max"])
            .output()
        {
            if let Ok(freq_str) = String::from_utf8(output.stdout) {
                if let Ok(freq) = freq_str.trim().parse::<u64>() {
                    return Ok(freq);
                }
            }
        }

        Err(ProfileError::ClockError)
    }

    fn estimate_frequency_by_timing() -> ProfileResult<u64> {
        // Measure how many operations we can do in a known time period
        let iterations = 10_000_000u64;
        let start_time = Instant::now();

        // Perform a tight loop with some arithmetic
        let mut sum = 0u64;
        for i in 0..iterations {
            sum = sum.wrapping_add(i).wrapping_mul(3);
        }

        let elapsed = start_time.elapsed();

        // Estimate cycles based on typical instructions per operation
        // This is a very rough approximation
        let estimated_instructions_per_iter = 4; // add, mul, increment, compare
        let total_instructions = iterations * estimated_instructions_per_iter;

        // Assume 1 instruction per cycle (rough average)
        let estimated_freq = (total_instructions as f64 / elapsed.as_secs_f64()) as u64;

        // Clamp to reasonable bounds (100 MHz to 10 GHz)
        let freq = estimated_freq.max(100_000_000).min(10_000_000_000);

        // Keep the sum to prevent optimization
        if sum == 0 {
            return Err(ProfileError::ClockError);
        }

        Ok(freq)
    }

    /// Convert CPU cycles to nanoseconds
    pub fn cycles_to_nanoseconds(cycles: u64, freq: u64) -> f64 {
        if freq == 0 {
            return 0.0;
        }

        (cycles as f64 * 1_000_000_000.0) / freq as f64
    }

    /// Convert CPU cycles to seconds
    pub fn cycles_to_seconds(cycles: u64, freq: u64) -> f64 {
        if freq == 0 {
            return 0.0;
        }

        cycles as f64 / freq as f64
    }

    /// Convert seconds to CPU cycles
    pub fn seconds_to_cycles(seconds: f64, freq: u64) -> u64 {
        (seconds * freq as f64) as u64
    }

    /// Get a high-resolution timestamp in nanoseconds
    pub fn get_timestamp_nanos() -> u64 {
        // Use system time for cross-platform compatibility
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Sleep for a precise number of nanoseconds (busy wait for very short durations)
    pub fn precise_sleep_nanos(nanos: u64) {
        if nanos < 1000 {
            // Less than 1 microsecond - busy wait
            let start = Self::get_timestamp_nanos();
            while Self::get_timestamp_nanos().saturating_sub(start) < nanos {
                std::hint::spin_loop();
            }
        } else {
            std::thread::sleep(Duration::from_nanos(nanos));
        }
    }
}

/// Timer scope guard for automatic timing of code blocks
pub struct TimerScope {
    start_cycles: u64,
    result: Option<Box<dyn FnOnce(u64)>>,
}

impl TimerScope {
    /// Create a new timer scope with a callback for the result
    pub fn new<F>(callback: F) -> ProfileResult<Self>
    where
        F: FnOnce(u64) + 'static,
    {
        let start_cycles = ProfileTimer::get_cpu_cycles()?;
        Ok(Self {
            start_cycles,
            result: Some(Box::new(callback)),
        })
    }

    /// Get elapsed cycles so far without ending the timer
    pub fn elapsed_cycles(&self) -> ProfileResult<u64> {
        let current_cycles = ProfileTimer::get_cpu_cycles()?;
        Ok(current_cycles.saturating_sub(self.start_cycles))
    }

    /// Get elapsed time in nanoseconds
    pub fn elapsed_nanos(&self, freq: u64) -> ProfileResult<f64> {
        let cycles = self.elapsed_cycles()?;
        Ok(ProfileTimer::cycles_to_nanoseconds(cycles, freq))
    }
}

impl Drop for TimerScope {
    fn drop(&mut self) {
        if let Some(callback) = self.result.take() {
            if let Ok(end_cycles) = ProfileTimer::get_cpu_cycles() {
                let elapsed = end_cycles.saturating_sub(self.start_cycles);
                callback(elapsed);
            }
        }
    }
}

/// Macro for timing a code block
#[macro_export]
macro_rules! time_block {
    ($name:expr, $block:block) => {{
        let start = $crate::timing::ProfileTimer::get_cpu_cycles().unwrap_or(0);
        let result = $block;
        let end = $crate::timing::ProfileTimer::get_cpu_cycles().unwrap_or(0);
        let elapsed = end.saturating_sub(start);
        log::debug!("Block '{}' took {} cycles", $name, elapsed);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cpu_cycle_measurement() {
        let cycles1 = ProfileTimer::get_cpu_cycles().unwrap();
        thread::sleep(Duration::from_millis(1));
        let cycles2 = ProfileTimer::get_cpu_cycles().unwrap();

        // Should have some elapsed cycles
        assert!(cycles2 > cycles1);
    }

    #[test]
    fn test_cpu_frequency_measurement() {
        let freq = ProfileTimer::measure_cpu_frequency().unwrap();

        // Should be a reasonable frequency (at least 100 MHz, at most 10 GHz)
        assert!(freq >= 100_000_000);
        assert!(freq <= 10_000_000_000);

        // Should be rounded to MHz boundary
        assert_eq!(freq % 1_000_000, 0);
    }

    #[test]
    fn test_cycle_conversions() {
        let freq = 2_000_000_000u64; // 2 GHz
        let cycles = freq; // 1 second worth of cycles

        let nanos = ProfileTimer::cycles_to_nanoseconds(cycles, freq);
        assert!((nanos - 1_000_000_000.0).abs() < 1.0); // Should be ~1 second

        let seconds = ProfileTimer::cycles_to_seconds(cycles, freq);
        assert!((seconds - 1.0).abs() < 0.001); // Should be ~1 second

        let back_to_cycles = ProfileTimer::seconds_to_cycles(1.0, freq);
        assert_eq!(back_to_cycles, freq);
    }

    #[test]
    fn test_timer_scope() {
        use std::sync::{Arc, Mutex};

        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        {
            let _timer = TimerScope::new(move |cycles| {
                *result_clone.lock().unwrap() = Some(cycles);
            })
            .unwrap();

            thread::sleep(Duration::from_millis(10));
        }

        let cycles = result.lock().unwrap().unwrap();
        assert!(cycles > 0);
    }

    #[test]
    fn test_precise_sleep() {
        let start = ProfileTimer::get_timestamp_nanos();
        ProfileTimer::precise_sleep_nanos(1_000_000); // 1ms
        let end = ProfileTimer::get_timestamp_nanos();

        let elapsed_ms = (end - start) / 1_000_000;
        // Should be roughly 1ms, allow some tolerance
        assert!(elapsed_ms >= 1 && elapsed_ms <= 5);
    }

    #[test]
    fn test_time_block_macro() {
        let result = time_block!("test", {
            thread::sleep(Duration::from_millis(1));
            42
        });

        assert_eq!(result, 42);
    }

    #[test]
    fn test_timer_scope_elapsed() {
        let timer = TimerScope::new(|_| {}).unwrap();
        thread::sleep(Duration::from_millis(1));

        let elapsed = timer.elapsed_cycles().unwrap();
        assert!(elapsed > 0);

        let freq = 1_000_000_000u64; // 1 GHz for testing
        let elapsed_nanos = timer.elapsed_nanos(freq).unwrap();
        assert!(elapsed_nanos > 0.0);
    }
}
