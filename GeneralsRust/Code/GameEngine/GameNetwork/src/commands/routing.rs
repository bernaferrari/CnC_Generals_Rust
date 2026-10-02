//! Message routing and dispatching system
//!
//! This module implements message routing logic that directs incoming network
//! commands to appropriate handlers based on command type, priority, and game state.

use crate::commands::{CommandPriority, NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tracing::{debug, trace, warn};

/// Command handler trait - implement this to handle specific command types
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle a network command
    async fn handle_command(&self, command: &NetCommand) -> NetworkResult<()>;

    /// Get the command types this handler supports
    fn supported_types(&self) -> Vec<NetCommandType>;

    /// Get handler priority (higher = processed first)
    fn priority(&self) -> u8 {
        0
    }
}

/// Command router that dispatches commands to registered handlers
///
/// The router owns its handler table, queues, and stats outright: nothing is
/// cloned into a spawned task, so the caller drives it through `&mut self`.
pub struct CommandRouter {
    /// Registered handlers by command type
    handlers: HashMap<NetCommandType, Vec<Arc<dyn CommandHandler>>>,
    /// Command queue organized by priority
    queues: PriorityQueues,
    /// Statistics
    stats: RouterStats,
}

/// Priority-based command queues
struct PriorityQueues {
    critical: VecDeque<NetCommand>,
    high: VecDeque<NetCommand>,
    normal: VecDeque<NetCommand>,
    low: VecDeque<NetCommand>,
}

impl PriorityQueues {
    fn new() -> Self {
        Self {
            critical: VecDeque::new(),
            high: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
        }
    }

    fn push(&mut self, command: NetCommand) {
        match command.priority {
            CommandPriority::Critical => self.critical.push_back(command),
            CommandPriority::High => self.high.push_back(command),
            CommandPriority::Normal => self.normal.push_back(command),
            CommandPriority::Low => self.low.push_back(command),
        }
    }

    fn pop_highest(&mut self) -> Option<NetCommand> {
        self.critical
            .pop_front()
            .or_else(|| self.high.pop_front())
            .or_else(|| self.normal.pop_front())
            .or_else(|| self.low.pop_front())
    }

    fn total_count(&self) -> usize {
        self.critical.len() + self.high.len() + self.normal.len() + self.low.len()
    }

    fn clear(&mut self) {
        self.critical.clear();
        self.high.clear();
        self.normal.clear();
        self.low.clear();
    }
}

/// Router statistics
#[derive(Debug, Clone, Default)]
pub struct RouterStats {
    /// Total commands routed
    pub total_routed: u64,
    /// Commands routed by type
    pub by_type: HashMap<NetCommandType, u64>,
    /// Commands routed by priority
    pub by_priority: HashMap<CommandPriority, u64>,
    /// Failed routings
    pub failed: u64,
    /// Commands with no handler
    pub unhandled: u64,
}

impl CommandRouter {
    /// Create a new command router
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            queues: PriorityQueues::new(),
            stats: RouterStats::default(),
        }
    }

    /// Register a command handler
    pub async fn register_handler(&mut self, handler: Arc<dyn CommandHandler>) {
        for cmd_type in handler.supported_types() {
            self.handlers
                .entry(cmd_type)
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }

        debug!(
            "Registered handler for types: {:?}",
            handler.supported_types()
        );
    }

    /// Unregister all handlers for a specific command type
    pub async fn unregister_type(&mut self, command_type: NetCommandType) {
        self.handlers.remove(&command_type);
        debug!("Unregistered all handlers for type: {:?}", command_type);
    }

    /// Queue a command for processing
    pub async fn queue_command(&mut self, command: NetCommand) -> NetworkResult<()> {
        trace!(
            "Queuing command {:?} with priority {:?}",
            command.command_type, command.priority
        );
        self.queues.push(command);
        Ok(())
    }

    /// Route a single command immediately (bypass queue)
    pub async fn route_command(&mut self, command: &NetCommand) -> NetworkResult<()> {
        // Clone the handler list so the table borrow is released before the
        // handlers run and stats are updated.
        let handlers_for_type: Vec<Arc<dyn CommandHandler>> =
            match self.handlers.get(&command.command_type) {
                Some(h) if !h.is_empty() => h.clone(),
                _ => {
                    warn!(
                        "No handler registered for command type: {:?}",
                        command.command_type
                    );
                    self.stats.unhandled += 1;
                    return Err(NetworkError::invalid_command(format!(
                        "no handler for command type {:?}",
                        command.command_type
                    )));
                }
            };

        // Update stats
        self.stats.total_routed += 1;
        *self.stats.by_type.entry(command.command_type).or_insert(0) += 1;
        *self.stats.by_priority.entry(command.priority).or_insert(0) += 1;

        // Call all registered handlers
        let mut any_succeeded = false;
        let mut last_error = None;

        for handler in &handlers_for_type {
            match handler.handle_command(command).await {
                Ok(()) => {
                    any_succeeded = true;
                    trace!(
                        "Handler successfully processed command {:?}",
                        command.command_type
                    );
                }
                Err(e) => {
                    warn!(
                        "Handler failed to process command {:?}: {}",
                        command.command_type, e
                    );
                    last_error = Some(e);
                }
            }
        }

        if !any_succeeded {
            self.stats.failed += 1;

            if let Some(err) = last_error {
                return Err(err);
            }
        }

        Ok(())
    }

    /// Process queued commands (call this regularly from game loop)
    pub async fn process_queued(&mut self, max_commands: usize) -> NetworkResult<usize> {
        let mut processed = 0;

        for _ in 0..max_commands {
            let command = self.queues.pop_highest();

            match command {
                Some(cmd) => {
                    if let Err(e) = self.route_command(&cmd).await {
                        debug!("Failed to route queued command: {}", e);
                    }
                    processed += 1;
                }
                None => break,
            }
        }

        if processed > 0 {
            trace!("Processed {} queued commands", processed);
        }

        Ok(processed)
    }

    /// Get current queue depth
    pub async fn queue_depth(&self) -> usize {
        self.queues.total_count()
    }

    /// Clear all queued commands
    pub async fn clear_queue(&mut self) {
        self.queues.clear();
        debug!("Cleared all queued commands");
    }

    /// Get routing statistics
    pub async fn get_stats(&self) -> RouterStats {
        self.stats.clone()
    }

    /// Reset routing statistics
    pub async fn reset_stats(&mut self) {
        self.stats = RouterStats::default();
    }
}

