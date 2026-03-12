//! Pool Configuration
//!
//! This module defines configuration options for memory pools,
//! including capacity, alignment, and growth strategies.

use std::alloc::Layout;

/// Configuration for a memory pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Human-readable name for debugging and statistics.
    pub name: String,

    /// Initial number of objects to pre-allocate.
    pub initial_capacity: usize,

    /// Number of objects to grow by when capacity is exceeded.
    /// If None, pool will not grow (allocations will fail).
    pub grow_by: Option<usize>,

    /// Whether to align objects to cache line boundaries (64 bytes).
    /// This improves performance for frequently accessed objects.
    pub cache_line_aligned: bool,

    /// Custom alignment requirement (overrides cache_line_aligned).
    pub custom_alignment: Option<usize>,

    /// Maximum pool capacity. If None, no limit.
    pub max_capacity: Option<usize>,

    /// Whether to enable detailed allocation tracking (has performance cost).
    pub track_allocations: bool,

    /// Whether to zero memory on allocation (for security/debugging).
    pub zero_on_alloc: bool,

    /// Whether to zero memory on deallocation (for security).
    pub zero_on_free: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            name: "UnnamedPool".to_string(),
            initial_capacity: 256,
            grow_by: Some(128),
            cache_line_aligned: false,
            custom_alignment: None,
            max_capacity: None,
            track_allocations: cfg!(debug_assertions),
            zero_on_alloc: false,
            zero_on_free: cfg!(debug_assertions),
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Get the alignment to use for this pool.
    pub fn alignment(&self) -> usize {
        if let Some(align) = self.custom_alignment {
            align
        } else if self.cache_line_aligned {
            64 // Standard cache line size
        } else {
            std::mem::align_of::<usize>() // Default alignment
        }
    }

    /// Validate the configuration.
    pub fn validate<T>(&self) -> Result<(), String> {
        // Check alignment is power of 2
        let align = self.alignment();
        if !align.is_power_of_two() {
            return Err(format!("Alignment {} is not a power of 2", align));
        }

        // Check initial capacity is not zero
        if self.initial_capacity == 0 {
            return Err("Initial capacity cannot be zero".to_string());
        }

        // Check max capacity is reasonable
        if let Some(max_cap) = self.max_capacity {
            if max_cap < self.initial_capacity {
                return Err(format!(
                    "Max capacity {} is less than initial capacity {}",
                    max_cap, self.initial_capacity
                ));
            }
        }

        // Ensure type alignment is compatible
        let type_align = std::mem::align_of::<T>();
        if align < type_align {
            return Err(format!(
                "Pool alignment {} is less than type alignment {}",
                align, type_align
            ));
        }

        Ok(())
    }

    /// Create a memory layout for this configuration.
    pub fn create_layout<T>(&self) -> Result<Layout, String> {
        let size = std::mem::size_of::<T>();
        let align = self.alignment();

        Layout::from_size_align(size, align).map_err(|e| format!("Invalid layout: {}", e))
    }
}

/// Builder for PoolConfig with fluent API.
pub struct PoolConfigBuilder {
    config: PoolConfig,
}

impl PoolConfigBuilder {
    /// Create a new builder with the given pool name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: PoolConfig::new(name),
        }
    }

    /// Set initial capacity.
    pub fn with_initial_capacity(mut self, capacity: usize) -> Self {
        self.config.initial_capacity = capacity;
        self
    }

    /// Set growth size.
    pub fn with_grow_by(mut self, grow_by: usize) -> Self {
        self.config.grow_by = Some(grow_by);
        self
    }

    /// Disable pool growth.
    pub fn fixed_size(mut self) -> Self {
        self.config.grow_by = None;
        self
    }

    /// Enable cache line alignment.
    pub fn cache_line_aligned(mut self) -> Self {
        self.config.cache_line_aligned = true;
        self
    }

    /// Set custom alignment (must be power of 2).
    pub fn with_alignment(mut self, align: usize) -> Self {
        self.config.custom_alignment = Some(align);
        self
    }

    /// Set maximum capacity.
    pub fn with_max_capacity(mut self, max_capacity: usize) -> Self {
        self.config.max_capacity = Some(max_capacity);
        self
    }

    /// Enable allocation tracking.
    pub fn track_allocations(mut self) -> Self {
        self.config.track_allocations = true;
        self
    }

    /// Enable zeroing memory on allocation.
    pub fn zero_on_alloc(mut self) -> Self {
        self.config.zero_on_alloc = true;
        self
    }

    /// Enable zeroing memory on deallocation.
    pub fn zero_on_free(mut self) -> Self {
        self.config.zero_on_free = true;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> PoolConfig {
        self.config
    }
}

