//! Benchmarks for the WWSaveLoad system

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::{Arc, Weak};
use ww_save_load::saveload::*;

/// Simple benchmark object
#[derive(Default)]
struct BenchmarkObject {
    id: RemapId,
    data: [u32; 16], // Some data to make save/load meaningful
    counter: u64,
}

impl BenchmarkObject {
    fn new(id: RemapId, counter: u64) -> Self {
        Self {
            id,
            data: [counter as u32; 16],
            counter,
        }
    }
}

impl Persist for BenchmarkObject {
    fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
        chunk_save.write_value(&self.data)?;
        chunk_save.write_value(&self.counter)?;
        Ok(())
    }

    fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        self.data = chunk_load.read_value()?;
        self.counter = chunk_load.read_value()?;
        Ok(())
    }

    fn get_factory(&self) -> Arc<dyn PersistFactory> {
        Arc::new(SimplePersistFactory::<BenchmarkObject>::new(0x12340000))
    }

    fn get_remap_id(&self) -> RemapId {
        self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl PostLoadable for BenchmarkObject {
    fn on_post_load(&mut self) -> SaveLoadResult<()> {
        self.counter += 1;
        Ok(())
    }

    fn is_post_load_registered(&self) -> bool {
        false
    }

    fn set_post_load_registered(&mut self, _registered: bool) {
        // No-op for benchmark
    }
}

/// Mock implementations for benchmarking
struct BenchmarkChunkSave {
    data: Vec<u8>,
    chunk_stack: Vec<usize>,
}

impl BenchmarkChunkSave {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(1024 * 1024), // Pre-allocate 1MB
            chunk_stack: Vec::new(),
        }
    }
}

impl ChunkSave for BenchmarkChunkSave {
    fn begin_chunk(&mut self, chunk_id: ChunkId) -> SaveLoadResult<()> {
        self.chunk_stack.push(self.data.len());
        self.data.extend_from_slice(&chunk_id.to_le_bytes());
        self.data.extend_from_slice(&0u32.to_le_bytes()); // Placeholder for size
        Ok(())
    }

    fn end_chunk(&mut self) -> SaveLoadResult<()> {
        if let Some(start_pos) = self.chunk_stack.pop() {
            let chunk_size = (self.data.len() - start_pos - 8) as u32;
            let size_bytes = chunk_size.to_le_bytes();
            self.data[start_pos + 4..start_pos + 8].copy_from_slice(&size_bytes);
            Ok(())
        } else {
            Err(SaveLoadError::General("No chunk to end".to_string()))
        }
    }

    fn write(&mut self, data: &[u8]) -> SaveLoadResult<()> {
        self.data.extend_from_slice(data);
        Ok(())
    }
}

struct BenchmarkChunkLoad {
    data: Vec<u8>,
    position: usize,
    chunk_stack: Vec<(ChunkId, usize)>, // (chunk_id, end_position)
}

impl BenchmarkChunkLoad {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: 0,
            chunk_stack: Vec::new(),
        }
    }
}

impl ChunkLoad for BenchmarkChunkLoad {
    fn open_chunk(&mut self) -> SaveLoadResult<bool> {
        if self.position + 8 > self.data.len() {
            return Ok(false);
        }

        let chunk_id = u32::from_le_bytes([
            self.data[self.position],
            self.data[self.position + 1],
            self.data[self.position + 2],
            self.data[self.position + 3],
        ]);

        let chunk_size = u32::from_le_bytes([
            self.data[self.position + 4],
            self.data[self.position + 5],
            self.data[self.position + 6],
            self.data[self.position + 7],
        ]) as usize;

        self.position += 8;
        let end_position = self.position + chunk_size;
        self.chunk_stack.push((chunk_id, end_position));

        Ok(true)
    }

    fn close_chunk(&mut self) -> SaveLoadResult<()> {
        if let Some((_, end_position)) = self.chunk_stack.pop() {
            self.position = end_position;
            Ok(())
        } else {
            Err(SaveLoadError::General("No chunk to close".to_string()))
        }
    }