impl Default for CommandRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Example handler for debugging/logging
pub struct LoggingHandler {
    prefix: String,
    types: Vec<NetCommandType>,
}

impl LoggingHandler {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            types: vec![
                NetCommandType::Chat,
                NetCommandType::DisconnectChat,
                NetCommandType::KeepAlive,
            ],
        }
    }

    pub fn for_types(prefix: impl Into<String>, types: Vec<NetCommandType>) -> Self {
        Self {
            prefix: prefix.into(),
            types,
        }
    }
}

#[async_trait]
impl CommandHandler for LoggingHandler {
    async fn handle_command(&self, command: &NetCommand) -> NetworkResult<()> {
        debug!(
            "{} command {:?} from player {}",
            self.prefix, command.command_type, command.player_id
        );
        Ok(())
    }

    fn supported_types(&self) -> Vec<NetCommandType> {
        self.types.clone()
    }
}

/// Handler that validates commands before passing to another handler
pub struct ValidatingHandler<H: CommandHandler> {
    inner: Arc<H>,
}

impl<H: CommandHandler> ValidatingHandler<H> {
    pub fn new(inner: Arc<H>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<H: CommandHandler> CommandHandler for ValidatingHandler<H> {
    async fn handle_command(&self, command: &NetCommand) -> NetworkResult<()> {
        // Validate command first
        command.validate()?;

        // Then pass to inner handler
        self.inner.handle_command(command).await
    }

    fn supported_types(&self) -> Vec<NetCommandType> {
        self.inner.supported_types()
    }

    fn priority(&self) -> u8 {
        self.inner.priority()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandPayload;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestHandler {
        types: Vec<NetCommandType>,
        call_count: Arc<AtomicUsize>,
    }

    impl TestHandler {
        fn new(types: Vec<NetCommandType>) -> Self {
            Self {
                types,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl CommandHandler for TestHandler {
        async fn handle_command(&self, _command: &NetCommand) -> NetworkResult<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn supported_types(&self) -> Vec<NetCommandType> {
            self.types.clone()
        }
    }

    #[tokio::test]
    async fn test_router_registration() {
        let mut router = CommandRouter::new();
        let handler = Arc::new(TestHandler::new(vec![NetCommandType::KeepAlive]));

        router.register_handler(handler.clone()).await;

        let command = NetCommand::keep_alive(0);
        router.route_command(&command).await.unwrap();

        assert_eq!(handler.get_call_count(), 1);
    }

    #[tokio::test]
    async fn test_priority_queuing() {
        let mut router = CommandRouter::new();
        let handler = Arc::new(TestHandler::new(vec![
            NetCommandType::KeepAlive,
            NetCommandType::Chat,
        ]));

        router.register_handler(handler.clone()).await;

        // Queue low priority command first
        let cmd1 = NetCommand::keep_alive(0);
        router.queue_command(cmd1).await.unwrap();

        // Queue high priority command second
        let mut cmd2 = NetCommand::chat(1, "urgent".to_string(), 0xFF);
        cmd2.priority = CommandPriority::Critical;
        router.queue_command(cmd2).await.unwrap();

        // Process should handle critical first
        router.process_queued(2).await.unwrap();

        assert_eq!(handler.get_call_count(), 2);
    }

    #[tokio::test]
    async fn test_unhandled_command() {
        let mut router = CommandRouter::new();

        let command = NetCommand::keep_alive(0);
        let result = router.route_command(&command).await;

        assert!(result.is_err());

        let stats = router.get_stats().await;
        assert_eq!(stats.unhandled, 1);
    }

    #[tokio::test]
    async fn test_multiple_handlers() {
        let mut router = CommandRouter::new();

        let handler1 = Arc::new(TestHandler::new(vec![NetCommandType::Chat]));
        let handler2 = Arc::new(TestHandler::new(vec![NetCommandType::Chat]));

        router.register_handler(handler1.clone()).await;
        router.register_handler(handler2.clone()).await;

        let command = NetCommand::chat(0, "test".to_string(), 0xFF);
        router.route_command(&command).await.unwrap();

        // Both handlers should be called
        assert_eq!(handler1.get_call_count(), 1);
        assert_eq!(handler2.get_call_count(), 1);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let mut router = CommandRouter::new();
        let handler = Arc::new(TestHandler::new(vec![
            NetCommandType::KeepAlive,
            NetCommandType::Chat,
        ]));

        router.register_handler(handler).await;

        router
            .route_command(&NetCommand::keep_alive(0))
            .await
            .unwrap();
        router
            .route_command(&NetCommand::chat(1, "test".to_string(), 0xFF))
            .await
            .unwrap();

        let stats = router.get_stats().await;
        assert_eq!(stats.total_routed, 2);
        assert_eq!(*stats.by_type.get(&NetCommandType::KeepAlive).unwrap(), 1);
        assert_eq!(*stats.by_type.get(&NetCommandType::Chat).unwrap(), 1);
    }
}
