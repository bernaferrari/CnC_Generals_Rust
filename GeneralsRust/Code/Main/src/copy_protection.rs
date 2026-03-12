////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: copy_protection.rs
//
// Copy protection and launcher integration system
// Provides stubs matching the C++ CopyProtect class implementation
// Designed to be extensible with actual protection mechanisms later
//
// Author: Generated from C++ WinMain.cpp patterns
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Development mode flag - when true, all copy protection is bypassed
static DEVELOPMENT_MODE: AtomicBool = AtomicBool::new(true);

/// Copy protection enabled flag - can be configured at runtime
static COPY_PROTECTION_ENABLED: AtomicBool = AtomicBool::new(false);

/// Launcher communication timeout in seconds
const _LAUNCHER_TIMEOUT_SECONDS: u64 = 30;

/// Maximum launcher message size
const _MAX_LAUNCHER_MESSAGE_SIZE: usize = 1024;

const _MAX_LOCAL_LAUNCHER_MESSAGE_QUEUE: usize = 64;
const _DEFAULT_HEARTBEAT_STALE_SECONDS: u64 = 60;

fn env_string(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

fn parse_message_file(path: &str) -> Option<LauncherMessage> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, ':');
        let kind = parts.next()?.trim();

        match kind {
            "Heartbeat" => {
                let process_id = parts
                    .next()
                    .and_then(|pid| pid.trim().parse::<u32>().ok())
                    .unwrap_or_else(std::process::id);

                if let Ok(()) = fs::write(path, "") {
                    debug!("Consumed launcher message file {}", path);
                }

                return Some(LauncherMessage::Heartbeat { process_id });
            }
            "GameStart" => {
                let process_id = parts
                    .next()
                    .and_then(|pid| pid.trim().parse::<u32>().ok())
                    .unwrap_or_else(std::process::id);

                let timestamp = parts
                    .next()
                    .and_then(|ts| ts.trim().parse::<u64>().ok())
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0, |t| t.as_secs())
                    });

                if let Ok(()) = fs::write(path, "") {
                    debug!("Consumed launcher message file {}", path);
                }

                return Some(LauncherMessage::GameStart {
                    process_id,
                    timestamp,
                });
            }
            "GameShutdown" => {
                let process_id = parts
                    .next()
                    .and_then(|pid| pid.trim().parse::<u32>().ok())
                    .unwrap_or_else(std::process::id);

                let timestamp = parts
                    .next()
                    .and_then(|ts| ts.trim().parse::<u64>().ok())
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0, |t| t.as_secs())
                    });

                if let Ok(()) = fs::write(path, "") {
                    debug!("Consumed launcher message file {}", path);
                }

                return Some(LauncherMessage::GameShutdown {
                    process_id,
                    timestamp,
                });
            }
            "VersionCheck" => {
                let game_version = parts.next().unwrap_or("").trim().to_string();

                if let Ok(()) = fs::write(path, "") {
                    debug!("Consumed launcher message file {}", path);
                }

                return Some(LauncherMessage::VersionCheck { game_version });
            }
            "Custom" => {
                let message_type = parts.next().unwrap_or("").trim().to_string();
                let data = parts
                    .next()
                    .map(|payload| payload.as_bytes().to_vec())
                    .unwrap_or_default();

                if let Ok(()) = fs::write(path, "") {
                    debug!("Consumed launcher message file {}", path);
                }

                return Some(LauncherMessage::Custom { message_type, data });
            }
            _ => {
                warn!("Unknown launcher message format in {}: {}", path, line);
            }
        }
    }

    None
}

/// Copy protection status
#[derive(Debug, Clone, PartialEq)]
pub enum ProtectionStatus {
    /// Protection check passed
    Valid,
    /// Protection check failed
    Invalid,
    /// Protection check skipped (development mode)
    Bypassed,
    /// Protection check timed out
    Timeout,
    /// Protection check error
    Error(String),
}

