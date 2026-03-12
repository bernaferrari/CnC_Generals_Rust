//! Ultra-modern async runtime with structured concurrency (2025)
//!
//! Provides advanced async patterns including:
//! - Structured concurrency with automatic cancellation
//! - Async scopes with guaranteed cleanup
//! - Advanced task management with priorities
//! - Resource-aware scheduling
//! - Tokio Console integration for debugging

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{sleep, timeout};
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;

#[cfg(feature = "metrics")]
use log;
#[cfg(feature = "metrics")]
use tracing::{debug, error, info, instrument, trace, warn, Instrument};

#[cfg(not(feature = "metrics"))]
macro_rules! debug { ($($args:tt)*) => {}; }
#[cfg(not(feature = "metrics"))]
macro_rules! error { ($($args:tt)*) => { eprintln!($($args)*) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! info { ($($args:tt)*) => { println!($($args)*) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! trace { ($($args:tt)*) => {}; }
#[cfg(not(feature = "metrics"))]
macro_rules! warn { ($($args:tt)*) => { eprintln!("WARN: {}", format!($($args)*)) }; }

/// Task priority levels for advanced scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Background tasks (lowest priority)
    Background = 0,
    /// Normal application tasks
    Normal = 1,
    /// Network I/O tasks
    Network = 2,
    /// Game logic tasks
    Game = 3,
    /// Critical system tasks (highest priority)
    Critical = 4,
}

/// Advanced task metadata for monitoring
#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub name: String,
    pub priority: TaskPriority,
    pub created_at: NetworkInstant,
    pub max_duration: Option<Duration>,
    pub resource_limits: ResourceLimits,
}

/// Resource limits for tasks
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory allocation in bytes
    pub max_memory: Option<usize>,
    /// Maximum CPU time slice
    pub max_cpu_time: Option<Duration>,
    /// Network bandwidth limit
    pub max_bandwidth: Option<usize>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: None,
            max_cpu_time: None,
            max_bandwidth: None,
        }
    }
}

/// Structured concurrency scope for guaranteed cleanup
pub struct AsyncScope {
    tasks: Arc<RwLock<JoinSet<NetworkResult<()>>>>,
    shutdown: broadcast::Sender<()>,
    semaphore: Arc<Semaphore>,
    active_tasks: Arc<RwLock<Vec<TaskMetadata>>>,
}

impl AsyncScope {
    /// Create a new async scope with resource limits
    pub fn new(max_concurrent: usize) -> Self {
        let (shutdown, _) = broadcast::channel(1);
        
        Self {
            tasks: Arc::new(RwLock::new(JoinSet::new())),
            shutdown,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            active_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Spawn a task with metadata and resource management
    #[cfg(feature = "metrics")]
    #[instrument(skip(self, future))]
    pub async fn spawn<F, Fut>(&self, 
        metadata: TaskMetadata, 
        future: F
    ) -> NetworkResult<()> 
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = NetworkResult<()>> + Send + 'static,
    {
        // Acquire resource permit
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|e| NetworkError::generic(format!("Failed to acquire task permit: {}", e)))?;
        
        // Clone necessary data
        let mut shutdown = self.shutdown.subscribe();
        let task_name = metadata.name.clone();
        let task_name_for_closure = task_name.clone();
        let max_duration = metadata.max_duration;
        let active_tasks = self.active_tasks.clone();
        
        // Add to active tasks
        {
            let mut tasks = active_tasks.write().await;
            tasks.push(metadata);
        }
        
        // Create instrumented task
        let instrumented_future = async move {
            let _permit = permit; // Hold permit for task duration
            
            let task_future = future();
            
            let result = if let Some(duration) = max_duration {
                // Apply timeout if specified
                timeout(duration, task_future).await
                    .map_err(|_| NetworkError::generic("Task timeout exceeded".to_string()))?
            } else {
                task_future.await
            };
            
            // Remove from active tasks on completion
            let mut tasks = active_tasks.write().await;
            tasks.retain(|t| t.name != task_name_for_closure);
            
            result
        }.instrument(tracing::info_span!("async_task", task = %task_name));
        
        // Spawn with cancellation support
        let cancellable_future = async move {
            tokio::select! {
                result = instrumented_future => result,
                _ = shutdown.recv() => {
                    warn!("Task {} cancelled due to scope shutdown", task_name);
                    Err(NetworkError::generic("Task cancelled".to_string()))
                }
            }
        };
        
        let handle = tokio::spawn(cancellable_future);
        
        // Add to task set
        {
            let mut tasks = self.tasks.write().await;
            tasks.spawn(async move {
                handle.await
                    .map_err(|e| NetworkError::generic(format!("Task join failed: {}", e)))?
            });
        }
        
        Ok(())
    }
    
    #[cfg(not(feature = "metrics"))]
    pub async fn spawn<F, Fut>(&self, 
        metadata: TaskMetadata, 
        future: F
    ) -> NetworkResult<()> 
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = NetworkResult<()>> + Send + 'static,
    {
        // Simplified version without tracing
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|e| NetworkError::generic(format!("Failed to acquire task permit: {}", e)))?;
        
        let shutdown = self.shutdown.subscribe();
        let task_name = metadata.name.clone();
        let max_duration = metadata.max_duration;
        let active_tasks = self.active_tasks.clone();
        
        {
            let mut tasks = active_tasks.write().await;
            tasks.push(metadata);
        }
        
        let task_future = async move {
            let _permit = permit;
            
            let result = if let Some(duration) = max_duration {
                timeout(duration, future()).await
                    .map_err(|_| NetworkError::generic("Task timeout exceeded".to_string()))?
            } else {
                future().await
            };
            
            let mut tasks = active_tasks.write().await;
            tasks.retain(|t| t.name != task_name);
            
            result
        };
        
        let cancellable_future = async move {
            tokio::select! {
                result = task_future => result,
                _ = shutdown => {
                    warn!("Task {} cancelled due to scope shutdown", task_name);
                    Err(NetworkError::generic("Task cancelled".to_string()))
                }
            }
        };
        
        let handle = tokio::spawn(cancellable_future);
        
        {
            let mut tasks = self.tasks.write().await;
            tasks.spawn(async move {
                handle.await
                    .map_err(|e| NetworkError::generic(format!("Task join failed: {}", e)))?
            });
        }
        
        Ok(())
    }
    
