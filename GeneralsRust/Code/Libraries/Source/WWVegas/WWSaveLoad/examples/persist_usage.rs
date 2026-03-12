//! Example demonstrating usage of the persist module for object persistence

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ww_save_load::saveload::{SaveLoadError, SaveLoadResult};
use ww_save_load::{
    get_save_load_system, ChunkLoad, ChunkLoadExt, ChunkSave, ChunkSaveExt, Persist,
    PersistFactory, PersistFactoryRegistry, PostLoadable, RemapId, SimplePersistFactory,
};

/// Example game entity that can be persisted
#[derive(Debug, Default)]
struct GameEntity {
    id: RemapId,
    position: (f32, f32, f32),
    health: i32,
    name: String,
    post_load_registered: bool,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl GameEntity {
    pub fn new(name: String, position: (f32, f32, f32), health: i32) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            position,
            health,
            name,
            post_load_registered: false,
        }
    }
}

impl PostLoadable for GameEntity {
    fn on_post_load(&mut self) -> SaveLoadResult<()> {
        println!(
            "Post-load processing for entity '{}' at {:?}",
            self.name, self.position
        );

        // Example validation
        if self.health < 0 {
            return Err(SaveLoadError::PostLoadError(format!(
                "Entity '{}' has invalid health: {}",
                self.name, self.health
            )));
        }

        Ok(())
    }

    fn is_post_load_registered(&self) -> bool {
        self.post_load_registered
    }

    fn set_post_load_registered(&mut self, registered: bool) {
        self.post_load_registered = registered;
    }
}

impl Persist for GameEntity {
    fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
        println!("Saving entity '{}' with ID {}", self.name, self.id);

        // Save entity data
        chunk_save.write_value(&self.position.0)?;
        chunk_save.write_value(&self.position.1)?;
        chunk_save.write_value(&self.position.2)?;
        chunk_save.write_value(&self.health)?;

        // Save string name
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len() as u32;
        chunk_save.write_value(&name_len)?;
        chunk_save.write(name_bytes)?;

        Ok(())
    }

    fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        // Load entity data
        self.position.0 = chunk_load.read_value()?;
        self.position.1 = chunk_load.read_value()?;
        self.position.2 = chunk_load.read_value()?;
        self.health = chunk_load.read_value()?;

        // Load string name
        let name_len: u32 = chunk_load.read_value()?;
        let mut name_bytes = vec![0u8; name_len as usize];
        chunk_load.read(&mut name_bytes)?;
        self.name = String::from_utf8(name_bytes)
            .map_err(|e| SaveLoadError::General(format!("Invalid UTF-8 in name: {}", e)))?;

        println!("Loaded entity '{}' with ID {}", self.name, self.id);
        Ok(())
    }

    fn get_factory(&self) -> Arc<dyn PersistFactory> {
        Arc::new(SimplePersistFactory::<GameEntity>::new(0x47414D45)) // "GAME" in hex
    }

    fn get_remap_id(&self) -> RemapId {
        self.id
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Persist Module Usage Example ===");

    // Create some game entities
    let entity1 = GameEntity::new("Tank".to_string(), (100.0, 0.0, 200.0), 150);
    let entity2 = GameEntity::new("Infantry".to_string(), (50.0, 0.0, 75.0), 100);
    let entity3 = GameEntity::new("Aircraft".to_string(), (200.0, 500.0, 300.0), 80);

    println!("\n--- Created Entities ---");
    println!("Entity 1: {:?}", entity1);
    println!("Entity 2: {:?}", entity2);
    println!("Entity 3: {:?}", entity3);

    // Register factory with the save/load system
    let factory = Arc::new(SimplePersistFactory::<GameEntity>::new(0x47414D45));
    get_save_load_system().register_persist_factory(factory);
    println!("\n--- Registered factory for GameEntity (chunk ID: 0x47414D45) ---");

    // Demonstrate factory registry
    let registry = PersistFactoryRegistry::new();
    let factory_clone = Arc::new(SimplePersistFactory::<GameEntity>::new(0x47414D45));

    registry.register_factory(factory_clone)?;
    println!("Factory registered in local registry");

    // Check if factory exists
    if let Some(found_factory) = registry.find_factory(0x47414D45) {
        println!(
            "Found factory with chunk ID: 0x{:08X}",
            found_factory.chunk_id()
        );
    }

    println!("Registry contains {} factories", registry.factory_count());
    println!(
        "Registered chunk IDs: {:?}",
        registry.registered_chunk_ids()
    );

    // Demonstrate macro usage
    struct SimpleEntity {
        id: u64,
        post_load_registered: bool,
    }

    ww_save_load::impl_default_post_loadable!(SimpleEntity, post_load_registered);

    let mut simple = SimpleEntity {
        id: 1,
        post_load_registered: false,
    };
    println!("\n--- Macro Usage Example ---");
    println!(
        "Simple entity post-load registered: {}",
        simple.is_post_load_registered()
    );

    simple.set_post_load_registered(true);
    println!("After setting: {}", simple.is_post_load_registered());

    // Demonstrate post-load processing
    simple.on_post_load()?;
    println!("Post-load processing completed successfully");

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