/// Launcher communication status
#[derive(Debug, Clone, PartialEq)]
pub enum LauncherStatus {
    /// Launcher is running and responsive
    Running,
    /// Launcher is not running
    NotRunning,
    /// Launcher communication error
    Error(String),
    /// Launcher check bypassed (development mode)
    Bypassed,
}

/// Launcher message types (matching C++ implementation)
#[derive(Debug, Clone)]
pub enum LauncherMessage {
    /// Game startup notification
    GameStart { process_id: u32, timestamp: u64 },
    /// Game shutdown notification
    GameShutdown { process_id: u32, timestamp: u64 },
    /// Heartbeat message
    Heartbeat { process_id: u32 },
    /// Version verification request
    VersionCheck { game_version: String },
    /// Custom message
    Custom { message_type: String, data: Vec<u8> },
}

/// Copy protection trait - allows different implementations to be swapped
pub trait CopyProtectionProvider {
    /// Check if launcher is running
    fn is_launcher_running(&self) -> Result<LauncherStatus>;

    /// Notify launcher of game start
    fn notify_launcher(&self, message: LauncherMessage) -> Result<()>;

    /// Check for launcher messages
    fn check_for_message(&self) -> Result<Option<LauncherMessage>>;

    /// Perform copy protection validation
    fn validate_protection(&self) -> Result<ProtectionStatus>;

    /// Clean shutdown of copy protection
    fn shutdown(&self) -> Result<()>;

    /// Get provider name for logging
    fn provider_name(&self) -> &'static str;
}

/// Development copy protection provider - bypasses all checks
pub struct DevelopmentProvider {
    start_time: SystemTime,
    _process_id: u32,
}

impl DevelopmentProvider {
    pub fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            _process_id: std::process::id(),
        }
    }
}

impl CopyProtectionProvider for DevelopmentProvider {
    fn is_launcher_running(&self) -> Result<LauncherStatus> {
        debug!("Development mode: Bypassing launcher check");
        Ok(LauncherStatus::Bypassed)
    }

    fn notify_launcher(&self, message: LauncherMessage) -> Result<()> {
        debug!(
            "Development mode: Would notify launcher with message: {:?}",
            message
        );
        Ok(())
    }

    fn check_for_message(&self) -> Result<Option<LauncherMessage>> {
        debug!("Development mode: No launcher messages (bypassed)");
        Ok(None)
    }

    fn validate_protection(&self) -> Result<ProtectionStatus> {
        debug!("Development mode: Bypassing copy protection validation");
        Ok(ProtectionStatus::Bypassed)
    }

    fn shutdown(&self) -> Result<()> {
        let elapsed = self.start_time.elapsed().unwrap_or_default();
        info!("Development copy protection shutdown after {:?}", elapsed);
        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "Development"
    }
}

/// Production copy protection provider - stub for real implementation
pub struct ProductionProvider {
    start_time: SystemTime,
    process_id: u32,
    launcher_last_seen: Mutex<Option<SystemTime>>,
    message_queue: Mutex<VecDeque<LauncherMessage>>,
}

