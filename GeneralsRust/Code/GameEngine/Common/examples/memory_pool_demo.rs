//! Memory Pool System Demonstration
//!
//! This example demonstrates the key features of the memory pooling system:
//! - Object allocation and deallocation
//! - Generational index safety
//! - Thread-safe concurrent access
//! - Statistics and monitoring
//! - Integration patterns
//!
//! Run with: cargo run --example memory_pool_demo

use game_engine::memory::*;
use std::sync::Arc;
use std::thread;

/// Example game object
#[derive(Debug, Clone)]
struct Unit {
    id: u32,
    name: String,
    health: f32,
    position: [f32; 3],
    velocity: [f32; 3],
}

impl Unit {
    fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            health: 100.0,
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
        }
    }

    fn update(&mut self, delta_time: f32) {
        // Update position based on velocity
        for i in 0..3 {
            self.position[i] += self.velocity[i] * delta_time;
        }
    }

    fn take_damage(&mut self, amount: f32) {
        self.health = (self.health - amount).max(0.0);
    }

    fn is_alive(&self) -> bool {
        self.health > 0.0
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        Memory Pool System Demonstration                 ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Demo 1: Basic allocation
    demo_basic_allocation();

    // Demo 2: Generational indices
    demo_generational_indices();

    // Demo 3: Concurrent access
    demo_concurrent_access();

    // Demo 4: Statistics and monitoring
    demo_statistics();

    // Demo 5: Pool configuration
    demo_configurations();

    // Demo 6: Global registry
    demo_global_registry();

    println!("\n✅ All demonstrations completed successfully!");
}

fn demo_basic_allocation() {
    println!("═══ Demo 1: Basic Allocation ═══");

    let config = PoolConfig::for_game_objects("Units");
    let pool = ObjectPool::<Unit>::new(config).unwrap();

    // Allocate units
    let unit1 = pool.alloc(Unit::new(1, "Tank")).unwrap();
    let unit2 = pool.alloc(Unit::new(2, "Infantry")).unwrap();
    let unit3 = pool.alloc(Unit::new(3, "Aircraft")).unwrap();

    println!("✓ Allocated 3 units");
    println!("  Pool size: {}/{}", pool.len(), pool.capacity());

    // Access units
    unit1
        .with(|u| {
            println!("  Unit {}: {}", u.id, u.name);
        })
        .unwrap();

    // Modify units
    unit2
        .with_mut(|u| {
            u.take_damage(25.0);
            println!("  Unit {} took damage, health: {}", u.id, u.health);
        })
        .unwrap();

    // Units are automatically freed when handles drop
    drop(unit3);
    println!("  Pool size after drop: {}/{}", pool.len(), pool.capacity());

    println!();
}

fn demo_generational_indices() {
    println!("═══ Demo 2: Generational Indices (Safety) ═══");

    let config = PoolConfig::for_game_objects("Units");
    let pool = ObjectPool::<Unit>::new(config).unwrap();

    // Allocate and get index
    let unit = pool.alloc(Unit::new(1, "Tank")).unwrap();
    let old_index = unit.index();
    println!("✓ Allocated unit with index: {}", old_index);

    // Free the unit
    unit.free().unwrap();
    println!("  Freed unit");

    // Allocate new unit in same slot
    let new_unit = pool.alloc(Unit::new(2, "Infantry")).unwrap();
    let new_index = new_unit.index();
    println!("  Allocated new unit with index: {}", new_index);

    // Check generations
    println!("  Old index valid: {}", pool.is_valid(old_index));
    println!("  New index valid: {}", pool.is_valid(new_index));
    println!("  Slot reused but generation changed ✓");

    println!();
}

