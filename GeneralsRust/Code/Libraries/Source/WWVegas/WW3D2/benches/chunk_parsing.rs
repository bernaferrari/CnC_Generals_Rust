//! Benchmarks for chunk parsing performance

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ww3d_rust::w3d_file::{W3dFile, W3dChunk, W3dChunkType};
use ww3d_rust::pluglib::chunk_io::{ChunkSaveClass, ChunkLoadClass};
use std::io::Cursor;

fn benchmark_chunk_parsing(c: &mut Criterion) {
    // Create sample chunk data
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    let mut chunk_writer = ChunkSaveClass::new(&mut cursor);
    
    // Create a sample mesh chunk with some data
    chunk_writer.begin_chunk(W3dChunkType::Mesh as u32).unwrap();
    
    // Write some dummy vertex data
    chunk_writer.begin_chunk(W3dChunkType::Vertices as u32).unwrap();
    for i in 0..1000 {
        chunk_writer.write(&(i as f32).to_le_bytes()).unwrap();
        chunk_writer.write(&(i as f32 * 2.0).to_le_bytes()).unwrap();
        chunk_writer.write(&(i as f32 * 3.0).to_le_bytes()).unwrap();
    }
    chunk_writer.end_chunk().unwrap();
    
    chunk_writer.end_chunk().unwrap();

    c.bench_function("parse_w3d_chunks", |b| {
        b.iter(|| {
            let cursor = Cursor::new(black_box(&buffer));
            let mut chunk_reader = ChunkLoadClass::new(cursor);
            
            while chunk_reader.open_chunk().unwrap_or(false) {
                let _chunk_id = chunk_reader.cur_chunk_id();
                let _chunk_size = chunk_reader.cur_chunk_length();
                chunk_reader.close_chunk().unwrap();
            }
        });
    });

    c.bench_function("create_w3d_file", |b| {
        b.iter(|| {
            let _w3d_file = W3dFile::from_bytes(black_box(&buffer)).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_chunk_parsing);
criterion_main!(benches);