impl ProductionProvider {
    pub fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            process_id: std::process::id(),
            launcher_last_seen: Mutex::new(None),
            message_queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Check if launcher process is actually running (stub)
    fn check_launcher_process(&self) -> Result<bool> {
        if env_string("CNC_GENERALS_SKIP_LAUNCHER_CHECK").is_some() {
            return Ok(true);
        }

        // Optional heartbeat file support:
        // CNC_GENERALS_LAUNCHER_HEARTBEAT=/absolute/path/to/heartbeat
        if let Some(path) = env_string("CNC_GENERALS_LAUNCHER_HEARTBEAT") {
            let heartbeat = fs::metadata(Path::new(&path));
            if let Ok(meta) = heartbeat {
                if let Ok(modified) = meta.modified() {
                    let age = SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_else(|_| Duration::from_secs(u64::MAX));

                    if age <= Duration::from_secs(_DEFAULT_HEARTBEAT_STALE_SECONDS) {
                        if let Ok(mut last_seen) = self.launcher_last_seen.lock() {
                            *last_seen = Some(SystemTime::now());
                        }

                        info!("Production launcher heartbeat is fresh for {}", path);
                        return Ok(true);
                    }

                    warn!(
                        "Launcher heartbeat file {} is stale ({}s)",
                        path,
                        age.as_secs()
                    );
                } else {
                    warn!(
                        "Unable to read launcher heartbeat modified time from {}",
                        path
                    );
                }
            } else {
                warn!("Launcher heartbeat file {} not accessible", path);
            }
        }

        // Optional PID-only check for launch integration in non-Windows environments.
        if let Some(pid) =
            env_string("CNC_GENERALS_LAUNCHER_PID").and_then(|pid| pid.parse::<u32>().ok())
        {
            if pid != 0 {
                if let Ok(mut last_seen) = self.launcher_last_seen.lock() {
                    *last_seen = Some(SystemTime::now());
                }
                return Ok(true);
            }
        }

        warn!("Launcher process could not be confirmed; running without required launcher context");
        Ok(false)
    }

    /// Perform actual copy protection checks (stub)
    fn perform_protection_validation(&self) -> Result<ProtectionStatus> {
        if let Some(fail_code) = env_string("CNC_GENERALS_FORCE_CP_FAIL") {
            if fail_code == "1" {
                warn!("Copy protection explicitly disabled by CNC_GENERALS_FORCE_CP_FAIL");
                return Ok(ProtectionStatus::Invalid);
            }
        }

        // Validate game path if explicitly provided.
        if let Some(path) = env_string("CNC_GENERALS_GAME_PATH") {
            if path.trim().is_empty() {
                return Err(anyhow::anyhow!("Game path is empty"));
            }

            if !Path::new(&path).exists() {
                return Err(anyhow::anyhow!("Game path does not exist: {}", path));
            }

            if fs::metadata(&path).is_err() {
                return Err(anyhow::anyhow!("Game path is not accessible: {}", path));
            }
        }

        info!("Production copy protection validation completed");
        Ok(ProtectionStatus::Valid)
    }
}

impl CopyProtectionProvider for ProductionProvider {
    fn is_launcher_running(&self) -> Result<LauncherStatus> {
        match self.check_launcher_process() {
            Ok(true) => {
                info!("Launcher detected and running");
                Ok(LauncherStatus::Running)
            }
            Ok(false) => {
                warn!("Launcher not detected");
                Ok(LauncherStatus::NotRunning)
            }
            Err(e) => {
                error!("Error checking launcher status: {}", e);
                Ok(LauncherStatus::Error(e.to_string()))
            }
        }
    }

    fn notify_launcher(&self, message: LauncherMessage) -> Result<()> {
        info!(
            "Production mode: Would notify launcher with message: {:?}",
            message
        );

        if let Ok(mut queue) = self.message_queue.lock() {
            if queue.len() >= _MAX_LOCAL_LAUNCHER_MESSAGE_QUEUE {
                while queue.len() >= _MAX_LOCAL_LAUNCHER_MESSAGE_QUEUE {
                    queue.pop_front();
                }
            }

            queue.push_back(message);
        } else {
            return Err(anyhow::anyhow!("Failed to access production message queue"));
        }

        if let Ok(mut last_seen) = self.launcher_last_seen.lock() {
            *last_seen = Some(SystemTime::now());
        }

        Ok(())
    }