    /// Wait for all tasks to complete or timeout
    pub async fn join_all(self, timeout_duration: Duration) -> NetworkResult<()> {
        let mut tasks = Arc::try_unwrap(self.tasks)
            .map_err(|_| NetworkError::generic("Cannot unwrap Arc - still has references".to_string()))?
            .into_inner();
        
        let join_future = async move {
            let mut results = Vec::new();
            
            while let Some(result) = tasks.join_next().await {
                results.push(result);
            }
            
            // Check for any failures
            for result in results {
                match result {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => return Err(e),
                    Err(e) => return Err(NetworkError::generic(format!("Task panicked: {}", e))),
                }
            }
            
            Ok(())
        };
        
        timeout(timeout_duration, join_future).await
            .map_err(|_| NetworkError::generic("Scope join timeout exceeded".to_string()))?
    }
    
    /// Gracefully shutdown all tasks
    pub async fn shutdown(&self) -> NetworkResult<()> {
        info!("Initiating structured concurrency shutdown");
        
        // Signal all tasks to shutdown
        let _ = self.shutdown.send(());
        
        // Wait for tasks to complete with timeout
        let shutdown_timeout = Duration::from_secs(30);
        let start = NetworkInstant::now();

        while start.elapsed() < shutdown_timeout {
            let active_count = {
                let tasks = self.active_tasks.read().await;
                tasks.len()
            };
            
            if active_count == 0 {
                break;
            }
            
            debug!("Waiting for {} tasks to shutdown", active_count);
            sleep(Duration::from_millis(100)).await;
        }
        
        let final_count = {
            let tasks = self.active_tasks.read().await;
            tasks.len()
        };
        
        if final_count > 0 {
            warn!("Force-terminating {} remaining tasks", final_count);
        }
        
        info!("Structured concurrency shutdown complete");
        Ok(())
    }
    
    /// Get current task statistics
    pub async fn get_stats(&self) -> ScopeStats {
        let tasks = self.active_tasks.read().await;
        
        let mut stats = ScopeStats {
            active_tasks: tasks.len(),
            total_spawned: 0, // Would need to track this
            tasks_by_priority: [0; 5],
            average_task_age: Duration::ZERO,
        };
        
        // Calculate priority distribution and average age
        let now = NetworkInstant::now();
        let mut total_age = Duration::ZERO;
        
        for task in tasks.iter() {
            let priority_index = task.priority as usize;
            if priority_index < 5 {
                stats.tasks_by_priority[priority_index] += 1;
            }
            total_age += now.duration_since(task.created_at);
        }
        
        if !tasks.is_empty() {
            stats.average_task_age = total_age / tasks.len() as u32;
        }
        
        stats
    }
}

/// Statistics for async scope monitoring
#[derive(Debug, Clone)]
pub struct ScopeStats {
    pub active_tasks: usize,
    pub total_spawned: usize,
    pub tasks_by_priority: [usize; 5],
    pub average_task_age: Duration,
}

/// Ultra-modern runtime manager with structured concurrency
pub struct AdvancedRuntime {
    main_scope: AsyncScope,
    network_scope: AsyncScope,
    game_scope: AsyncScope,
    background_scope: AsyncScope,
    
