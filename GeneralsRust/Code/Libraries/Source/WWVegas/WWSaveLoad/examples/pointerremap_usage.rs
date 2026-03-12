//! # Pointer Remapping Usage Example
//!
//! This example demonstrates how to use the pointer remapping system
//! for save/load operations in a safe Rust environment.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use ww_save_load::pointerremap::{PointerRemapClass, RefCountable};

/// Example object that might exist in a game world
#[derive(Debug)]
struct GameObject {
    id: u32,
    name: String,
    position: (f32, f32, f32),
}

impl GameObject {
    fn new(id: u32, name: &str, x: f32, y: f32, z: f32) -> Self {
        Self {
            id,
            name: name.to_string(),
            position: (x, y, z),
        }
    }
}

/// Example reference-counted object
struct RefCountedResource {
    ref_count: AtomicU32,
    data: String,
}

impl RefCountedResource {
    fn new(data: &str) -> Self {
        Self {
            ref_count: AtomicU32::new(1),
            data: data.to_string(),
        }
    }
}

impl RefCountable for RefCountedResource {
    fn add_ref(&self) {
        let old_count = self.ref_count.fetch_add(1, Ordering::SeqCst);
        println!(
            "Added reference to '{}': {} -> {}",
            self.data,
            old_count,
            old_count + 1
        );
    }

    fn release(&self) -> bool {
        let old_count = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        println!(
            "Released reference to '{}': {} -> {}",
            self.data,
            old_count,
            old_count - 1
        );
        old_count == 1
    }

    fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }
}

