//! Game statistics collector for tracking gameplay and performance metrics.

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Game statistic types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatType {
    // Unit stats
    UnitsBuilt,
    UnitsDestroyed,
    UnitsLost,
    UnitsKilled,
    UnitsHealed,
    UnitsPromoted,

    // Building stats
    BuildingsConstructed,
    BuildingsDestroyed,
    BuildingsLost,
    BuildingsCaptured,

    // Economy stats
    ResourcesGathered,
    ResourcesSpent,
    SuppliesCollected,

    // Combat stats
    BattlesWon,
    BattlesLost,
    DamageDealt,
    DamageTaken,
    ShotsHired,
    ShotsHit,

    // Special abilities
    SpecialPowersUsed,
    UpgradesCompleted,

    // Multiplayer stats
    GamesPlayed,
    GamesWon,
    GamesLost,

    // Performance stats
    AverageFrameTime,
    PeakFrameTime,
    TotalGameTime,
}

/// Per-player statistics
#[derive(Debug, Clone)]
pub struct PlayerStats {
    pub player_id: u32,
    pub stats: HashMap<StatType, u64>,
}

impl PlayerStats {
    pub fn new(player_id: u32) -> Self {
        Self {
            player_id,
            stats: HashMap::new(),
        }
    }

    pub fn increment(&mut self, stat_type: StatType, amount: u64) {
        *self.stats.entry(stat_type).or_insert(0) += amount;
    }

    pub fn get(&self, stat_type: &StatType) -> u64 {
        self.stats.get(stat_type).copied().unwrap_or(0)
    }
}

/// Rolling average tracker for performance metrics
#[derive(Debug, Clone)]
pub struct RollingAverage {
    values: Vec<f64>,
    max_samples: usize,
    sum: f64,
}

impl RollingAverage {
    pub fn new(max_samples: usize) -> Self {
        Self {
            values: Vec::with_capacity(max_samples),
            max_samples,
            sum: 0.0,
        }
    }

    pub fn add(&mut self, value: f64) {
        if self.values.len() >= self.max_samples {
            let removed = self.values.remove(0);
            self.sum -= removed;
        }
        self.values.push(value);
        self.sum += value;
    }

