//! Owned WWDebug session. Host constructs [`WwDebugSession`] and passes `&` / `Arc`.
//! Replaces process-wide COMMAND_STATE / PROFILER_STATE / INSTANCE / HANDLERS /
//! MEMORY_STATS / LOCK.

use crate::cmd::CommandInterfaceState;
use crate::debug_debug::Debug;
use crate::memory::MemoryStats;
use crate::wwdebug::DebugHandlers;
use crate::ProfilerState;
use std::sync::Mutex;

/// Per-game-instance debug / profiler / command / handler / memory session.
pub struct WwDebugSession {
    pub profiler: ProfilerState,
    pub command: CommandInterfaceState,
    pub debug: Mutex<Debug>,
    pub handlers: Mutex<DebugHandlers>,
    pub memory_stats: MemoryStats,
    pub lock: Mutex<()>,
}

impl Default for WwDebugSession {
    fn default() -> Self {
        Self::new()
    }
}

impl WwDebugSession {
    pub fn new() -> Self {
        Self {
            profiler: ProfilerState::new_session(),
            command: CommandInterfaceState::new_session(),
            debug: Mutex::new(Debug::new_session()),
            handlers: Mutex::new(DebugHandlers::new()),
            memory_stats: MemoryStats::new(),
            lock: Mutex::new(()),
        }
    }
}