    fn current_chunk_id(&self) -> ChunkId {
        self.chunk_stack.last().map_or(0, |(id, _)| *id)
    }

    fn read(&mut self, buffer: &mut [u8]) -> SaveLoadResult<usize> {
        if let Some((_, end_position)) = self.chunk_stack.last() {
            let available = end_position.saturating_sub(self.position);
            let to_read = buffer.len().min(available);

            if to_read > 0 {
                buffer[..to_read]
                    .copy_from_slice(&self.data[self.position..self.position + to_read]);
                self.position += to_read;
            }

            Ok(to_read)
        } else {
            Err(SaveLoadError::General("No chunk open".to_string()))
        }
    }
}

fn benchmark_save_single_object(c: &mut Criterion) {
    let obj = BenchmarkObject::new(12345, 42);
    let factory = SimplePersistFactory::<BenchmarkObject>::new(0x12340000);

    c.bench_function("save_single_object", |b| {
        b.iter(|| {
            let mut chunk_save = BenchmarkChunkSave::new();
            black_box(factory.save(&mut chunk_save, black_box(&obj)).unwrap());
        });
    });
}

fn benchmark_load_single_object(c: &mut Criterion) {
    let obj = BenchmarkObject::new(12345, 42);
    let factory = SimplePersistFactory::<BenchmarkObject>::new(0x12340000);

    // Create saved data
    let mut chunk_save = BenchmarkChunkSave::new();
    factory.save(&mut chunk_save, &obj).unwrap();
    let saved_data = chunk_save.data.clone();

    c.bench_function("load_single_object", |b| {
        b.iter(|| {
            let mut chunk_load = BenchmarkChunkLoad::new(saved_data.clone());
            black_box(factory.load(black_box(&mut chunk_load)).unwrap());
        });
    });
}

fn benchmark_pointer_remapping(c: &mut Criterion) {
    c.bench_function("pointer_remapping_1000_objects", |b| {
        b.iter(|| {
            let mut remapper = PointerRemap::new();
            let objects: Vec<Arc<BenchmarkObject>> = (0..1000)
                .map(|i| Arc::new(BenchmarkObject::new(i, i as u64)))
                .collect();

            // Register all pointers
            for obj in &objects {
                let remap_id = obj.id;
                let obj_dyn: Arc<dyn Persist> = obj.clone();
                let weak: Weak<dyn Persist> = Arc::downgrade(&obj_dyn);
                remapper.register_pointer(remap_id, weak);
            }

            // Request remapping for all objects
            for obj in &objects {
                let obj_id = obj.id;
                remapper.request_pointer_remap(obj_id, move |_remapped| Ok(()));
            }

            // Process all requests
            black_box(remapper.process().unwrap());
        });
    });
}

fn benchmark_save_load_system_registration(c: &mut Criterion) {
    c.bench_function("factory_registration_lookup", |b| {
        b.iter(|| {
            let system = SaveLoadSystem::new();

            // Register 100 factories
            for i in 0..100 {
                let factory =
                    Arc::new(SimplePersistFactory::<BenchmarkObject>::new(0x12340000 + i));
                system.register_persist_factory(factory);
            }

            // Look up all factories
            for i in 0..100 {
                black_box(system.find_persist_factory(0x12340000 + i));
            }
        });
    });
}

fn benchmark_save_multiple_objects(c: &mut Criterion) {
    let objects: Vec<BenchmarkObject> = (0..1000)
        .map(|i| BenchmarkObject::new(i, i as u64))
        .collect();
    let factory = SimplePersistFactory::<BenchmarkObject>::new(0x12340000);

    c.bench_function("save_1000_objects", |b| {
        b.iter(|| {
            let mut chunk_save = BenchmarkChunkSave::new();
            for obj in black_box(&objects) {
                black_box(factory.save(&mut chunk_save, obj).unwrap());
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_save_single_object,
    benchmark_load_single_object,
    benchmark_pointer_remapping,
    benchmark_save_load_system_registration,
    benchmark_save_multiple_objects
);
criterion_main!(benches);