    fn check_for_message(&self) -> Result<Option<LauncherMessage>> {
        if let Ok(mut queue) = self.message_queue.lock() {
            if let Some(message) = queue.pop_front() {
                return Ok(Some(message));
            }
        } else {
            return Err(anyhow::anyhow!("Failed to access production message queue"));
        }

        if let Some(path) = env_string("CNC_GENERALS_MESSAGE_FILE") {
            let message = parse_message_file(&path);
            if message.is_some() {
                return Ok(message);
            }
        }

        if let Some(path) = env_string("CNC_GENERALS_LAUNCHER_MESSAGE_FILE") {
            let message = parse_message_file(&path);
            if message.is_some() {
                return Ok(message);
            }
        }

        debug!("Production mode: No launcher messages available");
        Ok(None)
    }

    fn validate_protection(&self) -> Result<ProtectionStatus> {
        self.perform_protection_validation()
    }

    fn shutdown(&self) -> Result<()> {
        let elapsed = self.start_time.elapsed();
        info!("Production copy protection shutdown after {:?}", elapsed);

        if let Ok(mut queue) = self.message_queue.lock() {
            queue.clear();
        }

        if let Ok(mut last_seen) = self.launcher_last_seen.lock() {
            *last_seen = None;
        }

        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "Production"
    }
}

/// Main copy protection manager
pub struct CopyProtection {
    provider: Box<dyn CopyProtectionProvider + Send + Sync>,
    initialized: bool,
}

impl CopyProtection {
    /// Create new copy protection instance
    pub fn new() -> Self {
        let provider: Box<dyn CopyProtectionProvider + Send + Sync> =
            if DEVELOPMENT_MODE.load(Ordering::Relaxed) {
                info!("Initializing copy protection in development mode");
                Box::new(DevelopmentProvider::new())
            } else {
                info!("Initializing copy protection in production mode");
                Box::new(ProductionProvider::new())
            };

        Self {
            provider,
            initialized: false,
        }
    }

    /// Initialize copy protection system
    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            warn!("Copy protection already initialized");
            return Ok(());
        }

        info!(
            "Initializing copy protection system with {} provider",
            self.provider.provider_name()
        );

        // Check if protection is enabled
        if !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            info!("Copy protection is disabled via configuration");
            self.initialized = true;
            return Ok(());
        }

        // Perform initial validation
        let status = self.provider.validate_protection()?;
        match status {
            ProtectionStatus::Valid | ProtectionStatus::Bypassed => {
                info!("Copy protection validation successful: {:?}", status);
            }
            ProtectionStatus::Invalid => {
                error!("Copy protection validation failed");
                return Err(anyhow::anyhow!("Copy protection validation failed"));
            }
            ProtectionStatus::Timeout => {
                error!("Copy protection validation timed out");
                return Err(anyhow::anyhow!("Copy protection validation timed out"));
            }
            ProtectionStatus::Error(ref e) => {
                error!("Copy protection validation error: {}", e);
                return Err(anyhow::anyhow!("Copy protection validation error: {}", e));
            }
        }

        self.initialized = true;
        Ok(())
    }

    /// Check if launcher is running (matching C++ CopyProtect::isLauncherRunning)
    pub fn is_launcher_running(&self) -> bool {
        if !self.initialized {
            debug!(
                "Copy protection not initialized or disabled, returning false for launcher check"
            );
            return false;
        }

        if !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            debug!("Copy protection disabled, launcher check is bypassed");
            return true;
        }

        match self.provider.is_launcher_running() {
            Ok(LauncherStatus::Running) => true,
            Ok(LauncherStatus::Bypassed) => {
                debug!("Launcher check bypassed (development mode)");
                true
            }
            Ok(status) => {
                debug!("Launcher not running: {:?}", status);
                false
            }
            Err(e) => {
                error!("Error checking launcher status: {}", e);
                false
            }
        }
    }

    /// Notify launcher of game start (matching C++ CopyProtect::notifyLauncher)
    pub fn notify_launcher(&self) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Copy protection not initialized"));
        }

        if !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            debug!("Copy protection disabled, skipping launcher notification");
            return Ok(());
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let message = LauncherMessage::GameStart {
            process_id: std::process::id(),
            timestamp,
        };

        self.provider
            .notify_launcher(message)
            .context("Failed to notify launcher of game start")
    }

    /// Notify launcher with current game version for compatibility checks.
    pub fn notify_launcher_version(&self, game_version: &str) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Copy protection not initialized"));
        }

        if !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            debug!("Copy protection disabled, skipping launcher version notification");
            return Ok(());
        }

        self.provider
            .notify_launcher(LauncherMessage::VersionCheck {
                game_version: game_version.to_string(),
            })
            .context("Failed to notify launcher of game version")
    }

    /// Check for launcher messages (matching C++ CopyProtect::checkForMessage)
    pub fn check_for_message(&self) -> Result<Option<LauncherMessage>> {
        if !self.initialized || !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            return Ok(None);
        }

        self.provider
            .check_for_message()
            .context("Failed to check for launcher messages")
    }

    /// Clean shutdown (matching C++ CopyProtect::shutdown)
    pub fn shutdown(&self) -> Result<()> {
        if !self.initialized {
            debug!("Copy protection not initialized, nothing to shutdown");
            return Ok(());
        }

        info!("Shutting down copy protection system");

        // Notify launcher of shutdown
        if COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let message = LauncherMessage::GameShutdown {
                process_id: std::process::id(),
                timestamp,
            };

            if let Err(e) = self.provider.notify_launcher(message) {
                warn!("Failed to notify launcher of shutdown: {}", e);
            }
        }

        self.provider
            .shutdown()
            .context("Failed to shutdown copy protection")
    }

    /// Validate copy protection
    pub fn validate(&self) -> Result<ProtectionStatus> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Copy protection not initialized"));
        }

        if !COPY_PROTECTION_ENABLED.load(Ordering::Relaxed) {
            debug!("Copy protection disabled, returning bypassed status");
            return Ok(ProtectionStatus::Bypassed);
        }

        self.provider.validate_protection()
    }

    /// Check if copy protection is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get provider name
    pub fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }
}