fn demo_concurrent_access() {
    println!("═══ Demo 3: Concurrent Access ═══");

    let config = PoolConfig::for_game_objects("Units");
    let pool = ObjectPool::<Unit>::new(config).unwrap();

    // Allocate units from multiple threads
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let pool = Arc::clone(&pool);
            thread::spawn(move || {
                let mut thread_units = Vec::new();
                for i in 0..10 {
                    let id = t * 10 + i;
                    let unit = pool.alloc(Unit::new(id, "ThreadUnit")).unwrap();
                    thread_units.push(unit);
                }
                thread_units
            })
        })
        .collect();

    // Wait for all threads
    let all_units: Vec<_> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();

    println!("✓ Allocated {} units from 8 threads", all_units.len());
    println!("  Pool size: {}/{}", pool.len(), pool.capacity());

    // Update all units concurrently (read-only)
    let update_handles: Vec<_> = all_units
        .chunks(10)
        .map(|chunk| {
            let chunk = chunk.to_vec();
            thread::spawn(move || {
                for unit in chunk {
                    unit.with(|u| {
                        // Simulate read-only operations
                        let _ = u.is_alive();
                    })
                    .unwrap();
                }
            })
        })
        .collect();

    for handle in update_handles {
        handle.join().unwrap();
    }

    println!("  Updated all units concurrently ✓");
    println!();
}

fn demo_statistics() {
    println!("═══ Demo 4: Statistics and Monitoring ═══");

    let config = PoolConfig::for_game_objects("Units");
    let pool = ObjectPool::<Unit>::new(config).unwrap();

    // Allocate and deallocate to generate stats
    let mut units = Vec::new();
    for i in 0..50 {
        units.push(pool.alloc(Unit::new(i, "Unit")).unwrap());
    }

    // Remove some
    for _ in 0..20 {
        units.pop();
    }

    // Get statistics
    let stats = pool.stats().snapshot();
    println!("✓ Pool Statistics:");
    println!("  Total allocations:  {}", stats.total_allocations);
    println!("  Active allocations: {}", stats.active_allocations);
    println!("  Peak allocations:   {}", stats.peak_allocations);
    println!("  Utilization:        {:.1}%", stats.utilization * 100.0);
    println!("  Fragmentation:      {:.1}%", stats.fragmentation * 100.0);
    println!("  Avg alloc time:     {:.2} μs", stats.avg_alloc_time_us);

    // Check for optimization opportunities
    let recommendations = stats.needs_optimization();
    if recommendations.is_empty() {
        println!("  Pool is well-configured ✓");
    } else {
        println!("  Recommendations:");
        for rec in recommendations {
            println!("    • {}", rec);
        }
    }

    println!();
}

fn demo_configurations() {
    println!("═══ Demo 5: Pool Configurations ═══");

    // Game objects
    let game_config = PoolConfig::for_game_objects("GameObjects");
    println!("✓ Game Objects Config:");
    println!("  Initial capacity: {}", game_config.initial_capacity);
    println!("  Cache aligned:    {}", game_config.cache_line_aligned);
    println!("  Max capacity:     {:?}", game_config.max_capacity);

    // Projectiles
    let proj_config = PoolConfig::for_projectiles("Projectiles");
    println!("✓ Projectiles Config:");
    println!("  Initial capacity: {}", proj_config.initial_capacity);
    println!("  Max capacity:     {:?}", proj_config.max_capacity);

    // Custom configuration
    let custom_config = PoolConfigBuilder::new("CustomPool")
        .with_initial_capacity(128)
        .with_grow_by(64)
        .cache_line_aligned()
        .track_allocations()
        .build();
    println!("✓ Custom Config:");
    println!("  Initial capacity: {}", custom_config.initial_capacity);
    println!("  Growth size:      {:?}", custom_config.grow_by);
    println!("  Alignment:        {} bytes", custom_config.alignment());

    println!();
}

fn demo_global_registry() {
    println!("═══ Demo 6: Global Registry ═══");

    // Create and register pools
    let unit_pool = ObjectPool::<Unit>::new(PoolConfig::for_game_objects("Units")).unwrap();
    POOL_REGISTRY.register("Units".to_string(), unit_pool.clone());

    println!("✓ Registered 'Units' pool");

    // Allocate through pool
    let _unit = unit_pool.alloc(Unit::new(1, "Tank")).unwrap();

    // Retrieve from registry
    let _retrieved_pool = POOL_REGISTRY.get::<Unit>("Units").unwrap();
    println!("  Retrieved pool from registry ✓");

    // Get global stats
    let global_stats = POOL_REGISTRY.memory_stats();
    println!("  Total pools:       {}", global_stats.total_pools);
    println!("  Total allocations: {}", global_stats.total_allocations);

    println!();
}