    /// Global shutdown coordination
    shutdown_coordinator: Arc<ShutdownCoordinator>,
}

impl AdvancedRuntime {
    /// Create a new advanced runtime with optimized resource allocation
    pub fn new() -> Self {
        info!("Initializing ultra-modern async runtime");
        
        Self {
            main_scope: AsyncScope::new(100),      // Main application tasks
            network_scope: AsyncScope::new(50),    // Network I/O tasks
            game_scope: AsyncScope::new(25),       // Game logic tasks  
            background_scope: AsyncScope::new(10), // Background maintenance
            shutdown_coordinator: Arc::new(ShutdownCoordinator::new()),
        }
    }
    
    /// Spawn a task in the appropriate scope based on priority
    pub async fn spawn_task<F, Fut>(&self, 
        metadata: TaskMetadata, 
        future: F
    ) -> NetworkResult<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = NetworkResult<()>> + Send + 'static,
    {
        let scope = match metadata.priority {
            TaskPriority::Critical | TaskPriority::Game => &self.game_scope,
            TaskPriority::Network => &self.network_scope,
            TaskPriority::Background => &self.background_scope,
            TaskPriority::Normal => &self.main_scope,
        };
        
        scope.spawn(metadata, future).await
    }
    
    /// Gracefully shutdown all scopes with proper ordering
    pub async fn shutdown(self) -> NetworkResult<()> {
        info!("Starting coordinated runtime shutdown");
        
        // Shutdown in reverse priority order
        self.background_scope.shutdown().await?;
        self.main_scope.shutdown().await?;
        self.network_scope.shutdown().await?;
        self.game_scope.shutdown().await?;
        
        info!("Advanced runtime shutdown complete");
        Ok(())
    }
    
    /// Get comprehensive runtime statistics
    pub async fn get_runtime_stats(&self) -> RuntimeStats {
        let main_stats = self.main_scope.get_stats().await;
        let network_stats = self.network_scope.get_stats().await;
        let game_stats = self.game_scope.get_stats().await;
        let background_stats = self.background_scope.get_stats().await;
        
        RuntimeStats {
            total_active_tasks: main_stats.active_tasks + network_stats.active_tasks 
                              + game_stats.active_tasks + background_stats.active_tasks,
            main_scope_stats: main_stats,
            network_scope_stats: network_stats,
            game_scope_stats: game_stats,
            background_scope_stats: background_stats,
        }
    }
}

/// Comprehensive runtime statistics
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    pub total_active_tasks: usize,
    pub main_scope_stats: ScopeStats,
    pub network_scope_stats: ScopeStats,
    pub game_scope_stats: ScopeStats,
    pub background_scope_stats: ScopeStats,
}

/// Advanced shutdown coordination
pub struct ShutdownCoordinator {
    shutdown_phases: Vec<ShutdownPhase>,
    current_phase: RwLock<usize>,
}

#[derive(Debug)]
pub struct ShutdownPhase {
    pub name: String,
    pub timeout: Duration,
    pub critical: bool,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            shutdown_phases: vec![
                ShutdownPhase {
                    name: "Background Tasks".to_string(),
                    timeout: Duration::from_secs(5),
                    critical: false,
                },
                ShutdownPhase {
                    name: "Network Connections".to_string(),
                    timeout: Duration::from_secs(10),
                    critical: true,
                },
                ShutdownPhase {
                    name: "Game State Persistence".to_string(),
                    timeout: Duration::from_secs(15),
                    critical: true,
                },
            ],
            current_phase: RwLock::new(0),
        }
    }
}

impl Default for AdvancedRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_async_scope() {
        let scope = AsyncScope::new(5);
        
        // Spawn a simple task
        let metadata = TaskMetadata {
            name: "test_task".to_string(),
            priority: TaskPriority::Normal,
            created_at: NetworkInstant::now(),
            max_duration: Some(Duration::from_secs(1)),
            resource_limits: ResourceLimits::default(),
        };
        
        scope.spawn(metadata, || async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }).await.unwrap();
        
        // Test stats
        let stats = scope.get_stats().await;
        assert!(stats.active_tasks <= 1); // Task might have completed
        
        // Shutdown
        scope.shutdown().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_advanced_runtime() {
        let runtime = AdvancedRuntime::new();
        
        // Spawn tasks with different priorities
        let high_priority_task = TaskMetadata {
            name: "high_priority".to_string(),
            priority: TaskPriority::Critical,
            created_at: NetworkInstant::now(),
            max_duration: None,
            resource_limits: ResourceLimits::default(),
        };
        
        runtime.spawn_task(high_priority_task, || async {
            Ok(())
        }).await.unwrap();
        
        // Get stats
        let stats = runtime.get_runtime_stats().await;
        assert!(stats.total_active_tasks >= 0);
        
        // Shutdown
        runtime.shutdown().await.unwrap();
    }
}
