//! Specialized Pool Registries for Game Objects and Modules
//!
//! Provides pre-configured pools for common game engine types.

use super::{ObjectPool, PoolConfig};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry for game object pools.
///
/// This maintains pre-configured pools for different object types
/// (Units, Structures, Projectiles, etc.) with optimized settings.
///
/// Single-owner like the pools it holds; callers own each registry explicitly.
pub struct ObjectPoolRegistry {
    pools: HashMap<String, Box<dyn std::any::Any>>,
}

impl ObjectPoolRegistry {
    /// Create a new object pool registry.
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Get or create a pool for a specific type.
    pub fn get_or_create<T: 'static + Send>(
        &mut self,
        name: &str,
        config_fn: impl FnOnce() -> PoolConfig,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        if let Some(pool) = self.pools.get(name) {
            return pool
                .downcast_ref::<Arc<ObjectPool<T>>>()
                .map(Arc::clone)
                .ok_or_else(|| format!("Type mismatch for pool '{}'", name));
        }

        let config = config_fn();
        let pool = ObjectPool::<T>::new(config)?;
        self.pools
            .insert(name.to_string(), Box::new(Arc::clone(&pool)));

        Ok(pool)
    }

    /// Get an existing pool.
    pub fn get<T: 'static>(&self, name: &str) -> Option<Arc<ObjectPool<T>>> {
        self.pools
            .get(name)
            .and_then(|p| p.downcast_ref::<Arc<ObjectPool<T>>>().map(Arc::clone))
    }

    /// Register a pre-created pool.
    pub fn register<T: 'static + Send>(&mut self, name: &str, pool: Arc<ObjectPool<T>>) {
        self.pools.insert(name.to_string(), Box::new(pool));
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
    pools: HashMap<String, Box<dyn std::any::Any>>,
}

impl ModulePoolRegistry {
    /// Create a new module pool registry.
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Get or create a module pool.
    pub fn get_or_create<T: 'static + Send>(
        &mut self,
        module_name: &str,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        if let Some(pool) = self.pools.get(module_name) {
            return pool
                .downcast_ref::<Arc<ObjectPool<T>>>()
                .map(Arc::clone)
                .ok_or_else(|| format!("Type mismatch for module pool '{}'", module_name));
        }

        // Use module preset configuration
        let config = PoolConfig::for_modules(format!("Module_{}", module_name));
        let pool = ObjectPool::<T>::new(config)?;
        self.pools
            .insert(module_name.to_string(), Box::new(Arc::clone(&pool)));

        Ok(pool)
    }

    /// Get an existing module pool.
    pub fn get<T: 'static>(&self, module_name: &str) -> Option<Arc<ObjectPool<T>>> {
        self.pools
            .get(module_name)
            .and_then(|p| p.downcast_ref::<Arc<ObjectPool<T>>>().map(Arc::clone))
    }
}

impl Default for ModulePoolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create game object pool with appropriate settings.
pub fn create_game_object_pool<T: 'static + Send>(
    name: &str,
) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_game_objects(name);
    ObjectPool::new(config)
}

/// Helper function to create projectile pool with appropriate settings.
pub fn create_projectile_pool<T: 'static + Send>(name: &str) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_projectiles(name);
    ObjectPool::new(config)
}

/// Helper function to create small object pool (particles, etc).
pub fn create_small_object_pool<T: 'static + Send>(
    name: &str,
) -> Result<Arc<ObjectPool<T>>, String> {
    let config = PoolConfig::for_small_objects(name);
    ObjectPool::new(config)
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
        let mut registry = ObjectPoolRegistry::new();

        let pool = registry
            .get_or_create::<TestObject>("TestObj", || PoolConfig::for_game_objects("TestObj"))
            .unwrap();

        let obj = TestObject {
            id: 1,
            data: [0; 64],
        };
        let handle = pool.alloc(obj).unwrap();

        assert!(handle.is_valid());
        let existing = registry
            .get_or_create::<TestObject>("TestObj", || panic!("existing config must not rerun"))
            .unwrap();
        assert!(Arc::ptr_eq(&pool, &existing));
        assert!(Arc::ptr_eq(
            &pool,
            &registry.get::<TestObject>("TestObj").unwrap()
        ));
        assert!(registry.get::<u64>("TestObj").is_none());
        assert_eq!(
            registry
                .get_or_create::<u64>("TestObj", || panic!("type mismatch must not run config"))
                .err()
                .expect("type mismatch"),
            "Type mismatch for pool 'TestObj'"
        );
        let mut other = ObjectPoolRegistry::new();
        assert!(other.get::<TestObject>("TestObj").is_none());
        let independent = other
            .get_or_create::<TestObject>("TestObj", || PoolConfig::for_game_objects("Other"))
            .unwrap();
        assert!(!Arc::ptr_eq(&pool, &independent));
        registry.register("TestObj", Arc::clone(&independent));
        assert!(Arc::ptr_eq(
            &independent,
            &registry.get::<TestObject>("TestObj").unwrap()
        ));
        drop(registry);
        assert!(handle.is_valid(), "pool handles outlive the registry");
    }

    #[test]
    fn test_module_pool_registry() {
        let mut registry = ModulePoolRegistry::new();

        let pool = registry.get_or_create::<u64>("TestModule").unwrap();
        let handle = pool.alloc(42).unwrap();

        assert_eq!(handle.with(|v| *v).unwrap(), 42);
        let existing = registry.get_or_create::<u64>("TestModule").unwrap();
        assert!(Arc::ptr_eq(&pool, &existing));
        assert!(Arc::ptr_eq(
            &pool,
            &registry.get::<u64>("TestModule").unwrap()
        ));
        assert!(registry.get::<u32>("TestModule").is_none());
        assert_eq!(
            registry
                .get_or_create::<u32>("TestModule")
                .err()
                .expect("type mismatch"),
            "Type mismatch for module pool 'TestModule'"
        );
        let mut other = ModulePoolRegistry::new();
        assert!(other.get::<u64>("TestModule").is_none());
        let independent = other.get_or_create::<u64>("TestModule").unwrap();
        assert!(!Arc::ptr_eq(&pool, &independent));
        assert_eq!(independent.config().name, "Module_TestModule");
        drop(registry);
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