    pub fn average(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f64
        }
    }

    pub fn min(&self) -> f64 {
        self.values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

/// Statistics collector
#[derive(Debug, Clone)]
pub struct StatsCollector {
    // Global game stats
    stats: HashMap<StatType, u64>,

    // Per-player stats
    player_stats: HashMap<u32, PlayerStats>,

    // Timing information
    start_time: Instant,
    last_update: Instant,
    frame_count: u64,

    // Performance metrics
    frame_times: RollingAverage,
    update_times: RollingAverage,

    // Session tracking
    session_id: String,
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsCollector {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            stats: HashMap::new(),
            player_stats: HashMap::new(),
            start_time: now,
            last_update: now,
            frame_count: 0,
            frame_times: RollingAverage::new(60),
            update_times: RollingAverage::new(60),
            session_id: format!("{:?}", now),
        }
    }

    // Global stats methods
    pub fn increment_stat(&mut self, stat_type: StatType, amount: u64) {
        *self.stats.entry(stat_type).or_insert(0) += amount;
    }

    pub fn set_stat(&mut self, stat_type: StatType, value: u64) {
        self.stats.insert(stat_type, value);
    }

    pub fn get_stat(&self, stat_type: &StatType) -> u64 {
        self.stats.get(stat_type).copied().unwrap_or(0)
    }

    pub fn get_all_stats(&self) -> &HashMap<StatType, u64> {
        &self.stats
    }

    // Player stats methods
    pub fn register_player(&mut self, player_id: u32) {
        self.player_stats
            .insert(player_id, PlayerStats::new(player_id));
    }

    pub fn increment_player_stat(&mut self, player_id: u32, stat_type: StatType, amount: u64) {
        if let Some(player) = self.player_stats.get_mut(&player_id) {
            player.increment(stat_type, amount);
        }
    }

    pub fn get_player_stat(&self, player_id: u32, stat_type: &StatType) -> u64 {
        self.player_stats
            .get(&player_id)
            .map(|p| p.get(stat_type))
            .unwrap_or(0)
    }

    pub fn get_player_stats(&self, player_id: u32) -> Option<&PlayerStats> {
        self.player_stats.get(&player_id)
    }

    pub fn get_all_player_stats(&self) -> &HashMap<u32, PlayerStats> {
        &self.player_stats
    }

    // Timing methods
    pub fn get_elapsed_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn get_average_frame_time(&self) -> f64 {
        self.frame_times.average()
    }

    pub fn get_average_fps(&self) -> f64 {
        let avg_frame_time = self.frame_times.average();
        if avg_frame_time > 0.0 {
            1000.0 / avg_frame_time
        } else {
            0.0
        }
    }

    pub fn get_min_frame_time(&self) -> f64 {
        self.frame_times.min()
    }

    pub fn get_max_frame_time(&self) -> f64 {
        self.frame_times.max()
    }

    // Session methods
    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }

    pub fn reset_stats(&mut self) {
        self.stats.clear();
        self.player_stats.clear();
        let now = Instant::now();
        self.start_time = now;
        self.last_update = now;
        self.frame_count = 0;
        self.frame_times = RollingAverage::new(60);
        self.update_times = RollingAverage::new(60);
        self.session_id = format!("{:?}", now);
    }

    /// Called once per frame to update rolling statistics
    pub fn update(&mut self) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_update).as_secs_f64() * 1000.0;

        self.frame_times.add(frame_time);
        self.frame_count += 1;
        self.last_update = now;

        // Update global stats
        self.set_stat(StatType::TotalGameTime, self.get_elapsed_time().as_secs());
    }

    /// Record frame timing for performance tracking
    pub fn record_frame_time(&mut self, frame_time_ms: f64) {
        self.frame_times.add(frame_time_ms);
    }

    /// Record update timing for performance tracking
    pub fn record_update_time(&mut self, update_time_ms: f64) {
        self.update_times.add(update_time_ms);
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("Game Statistics Report\n");
        report.push_str("======================\n\n");

        report.push_str(&format!("Session ID: {}\n", self.session_id));
        report.push_str(&format!("Total Game Time: {:?}\n", self.get_elapsed_time()));
        report.push_str(&format!("Total Frames: {}\n", self.frame_count));
        report.push_str(&format!("Average FPS: {:.2}\n", self.get_average_fps()));
        report.push_str(&format!(
            "Frame Time: avg={:.2}ms, min={:.2}ms, max={:.2}ms\n\n",
            self.get_average_frame_time(),
            self.get_min_frame_time(),
            self.get_max_frame_time()
        ));

        report.push_str("Global Statistics:\n");
        report.push_str("------------------\n");
        for (stat_type, value) in &self.stats {
            report.push_str(&format!("  {:?}: {}\n", stat_type, value));
        }
        report.push('\n');

        if !self.player_stats.is_empty() {
            report.push_str("Player Statistics:\n");
            report.push_str("------------------\n");
            for (player_id, player) in &self.player_stats {
                report.push_str(&format!("Player {}:\n", player_id));
                for (stat_type, value) in &player.stats {
                    report.push_str(&format!("  {:?}: {}\n", stat_type, value));
                }
                report.push('\n');
            }
        }

        report
    }

    /// Generate a summary report with key metrics
    pub fn generate_summary(&self) -> String {
        format!(
            "Session: {} | Time: {:?} | Frames: {} | FPS: {:.1} | Frame Time: {:.1}ms",
            self.session_id,
            self.get_elapsed_time(),
            self.frame_count,
            self.get_average_fps(),
            self.get_average_frame_time()
        )
    }
}

static GLOBAL_STATS_COLLECTOR: OnceCell<Mutex<StatsCollector>> = OnceCell::new();

/// Initialise and obtain the global stats collector.
pub fn init_stats_collector() -> &'static Mutex<StatsCollector> {
    GLOBAL_STATS_COLLECTOR.get_or_init(|| Mutex::new(StatsCollector::new()))
}

/// Execute a closure with a mutable reference to the global stats collector.
pub fn with_stats_collector_mut<R>(f: impl FnOnce(&mut StatsCollector) -> R) -> Option<R> {
    GLOBAL_STATS_COLLECTOR
        .get()
        .and_then(|collector| collector.lock().ok().map(|mut guard| f(&mut *guard)))
}
