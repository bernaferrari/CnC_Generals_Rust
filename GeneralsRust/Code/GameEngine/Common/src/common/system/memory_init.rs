//! Memory initialization and pool configuration
//!
//! This module provides memory pool initialization functionality similar to
//! the C++ MemoryInit.cpp file. It defines default pool sizes and configuration
//! for the memory management system.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Pool initialization record structure
#[derive(Debug, Clone)]
pub struct PoolInitRec {
    /// Name of the memory pool
    pub name: String,
    /// Size of individual allocations
    pub alloc_size: usize,
    /// Initial number of objects to allocate
    pub initial_count: usize,
    /// Number of objects to allocate when pool overflows
    pub overflow_count: usize,
}

impl PoolInitRec {
    /// Create a new pool initialization record
    pub fn new(name: &str, alloc_size: usize, initial_count: usize, overflow_count: usize) -> Self {
        Self {
            name: name.to_string(),
            alloc_size,
            initial_count,
            overflow_count,
        }
    }
}

/// Memory pool size record for configuration
#[derive(Debug, Clone)]
pub struct PoolSizeRec {
    /// Pool name
    pub name: String,
    /// Initial allocation count
    pub initial: usize,
    /// Overflow allocation count
    pub overflow: usize,
}

impl PoolSizeRec {
    /// Create a new pool size record
    pub fn new(name: &str, initial: usize, overflow: usize) -> Self {
        Self {
            name: name.to_string(),
            initial,
            overflow,
        }
    }
}

/// Default DMA pool parameters
///
/// These correspond to the defaultDMA array in the C++ code.
pub fn get_default_dma_params() -> Vec<PoolInitRec> {
    vec![
        PoolInitRec::new("dmaPool_16", 16, 130000, 10000),
        PoolInitRec::new("dmaPool_32", 32, 250000, 10000),
        PoolInitRec::new("dmaPool_64", 64, 100000, 10000),
        PoolInitRec::new("dmaPool_128", 128, 80000, 10000),
        PoolInitRec::new("dmaPool_256", 256, 20000, 5000),
        PoolInitRec::new("dmaPool_512", 512, 16000, 5000),
        PoolInitRec::new("dmaPool_1024", 1024, 6000, 1024),
    ]
}

/// Default pool sizes for game objects
///
/// This corresponds to the sizes array in the C++ MemoryInit.cpp file.
/// It includes pool sizes for various game object types.
pub fn get_default_pool_sizes() -> Vec<PoolSizeRec> {
    vec![
        // Core system pools
        PoolSizeRec::new("PartitionContactListNode", 2048, 512),
        PoolSizeRec::new("GameMessage", 2048, 32),
        PoolSizeRec::new("NameKeyBucketPool", 9000, 1024),
        // Game object pools
        PoolSizeRec::new("ObjectPool", 1500, 256),
        PoolSizeRec::new("Drawable", 4096, 32),
        PoolSizeRec::new("Image", 2048, 32),
        PoolSizeRec::new("ParticlePool", 1400, 1024),
        PoolSizeRec::new("ParticleSystemTemplatePool", 1100, 32),
        PoolSizeRec::new("ParticleSystemPool", 1024, 32),
        // AI and pathfinding pools
        PoolSizeRec::new("PathNodePool", 8192, 1024),
        PoolSizeRec::new("PathPool", 256, 16),
        PoolSizeRec::new("AIStateMachine", 600, 32),
        PoolSizeRec::new("AIAttackMoveStateMachine", 2048, 32),
        PoolSizeRec::new("AttackStateMachine", 512, 32),
        // Unit and structure pools
        PoolSizeRec::new("Weapon", 4096, 32),
        PoolSizeRec::new("WeaponTemplate", 360, 32),
        PoolSizeRec::new("Locomotor", 2048, 32),
        PoolSizeRec::new("LocomotorTemplate", 192, 32),
        // Audio pools
        PoolSizeRec::new("AudioEventInfo", 4096, 64),
        PoolSizeRec::new("AudioRequest", 256, 8),
        PoolSizeRec::new("DynamicAudioEventInfo", 16, 256),
        // UI pools
        PoolSizeRec::new("CommandButton", 1024, 256),
        PoolSizeRec::new("CommandSet", 820, 16),
        PoolSizeRec::new("WindowLayoutPool", 32, 32),
        PoolSizeRec::new("W3DGameWindow", 700, 256),
        // Special behavior pools
        PoolSizeRec::new("SlowDeathBehavior", 1400, 256),
        PoolSizeRec::new("AutoHealBehavior", 1024, 256),
        PoolSizeRec::new("PhysicsBehavior", 600, 32),
        PoolSizeRec::new("PoisonedBehavior", 512, 64),
        // Network and scripting pools
        PoolSizeRec::new("NetCommandList", 512, 32),
        PoolSizeRec::new("ScriptAction", 2600, 512),
        PoolSizeRec::new("Script", 1024, 256),
        PoolSizeRec::new("Parameter", 8192, 1024),
        PoolSizeRec::new("Condition", 2048, 256),
        // File system pools
        PoolSizeRec::new("Win32LocalFile", 1024, 256),
        PoolSizeRec::new("RAMFile", 32, 32),
        PoolSizeRec::new("StreamingArchiveFile", 8, 8),
        // Rendering pools
        PoolSizeRec::new("W3DDefaultDraw", 1024, 128),
        PoolSizeRec::new("W3DModelDraw", 2048, 512),
        PoolSizeRec::new("W3DTankDraw", 256, 32),
        PoolSizeRec::new("W3DTruckDraw", 128, 32),
        // Memory management pools
        PoolSizeRec::new("VertexMaterialClass", 6000, 2048),
        PoolSizeRec::new("TextureClass", 1200, 256),
        PoolSizeRec::new("MeshClass", 14000, 2000),
        PoolSizeRec::new("ShareBufferClass", 32768, 1024),
    ]
}

