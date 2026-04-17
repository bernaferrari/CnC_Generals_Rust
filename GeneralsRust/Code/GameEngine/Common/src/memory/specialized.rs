//! Specialized Pool Registries for Game Objects and Modules
//!
//! Provides pre-configured pools for common game engine types.

use super::{ObjectPool, PoolConfig};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

/// Registry for game object pools.
///
/// This maintains pre-configured pools for different object types
/// (Units, Structures, Projectiles, etc.) with optimized settings.
pub struct ObjectPoolRegistry {
    pools: RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
}

impl ObjectPoolRegistry {
    /// Create a new object pool registry.
    pub fn new() -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a pool for a specific type.
    pub fn get_or_create<T: 'static + Send + Sync>(
        &self,
        name: &str,
        config_fn: impl FnOnce() -> PoolConfig,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let mut pools = self.pools.write().unwrap();

        if let Some(pool) = pools.get(name) {
            return pool
                .downcast_ref::<Arc<ObjectPool<T>>>()
                .map(Arc::clone)
                .ok_or_else(|| format!("Type mismatch for pool '{}'", name));
        }

        let config = config_fn();
        let pool = ObjectPool::<T>::new(config)?;
        pools.insert(
            name.to_string(),
            Arc::new(pool.clone()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        Ok(pool)
    }

    /// Get an existing pool.
    pub fn get<T: 'static>(&self, name: &str) -> Option<Arc<ObjectPool<T>>> {
        self.pools
            .read()
            .unwrap()
            .get(name)
            .and_then(|p| p.downcast_ref::<Arc<ObjectPool<T>>>().map(Arc::clone))
    }

    /// Register a pre-created pool.
    pub fn register<T: 'static + Send + Sync>(&self, name: &str, pool: Arc<ObjectPool<T>>) {
        self.pools.write().unwrap().insert(
            name.to_string(),
            Arc::new(pool) as Arc<dyn std::any::Any + Send + Sync>,
        );
    }
}

impl Default for ObjectPoolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry for module pools.
///
/// Modules are components attached to game objects. This registry
/// manages pools for all module types.
pub struct ModulePoolRegistry {
    pools: RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
}

impl ModulePoolRegistry {
    /// Create a new module pool registry.
    pub fn new() -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a module pool.
    pub fn get_or_create<T: 'static + Send + Sync>(
        &self,
        module_name: &str,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let mut pools = self.pools.write().unwrap();

        if let Some(pool) = pools.get(module_name) {
            return pool
                .downcast_ref::<Arc<ObjectPool<T>>>()
                .map(Arc::clone)
                .ok_or_else(|| format!("Type mismatch for module pool '{}'", module_name));
        }

        // Use module preset configuration
        let config = PoolConfig::for_modules(format!("Module_{}", module_name));
        let pool = ObjectPool::<T>::new(config)?;
        pools.insert(
            module_name.to_string(),
            Arc::new(pool.clone()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        Ok(pool)
    }

    /// Get an existing module pool.
    pub fn get<T: 'static>(&self, module_name: &str) -> Option<Arc<ObjectPool<T>>> {
        self.pools
            .read()
            .unwrap()
            .get(module_name)
            .and_then(|p| p.downcast_ref::<Arc<ObjectPool<T>>>().map(Arc::clone))
    }
}

impl Default for ModulePoolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global object pool registry.
pub static OBJECT_POOLS: Lazy<ObjectPoolRegistry> = Lazy::new(ObjectPoolRegistry::new);

/// Global module pool registry.
pub static MODULE_POOLS: Lazy<ModulePoolRegistry> = Lazy::new(ModulePoolRegistry::new);

/// Helper function to create game object pool with appropriate settings.
pub fn create_game_object_pool<T: 'static + Send + Sync>(
    name: &str,
) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_game_objects(name);
    ObjectPool::new(config)
}

/// Helper function to create projectile pool with appropriate settings.
pub fn create_projectile_pool<T: 'static + Send + Sync>(
    name: &str,
) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_projectiles(name);
    ObjectPool::new(config)
}

/// Helper function to create small object pool (particles, etc).
pub fn create_small_object_pool<T: 'static + Send + Sync>(
    name: &str,
) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_small_objects(name);
    ObjectPool::new(config)
}

/// Macros for convenient pool access.

/// Get or create a game object pool.
#[macro_export]
macro_rules! game_object_pool {
    ($ty:ty, $name:expr) => {{
        use $crate::memory::specialized::OBJECT_POOLS;
        OBJECT_POOLS.get_or_create::<$ty>($name, || {
            $crate::memory::PoolConfig::for_game_objects($name)
        })
    }};
}

/// Get or create a module pool.
#[macro_export]
macro_rules! module_pool {
    ($ty:ty, $name:expr) => {{
        use $crate::memory::specialized::MODULE_POOLS;
        MODULE_POOLS.get_or_create::<$ty>($name)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestObject {
        id: u32,
        data: [u8; 64],
    }

    #[test]
    fn test_object_pool_registry() {
        let registry = ObjectPoolRegistry::new();

        let pool = registry
            .get_or_create::<TestObject>("TestObj", || PoolConfig::for_game_objects("TestObj"))
            .unwrap();

        let obj = TestObject {
            id: 1,
            data: [0; 64],
        };
        let handle = pool.alloc(obj).unwrap();

        assert!(handle.is_valid());
    }

    #[test]
    fn test_module_pool_registry() {
        let registry = ModulePoolRegistry::new();

        let pool = registry.get_or_create::<u64>("TestModule").unwrap();
        let handle = pool.alloc(42).unwrap();

        assert_eq!(handle.with(|v| *v).unwrap(), 42);
    }

    #[test]
    fn test_global_registries() {
        let pool = create_game_object_pool::<TestObject>("GlobalTest").unwrap();

        let obj = TestObject {
            id: 99,
            data: [0; 64],
        };
        let handle = pool.alloc(obj).unwrap();

        assert!(handle.is_valid());
    }
}