/// Preset configurations for common use cases.
impl PoolConfig {
    /// Configuration for small, frequently allocated objects (e.g., particles).
    pub fn for_small_objects(name: impl Into<String>) -> Self {
        PoolConfigBuilder::new(name)
            .with_initial_capacity(2048)
            .with_grow_by(1024)
            .cache_line_aligned()
            .build()
    }

    /// Configuration for game objects (medium-sized, long-lived).
    pub fn for_game_objects(name: impl Into<String>) -> Self {
        PoolConfigBuilder::new(name)
            .with_initial_capacity(512)
            .with_grow_by(256)
            .with_max_capacity(4096)
            .cache_line_aligned()
            .track_allocations()
            .build()
    }

    /// Configuration for modules (many types, moderate allocation).
    pub fn for_modules(name: impl Into<String>) -> Self {
        PoolConfigBuilder::new(name)
            .with_initial_capacity(1024)
            .with_grow_by(512)
            .cache_line_aligned()
            .build()
    }

    /// Configuration for projectiles (high churn, temporary).
    pub fn for_projectiles(name: impl Into<String>) -> Self {
        PoolConfigBuilder::new(name)
            .with_initial_capacity(256)
            .with_grow_by(128)
            .with_max_capacity(2048)
            .cache_line_aligned()
            .build()
    }

    /// Configuration for debug/development with strict checking.
    pub fn for_debug(name: impl Into<String>) -> Self {
        PoolConfigBuilder::new(name)
            .with_initial_capacity(64)
            .with_grow_by(32)
            .track_allocations()
            .zero_on_alloc()
            .zero_on_free()
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PoolConfig::default();
        assert_eq!(config.initial_capacity, 256);
        assert_eq!(config.grow_by, Some(128));
    }

    #[test]
    fn test_builder() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(1024)
            .with_grow_by(512)
            .cache_line_aligned()
            .build();

        assert_eq!(config.name, "TestPool");
        assert_eq!(config.initial_capacity, 1024);
        assert_eq!(config.grow_by, Some(512));
        assert!(config.cache_line_aligned);
        assert_eq!(config.alignment(), 64);
    }

    #[test]
    fn test_alignment() {
        let config = PoolConfig::default();
        assert!(config.alignment().is_power_of_two());

        let config = PoolConfigBuilder::new("Test").cache_line_aligned().build();
        assert_eq!(config.alignment(), 64);

        let config = PoolConfigBuilder::new("Test").with_alignment(128).build();
        assert_eq!(config.alignment(), 128);
    }

    #[test]
    fn test_preset_configs() {
        let config = PoolConfig::for_game_objects("Objects");
        assert!(config.cache_line_aligned);
        assert_eq!(config.max_capacity, Some(4096));

        let config = PoolConfig::for_projectiles("Projectiles");
        assert_eq!(config.initial_capacity, 256);
    }

    #[test]
    fn test_validation() {
        let config = PoolConfig::for_game_objects("Test");
        assert!(config.validate::<u64>().is_ok());

        let mut bad_config = PoolConfig::default();
        bad_config.initial_capacity = 0;
        assert!(bad_config.validate::<u64>().is_err());

        let mut bad_config = PoolConfig::default();
        bad_config.max_capacity = Some(10);
        assert!(bad_config.validate::<u64>().is_err());
    }
}