/// Memory pool manager
///
/// This manages the configuration and initialization of memory pools.
pub struct MemoryPoolManager {
    /// Pool configurations indexed by name
    pool_configs: HashMap<String, PoolSizeRec>,
    /// DMA pool configurations
    dma_configs: Vec<PoolInitRec>,
    /// Whether the manager has been initialized
    initialized: bool,
}

impl MemoryPoolManager {
    /// Create a new memory pool manager
    pub fn new() -> Self {
        Self {
            pool_configs: HashMap::new(),
            dma_configs: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize with default pool configurations
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Load default DMA configurations
        self.dma_configs = get_default_dma_params();

        // Load default pool size configurations
        for pool_size in get_default_pool_sizes() {
            self.pool_configs.insert(pool_size.name.clone(), pool_size);
        }

        // Try to load overrides from configuration file
        self.load_config_overrides();

        self.initialized = true;
    }

    /// Load configuration overrides from MemoryPools.ini
    ///
    /// This corresponds to the userMemoryManagerInitPools function in C++.
    fn load_config_overrides(&mut self) {
        // In a real implementation, this would read from "Data/INI/MemoryPools.ini"
        // For now, it's a placeholder that could be extended to read from a config file

        // Example of how configuration might be loaded:
        // if let Ok(config_content) = std::fs::read_to_string("Data/INI/MemoryPools.ini") {
        //     self.parse_config_file(&config_content);
        // }
    }

    /// Get pool size configuration for a named pool
    pub fn get_pool_config(&self, pool_name: &str) -> Option<&PoolSizeRec> {
        self.pool_configs.get(pool_name)
    }

    /// Adjust pool size for a named pool
    ///
    /// This corresponds to userMemoryAdjustPoolSize in the C++ code.
    pub fn adjust_pool_size(&mut self, pool_name: &str, initial: usize, overflow: usize) {
        if let Some(config) = self.pool_configs.get_mut(pool_name) {
            config.initial = initial;
            config.overflow = overflow;
        } else {
            // Create new configuration if it doesn't exist
            self.pool_configs.insert(
                pool_name.to_string(),
                PoolSizeRec::new(pool_name, initial, overflow),
            );
        }
    }

    /// Get DMA pool configurations
    pub fn get_dma_configs(&self) -> &[PoolInitRec] {
        &self.dma_configs
    }

    /// Get all pool configurations
    pub fn get_all_pool_configs(&self) -> &HashMap<String, PoolSizeRec> {
        &self.pool_configs
    }

    /// Round up memory boundary for alignment
    ///
    /// This corresponds to roundUpMemBound in the C++ code.
    pub fn round_up_mem_bound(size: usize) -> usize {
        const MEM_BOUND_ALIGNMENT: usize = 4;

        if size < MEM_BOUND_ALIGNMENT {
            MEM_BOUND_ALIGNMENT
        } else {
            (size + (MEM_BOUND_ALIGNMENT - 1)) & !(MEM_BOUND_ALIGNMENT - 1)
        }
    }
}

impl Default for MemoryPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global memory pool manager instance
lazy_static::lazy_static! {
    pub static ref MEMORY_POOL_MANAGER: Arc<Mutex<MemoryPoolManager>> =
        Arc::new(Mutex::new(MemoryPoolManager::new()));
}

/// Initialize memory pools
///
/// This corresponds to the userMemoryManagerInitPools function in C++.
pub fn init_memory_pools() {
    let mut manager = MEMORY_POOL_MANAGER.lock().unwrap();
    manager.init();
}

/// Get memory pool manager instance
pub fn get_memory_pool_manager() -> Arc<Mutex<MemoryPoolManager>> {
    MEMORY_POOL_MANAGER.clone()
}

/// User memory manager DMA parameters callback
///
/// This corresponds to userMemoryManagerGetDmaParms in the C++ code.
pub fn get_user_memory_dma_params() -> (usize, Vec<PoolInitRec>) {
    let dma_params = get_default_dma_params();
    let num_sub_pools = dma_params.len();
    (num_sub_pools, dma_params)
}

/// Adjust pool size for a specific pool
///
/// This corresponds to userMemoryAdjustPoolSize in the C++ code.
pub fn adjust_pool_size(
    pool_name: &str,
    initial_allocation_count: &mut usize,
    overflow_allocation_count: &mut usize,
) {
    if *initial_allocation_count > 0 {
        return; // Already configured
    }

    let manager = MEMORY_POOL_MANAGER.lock().unwrap();
    if let Some(config) = manager.get_pool_config(pool_name) {
        *initial_allocation_count = config.initial;
        *overflow_allocation_count = config.overflow;
    } else {
        eprintln!(
            "Initial size for pool {} not found -- you should add it to memory pool configuration",
            pool_name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_init_rec() {
        let rec = PoolInitRec::new("TestPool", 64, 100, 50);
        assert_eq!(rec.name, "TestPool");
        assert_eq!(rec.alloc_size, 64);
        assert_eq!(rec.initial_count, 100);
        assert_eq!(rec.overflow_count, 50);
    }

    #[test]
    fn test_pool_size_rec() {
        let rec = PoolSizeRec::new("TestPool", 200, 100);
        assert_eq!(rec.name, "TestPool");
        assert_eq!(rec.initial, 200);
        assert_eq!(rec.overflow, 100);
    }

    #[test]
    fn test_default_dma_params() {
        let dma_params = get_default_dma_params();
        assert_eq!(dma_params.len(), 7);

        // Verify first DMA pool
        assert_eq!(dma_params[0].name, "dmaPool_16");
        assert_eq!(dma_params[0].alloc_size, 16);
        assert_eq!(dma_params[0].initial_count, 130000);
        assert_eq!(dma_params[0].overflow_count, 10000);
    }

    #[test]
    fn test_default_pool_sizes() {
        let pool_sizes = get_default_pool_sizes();
        assert!(!pool_sizes.is_empty());

        // Check for some expected pools
        let pool_names: Vec<&str> = pool_sizes.iter().map(|p| p.name.as_str()).collect();
        assert!(pool_names.contains(&"ObjectPool"));
        assert!(pool_names.contains(&"ParticlePool"));
        assert!(pool_names.contains(&"PathNodePool"));
    }

    #[test]
    fn test_memory_pool_manager() {
        let mut manager = MemoryPoolManager::new();
        assert!(!manager.initialized);

        manager.init();
        assert!(manager.initialized);
        assert!(!manager.get_all_pool_configs().is_empty());

        // Test getting specific pool config
        let object_pool = manager.get_pool_config("ObjectPool");
        assert!(object_pool.is_some());

        let config = object_pool.unwrap();
        assert_eq!(config.name, "ObjectPool");
    }

    #[test]
    fn test_adjust_pool_size() {
        let mut manager = MemoryPoolManager::new();
        manager.init();

        // Adjust existing pool
        manager.adjust_pool_size("ObjectPool", 2000, 500);
        let config = manager.get_pool_config("ObjectPool").unwrap();
        assert_eq!(config.initial, 2000);
        assert_eq!(config.overflow, 500);

        // Create new pool configuration
        manager.adjust_pool_size("NewPool", 100, 25);
        let new_config = manager.get_pool_config("NewPool").unwrap();
        assert_eq!(new_config.initial, 100);
        assert_eq!(new_config.overflow, 25);
    }

    #[test]
    fn test_round_up_mem_bound() {
        assert_eq!(MemoryPoolManager::round_up_mem_bound(1), 4);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(4), 4);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(5), 8);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(8), 8);
        assert_eq!(MemoryPoolManager::round_up_mem_bound(9), 12);
    }

    #[test]
    fn test_user_memory_dma_params() {
        let (num_pools, dma_params) = get_user_memory_dma_params();
        assert_eq!(num_pools, dma_params.len());
        assert_eq!(num_pools, 7);
    }

    #[test]
    fn test_adjust_pool_size_function() {
        // Test with already configured pool
        let mut initial = 100usize;
        let mut overflow = 50usize;

        adjust_pool_size("TestPool", &mut initial, &mut overflow);
        assert_eq!(initial, 100); // Should remain unchanged
        assert_eq!(overflow, 50);

        // Test with unconfigured pool
        initial = 0;
        overflow = 0;

        init_memory_pools(); // Ensure manager is initialized
        adjust_pool_size("ObjectPool", &mut initial, &mut overflow);
        assert_ne!(initial, 0); // Should be set to default value
        assert_ne!(overflow, 0);
    }
}