/// Global copy protection instance
static COPY_PROTECTION: OnceLock<Arc<Mutex<CopyProtection>>> = OnceLock::new();

/// Cloneable handle for accessing the copy protection subsystem safely.
#[derive(Clone)]
pub struct CopyProtectionHandle {
    inner: Arc<Mutex<CopyProtection>>,
}

impl CopyProtectionHandle {
    fn new(inner: Arc<Mutex<CopyProtection>>) -> Self {
        Self { inner }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, CopyProtection> {
        self.inner.lock().expect("CopyProtection mutex poisoned")
    }
}

/// Initialize global copy protection instance
pub fn initialize_copy_protection() -> Result<()> {
    if COPY_PROTECTION.get().is_none() {
        let mut cp = CopyProtection::new();
        cp.initialize()?;

        let arc = Arc::new(Mutex::new(cp));
        COPY_PROTECTION
            .set(arc)
            .map_err(|_| anyhow::anyhow!("Copy protection already initialized"))?;
    }

    Ok(())
}

/// Get global copy protection instance
pub fn get_copy_protection() -> Option<CopyProtectionHandle> {
    COPY_PROTECTION
        .get()
        .cloned()
        .map(CopyProtectionHandle::new)
}

/// Shutdown global copy protection
pub fn shutdown_copy_protection() -> Result<()> {
    if let Some(arc) = COPY_PROTECTION.get() {
        let cp = arc
            .lock()
            .expect("CopyProtection mutex poisoned during shutdown");
        cp.shutdown()?;
    }
    Ok(())
}

/// Configure copy protection settings
pub fn configure_copy_protection(development_mode: bool, enabled: bool) {
    DEVELOPMENT_MODE.store(development_mode, Ordering::Relaxed);
    COPY_PROTECTION_ENABLED.store(enabled, Ordering::Relaxed);

    info!(
        "Copy protection configured: development_mode={}, enabled={}",
        development_mode, enabled
    );
}

/// Check if development mode is enabled
pub fn is_development_mode() -> bool {
    DEVELOPMENT_MODE.load(Ordering::Relaxed)
}

/// Check if copy protection is enabled
pub fn is_copy_protection_enabled() -> bool {
    COPY_PROTECTION_ENABLED.load(Ordering::Relaxed)
}

// Convenience functions matching C++ CopyProtect static methods

/// Check if launcher is running (C++ compatible interface)
pub fn is_launcher_running() -> bool {
    match get_copy_protection() {
        Some(handle) => {
            let cp = handle.lock();
            cp.is_launcher_running()
        }
        None => {
            debug!("Copy protection not initialized for launcher check");
            false
        }
    }
}

/// Notify launcher (C++ compatible interface)
pub fn notify_launcher() -> Result<()> {
    match get_copy_protection() {
        Some(handle) => {
            let cp = handle.lock();
            cp.notify_launcher()
        }
        None => Err(anyhow::anyhow!("Copy protection not initialized")),
    }
}

/// Notify launcher with game version (C++ compatible helper for version checks).
pub fn notify_launcher_version(game_version: &str) -> Result<()> {
    match get_copy_protection() {
        Some(handle) => {
            let cp = handle.lock();
            cp.notify_launcher_version(game_version)
        }
        None => Err(anyhow::anyhow!("Copy protection not initialized")),
    }
}

/// Check for launcher message (C++ compatible interface)
pub fn check_for_message() -> Result<Option<LauncherMessage>> {
    match get_copy_protection() {
        Some(handle) => {
            let cp = handle.lock();
            cp.check_for_message()
        }
        None => Ok(None),
    }
}

/// Shutdown copy protection (C++ compatible interface)
pub fn shutdown() -> Result<()> {
    shutdown_copy_protection()
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_development_provider() {
        let provider = DevelopmentProvider::new();

        // All checks should return bypassed in development mode
        assert_eq!(
            provider.is_launcher_running().unwrap(),
            LauncherStatus::Bypassed
        );
        assert_eq!(
            provider.validate_protection().unwrap(),
            ProtectionStatus::Bypassed
        );
        assert!(provider.check_for_message().unwrap().is_none());
        assert!(provider
            .notify_launcher(LauncherMessage::Heartbeat { process_id: 123 })
            .is_ok());
        assert!(provider.shutdown().is_ok());
    }

    #[test]
    fn test_production_provider() {
        let provider = ProductionProvider::new();

        // Production provider should return actual status (currently stubs)
        assert_eq!(
            provider.is_launcher_running().unwrap(),
            LauncherStatus::NotRunning
        );
        assert_eq!(
            provider.validate_protection().unwrap(),
            ProtectionStatus::Valid
        );
        assert!(provider.check_for_message().unwrap().is_none());
        assert!(provider
            .notify_launcher(LauncherMessage::Heartbeat { process_id: 123 })
            .is_ok());
        assert!(provider.shutdown().is_ok());
    }

    #[test]
    fn test_copy_protection_initialization() {
        configure_copy_protection(true, true);

        let mut cp = CopyProtection::new();
        assert!(!cp.is_initialized());

        assert!(cp.initialize().is_ok());
        assert!(cp.is_initialized());
        assert_eq!(cp.provider_name(), "Development");
    }

    #[test]
    fn test_configuration() {
        configure_copy_protection(false, true);
        assert!(!is_development_mode());
        assert!(is_copy_protection_enabled());

        configure_copy_protection(true, false);
        assert!(is_development_mode());
        assert!(!is_copy_protection_enabled());
    }

    #[test]
    fn test_launcher_message_creation() {
        let msg = LauncherMessage::GameStart {
            process_id: 123,
            timestamp: 456,
        };

        match msg {
            LauncherMessage::GameStart {
                process_id,
                timestamp,
            } => {
                assert_eq!(process_id, 123);
                assert_eq!(timestamp, 456);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