fn main() {
    println!("Pointer Remapping Example");
    println!("=========================");

    // Create a pointer remapping instance
    let mut pointer_remap = PointerRemapClass::new();

    // Simulate a save/load scenario
    println!("\n1. Creating original objects (pre-save state):");

    let original_obj1 = Box::new(GameObject::new(1, "Player", 10.0, 20.0, 30.0));
    let original_obj2 = Box::new(GameObject::new(2, "Enemy", 100.0, 200.0, 300.0));
    let original_obj3 = Box::new(RefCountedResource::new("Texture.png"));

    println!(
        "   Original Object 1: {:p} - {:?}",
        original_obj1.as_ref(),
        original_obj1
    );
    println!(
        "   Original Object 2: {:p} - {:?}",
        original_obj2.as_ref(),
        original_obj2
    );
    println!(
        "   Original Resource: {:p} - {}",
        original_obj3.as_ref(),
        original_obj3.data
    );

    // Get memory addresses for the original objects
    let old_addr1 = original_obj1.as_ref() as *const GameObject as usize;
    let old_addr2 = original_obj2.as_ref() as *const GameObject as usize;
    let old_addr3 = original_obj3.as_ref() as *const RefCountedResource as usize;

    println!("\n2. Creating new objects (post-load state):");

    // Simulate loading new objects with different addresses
    let new_obj1 = Box::new(GameObject::new(1, "Player", 10.0, 20.0, 30.0));
    let new_obj2 = Box::new(GameObject::new(2, "Enemy", 100.0, 200.0, 300.0));
    let new_obj3 = Box::new(RefCountedResource::new("Texture.png"));

    println!("   New Object 1: {:p} - {:?}", new_obj1.as_ref(), new_obj1);
    println!("   New Object 2: {:p} - {:?}", new_obj2.as_ref(), new_obj2);
    println!(
        "   New Resource: {:p} - {}",
        new_obj3.as_ref(),
        new_obj3.data
    );

    let new_addr1 = new_obj1.as_ref() as *const GameObject as usize;
    let new_addr2 = new_obj2.as_ref() as *const GameObject as usize;
    let new_addr3 = new_obj3.as_ref() as *const RefCountedResource as usize;

    println!("\n3. Registering pointer mappings:");

    // Register the mappings between old and new addresses
    pointer_remap.register_pointer(old_addr1, new_addr1);
    pointer_remap.register_pointer(old_addr2, new_addr2);
    pointer_remap.register_pointer(old_addr3, new_addr3);

    println!(
        "   Registered mapping: 0x{:x} -> 0x{:x}",
        old_addr1, new_addr1
    );
    println!(
        "   Registered mapping: 0x{:x} -> 0x{:x}",
        old_addr2, new_addr2
    );
    println!(
        "   Registered mapping: 0x{:x} -> 0x{:x}",
        old_addr3, new_addr3
    );

    // Show statistics
    let stats = pointer_remap.get_statistics();
    println!("\n4. Pointer remap statistics:");
    println!("   Registered pairs: {}", stats.registered_pairs);
    println!(
        "   Pending regular requests: {}",
        stats.pending_regular_requests
    );
    println!(
        "   Pending refcount requests: {}",
        stats.pending_refcount_requests
    );
    println!("   Needs sorting: {}", stats.needs_sorting);

    println!("\n5. Simulating pointer remapping requests:");

    // Simulate some pointers that need to be remapped
    let mut remapped_pointers: Vec<usize> = Vec::new();

    // Request remapping for the first object
    let remapped_pointers_1 = Arc::new(std::sync::Mutex::new(&mut remapped_pointers));
    let remapped_pointers_clone = Arc::clone(&remapped_pointers_1);

    #[cfg(debug_assertions)]
    pointer_remap.request_pointer_remap(
        old_addr1 as *const GameObject,
        move |new_ptr| {
            println!(
                "   Regular pointer remapped: 0x{:x} -> 0x{:x}",
                old_addr1, new_ptr as usize
            );
            // In a real scenario, you would update the actual pointer here
        },
        "example.rs",
        100,
    );

    #[cfg(not(debug_assertions))]
    pointer_remap.request_pointer_remap(old_addr1 as *const GameObject, move |new_ptr| {
        println!(
            "   Regular pointer remapped: 0x{:x} -> 0x{:x}",
            old_addr1, new_ptr as usize
        );
        // In a real scenario, you would update the actual pointer here
    });

    // Request remapping for the reference-counted object
    #[cfg(debug_assertions)]
    pointer_remap.request_ref_counted_pointer_remap(
        old_addr3 as *const RefCountedResource,
        move |new_ptr| {
            println!(
                "   Ref-counted pointer remapped: 0x{:x} -> 0x{:x}",
                old_addr3, new_ptr as usize
            );
            // The reference count will be automatically incremented
        },
        "example.rs",
        110,
    );

    #[cfg(not(debug_assertions))]
    pointer_remap.request_ref_counted_pointer_remap(
        old_addr3 as *const RefCountedResource,
        move |new_ptr| {
            println!(
                "   Ref-counted pointer remapped: 0x{:x} -> 0x{:x}",
                old_addr3, new_ptr as usize
            );
            // The reference count will be automatically incremented
        },
    );

    println!("\n6. Processing remap requests:");

    // Process all pending remap requests
    match pointer_remap.process() {
        Ok(()) => {
            println!("   All pointer remapping completed successfully!");
        }
        Err(e) => {
            println!("   Error during pointer remapping: {}", e);
        }
    }

    // Show final statistics
    let final_stats = pointer_remap.get_statistics();
    println!("\n7. Final statistics:");
    println!("   Registered pairs: {}", final_stats.registered_pairs);
    println!(
        "   Pending regular requests: {}",
        final_stats.pending_regular_requests
    );
    println!(
        "   Pending refcount requests: {}",
        final_stats.pending_refcount_requests
    );
    println!("   Needs sorting: {}", final_stats.needs_sorting);

    println!("\n8. Testing direct mapping lookups:");

    // Test direct mapping lookups
    for &old_addr in &[old_addr1, old_addr2, old_addr3] {
        if pointer_remap.has_mapping(old_addr) {
            if let Some(new_addr) = pointer_remap.get_mapping(old_addr) {
                println!("   Direct lookup: 0x{:x} -> 0x{:x}", old_addr, new_addr);
            }
        } else {
            println!("   No mapping found for: 0x{:x}", old_addr);
        }
    }

    // Test a non-existent mapping
    let fake_addr = 0xDEADBEEF;
    if !pointer_remap.has_mapping(fake_addr) {
        println!(
            "   No mapping found for fake address: 0x{:x} (expected)",
            fake_addr
        );
    }

    println!("\n9. Demonstrating Arc integration:");

    // Demonstrate safe Arc-based usage
    let arc_obj = Arc::new(GameObject::new(99, "Arc Object", 1.0, 2.0, 3.0));
    let arc_addr = Arc::as_ptr(&arc_obj) as usize;

    println!("   Arc object at: 0x{:x}", arc_addr);

    // Register and look up Arc pointer
    let new_arc_addr = arc_addr + 1000; // Simulate different address
    pointer_remap.register_pointer(arc_addr, new_arc_addr);

    if let Some(mapped_addr) = pointer_remap.get_mapping(arc_addr) {
        println!("   Arc mapping: 0x{:x} -> 0x{:x}", arc_addr, mapped_addr);
    }

    println!("\nExample completed successfully!");
}
