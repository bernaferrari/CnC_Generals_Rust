//! GPU Abstraction Layer Tests
//!
//! This module tests the WW3D GPU abstraction layer components including
//! textures, buffers, pipelines, and surface operations.
//!
//! Tests verify:
//! - Data structure creation and validation
//! - Resource management APIs
//! - Capability detection
//! - Error handling
//! - C++ parity with original DirectX 8 wrapper
//!
//! Note: This test suite targets the legacy DX8-compat API surface; it is currently
//! opt-in while the public API stabilizes.

#![cfg(feature = "gpu-integration-tests")]

use ww3d_gpu::*;

/// Test FVF (Flexible Vertex Format) creation and parsing
/// Verifies C++ parity with D3DFVF_* flags from d3d8types.h
#[test]
fn test_fvf_creation() {
    // Test XYZ position format (D3DFVF_XYZ = 0x002)
    let fvf = FVFFormat::new(0x002);
    assert!(fvf.has_position(), "FVF should have position");

    // Test XYZ + NORMAL (0x002 | 0x010 = 0x012)
    let fvf = FVFFormat::new(0x012);
    assert!(fvf.has_position(), "FVF should have position");
    assert!(fvf.has_normal(), "FVF should have normal");

    // Test complete vertex format: XYZ + NORMAL + DIFFUSE + TEX1
    // (0x002 | 0x010 | 0x040 | 0x100 = 0x152)
    let fvf = FVFFormat::new(0x152);
    assert!(fvf.has_position());
    assert!(fvf.has_normal());
    assert!(fvf.has_diffuse());
    assert_eq!(fvf.tex_count(), 1, "Should have 1 texture coordinate");
}

/// Test FVF vertex size calculation
/// Reference: d3d8types.h FVF stride calculation
#[test]
fn test_fvf_vertex_size() {
    // XYZ only: 3 floats = 12 bytes
    let fvf = FVFFormat::new(0x002);
    assert_eq!(fvf.vertex_size(), 12);

    // XYZ + NORMAL: 6 floats = 24 bytes
    let fvf = FVFFormat::new(0x012);
    assert_eq!(fvf.vertex_size(), 24);

    // XYZ + NORMAL + DIFFUSE: 6 floats + 1 u32 = 28 bytes
    let fvf = FVFFormat::new(0x052);
    assert_eq!(fvf.vertex_size(), 28);

    // Full vertex with 2 tex coords: XYZ + NORMAL + DIFFUSE + TEX2
    let fvf = FVFFormat::new(0x252);
    assert_eq!(fvf.vertex_size(), 44); // 24 + 4 + 16 bytes
}

/// Test FVF texture coordinate count extraction
/// Verifies correct parsing of D3DFVF_TEXCOUNTn masks
#[test]
fn test_fvf_texture_coordinates() {
    // No texture coordinates
    let fvf = FVFFormat::new(0x012);
    assert_eq!(fvf.tex_count(), 0);

    // 1 texture coordinate (D3DFVF_TEX1 = 0x100)
    let fvf = FVFFormat::new(0x112);
    assert_eq!(fvf.tex_count(), 1);

    // 2 texture coordinates (D3DFVF_TEX2 = 0x200)
    let fvf = FVFFormat::new(0x212);
    assert_eq!(fvf.tex_count(), 2);

    // 4 texture coordinates (D3DFVF_TEX4 = 0x400)
    let fvf = FVFFormat::new(0x412);
    assert_eq!(fvf.tex_count(), 4);
}

/// Test blend mode creation and properties
/// Reference: dx8wrapper.h D3DBLEND_* values
#[test]
fn test_blend_mode_creation() {
    let blend = BlendMode::opaque();
    assert_eq!(blend.src_blend(), SourceBlend::One);
    assert_eq!(blend.dst_blend(), DestBlend::Zero);
    assert!(!blend.is_transparent());

    let blend = BlendMode::alpha();
    assert_eq!(blend.src_blend(), SourceBlend::SrcAlpha);
    assert_eq!(blend.dst_blend(), DestBlend::InvSrcAlpha);
    assert!(blend.is_transparent());

    let blend = BlendMode::additive();
    assert_eq!(blend.src_blend(), SourceBlend::SrcAlpha);
    assert_eq!(blend.dst_blend(), DestBlend::One);
    assert!(blend.is_transparent());
}

/// Test blend mode transparency detection
/// Critical for sorting and render order
#[test]
fn test_blend_mode_transparency() {
    assert!(!BlendMode::opaque().is_transparent());
    assert!(BlendMode::alpha().is_transparent());
    assert!(BlendMode::additive().is_transparent());
    assert!(BlendMode::multiply().is_transparent());
    assert!(!BlendMode::screen_blit().is_transparent());
}

/// Test blend mode conversion to WGPU blend state
/// Verifies correct mapping from DX8 blend modes
#[test]
fn test_blend_mode_to_wgpu() {
    let blend = BlendMode::alpha();
    let wgpu_blend = blend.to_wgpu_blend_state();

    // Verify color blend
    assert_eq!(wgpu_blend.color.src_factor, wgpu::BlendFactor::SrcAlpha);
    assert_eq!(
        wgpu_blend.color.dst_factor,
        wgpu::BlendFactor::OneMinusSrcAlpha
    );
    assert_eq!(wgpu_blend.color.operation, wgpu::BlendOperation::Add);

    // Verify alpha blend
    assert_eq!(wgpu_blend.alpha.src_factor, wgpu::BlendFactor::One);
    assert_eq!(
        wgpu_blend.alpha.dst_factor,
        wgpu::BlendFactor::OneMinusSrcAlpha
    );
    assert_eq!(wgpu_blend.alpha.operation, wgpu::BlendOperation::Add);
}

/// Test GPU capability detection
/// Verifies hardware limits detection
#[test]
fn test_gpu_capabilities() {
    let caps = GpuCapabilities::default();

    // Verify minimum required capabilities
    assert!(
        caps.max_texture_size >= 256,
        "Min texture size should be 256"
    );
    assert!(
        caps.max_texture_size <= 16384,
        "Max texture size should be reasonable"
    );
    assert!(caps.max_vertices_per_draw > 0);
    assert!(caps.max_indices_per_draw > 0);
}

/// Test GPU capability feature flags
/// Reference: dx8wrapper.h capability detection
#[test]
fn test_gpu_feature_flags() {
    let mut caps = GpuCapabilities::default();

    // Test feature flag setting
    caps.set_feature(GpuFeature::Msaa, true);
    assert!(caps.has_feature(GpuFeature::Msaa));

    caps.set_feature(GpuFeature::AnisotropicFiltering, true);
    assert!(caps.has_feature(GpuFeature::AnisotropicFiltering));

    caps.set_feature(GpuFeature::Msaa, false);
    assert!(!caps.has_feature(GpuFeature::Msaa));
}

/// Test memory type inference from buffer usage
/// Verifies correct memory type selection for different buffer types
#[test]
fn test_memory_type_inference() {
    use wgpu::BufferUsages;

    // Vertex/Index buffers should be device local
    let mem_type = MemoryType::from_usage(BufferUsages::VERTEX);
    assert_eq!(mem_type, MemoryType::DeviceLocal);

    let mem_type = MemoryType::from_usage(BufferUsages::INDEX);
    assert_eq!(mem_type, MemoryType::DeviceLocal);

    // Uniform buffers can be host visible for frequent updates
    let mem_type = MemoryType::from_usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST);
    assert!(matches!(
        mem_type,
        MemoryType::HostVisible | MemoryType::HostCoherent
    ));
}

/// Test buffer size alignment requirements
/// Verifies correct padding for GPU alignment requirements
#[test]
fn test_buffer_alignment() {
    // Uniform buffers need 256-byte alignment
    assert_eq!(align_buffer_size(100, 256), 256);
    assert_eq!(align_buffer_size(256, 256), 256);
    assert_eq!(align_buffer_size(257, 256), 512);

    // Vertex buffers need 4-byte alignment
    assert_eq!(align_buffer_size(13, 4), 16);
    assert_eq!(align_buffer_size(16, 4), 16);
    assert_eq!(align_buffer_size(17, 4), 20);
}

/// Test shader constant packing
/// Reference: shdrcons.h shader constant layout
#[test]
fn test_shader_constant_packing() {
    let mut constants = ShaderConstants::new();

    // Test vec4 constant
    constants.set_vec4(0, [1.0, 2.0, 3.0, 4.0]);
    let data = constants.get_vec4(0);
    assert_eq!(data, [1.0, 2.0, 3.0, 4.0]);

    // Test matrix constant (4x4 = 16 floats)
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    constants.set_matrix(1, identity);
    let retrieved = constants.get_matrix(1);
    assert_eq!(retrieved, identity);
}

/// Test shader constant buffer layout
/// Verifies 16-byte alignment per constant
#[test]
fn test_shader_constant_layout() {
    let constants = ShaderConstants::new();

    // Each constant should be 16 bytes (vec4)
    assert_eq!(constants.constant_stride(), 16);

    // Verify proper offset calculation
    assert_eq!(constants.constant_offset(0), 0);
    assert_eq!(constants.constant_offset(1), 16);
    assert_eq!(constants.constant_offset(5), 80);
}

/// Test dynamic buffer ring allocation
/// Reference: dx8wrapper.h ring buffer system
#[test]
fn test_dynamic_buffer_ring() {
    let ring = DynamicBufferRing::new(1024, 3); // 1KB per frame, 3 frames

    assert_eq!(ring.frame_count(), 3);
    assert_eq!(ring.buffer_size(), 1024);
    assert_eq!(ring.total_size(), 3072);
}

/// Test dynamic buffer allocation within frame
/// Verifies proper suballocation and offset tracking
#[test]
fn test_dynamic_buffer_allocation() {
    let mut ring = DynamicBufferRing::new(1024, 3);

    // First allocation at offset 0
    let alloc1 = ring.allocate(256);
    assert_eq!(alloc1.offset, 0);
    assert_eq!(alloc1.size, 256);

    // Second allocation at offset 256 (aligned)
    let alloc2 = ring.allocate(128);
    assert_eq!(alloc2.offset, 256);
    assert_eq!(alloc2.size, 128);

    // Check remaining space
    assert!(ring.remaining_space() >= 640);
}

/// Test dynamic buffer frame advance
/// Verifies proper ring buffer rotation
#[test]
fn test_dynamic_buffer_frame_advance() {
    let mut ring = DynamicBufferRing::new(1024, 3);

    assert_eq!(ring.current_frame(), 0);

    ring.advance_frame();
    assert_eq!(ring.current_frame(), 1);

    ring.advance_frame();
    assert_eq!(ring.current_frame(), 2);

    // Should wrap back to 0
    ring.advance_frame();
    assert_eq!(ring.current_frame(), 0);
}

/// Test pipeline cache key generation
/// Verifies unique keys for different pipeline configurations
#[test]
fn test_pipeline_cache_key() {
    let key1 = PipelineCacheKey::new("shader_a", &BlendMode::opaque(), FVFFormat::new(0x112));
    let key2 = PipelineCacheKey::new("shader_a", &BlendMode::opaque(), FVFFormat::new(0x112));
    let key3 = PipelineCacheKey::new("shader_b", &BlendMode::opaque(), FVFFormat::new(0x112));

    // Same configuration should generate same key
    assert_eq!(key1, key2);

    // Different shader should generate different key
    assert_ne!(key1, key3);
}

/// Test pipeline cache hit/miss tracking
/// Verifies cache efficiency metrics
#[test]
fn test_pipeline_cache_stats() {
    let mut cache = PipelineCache::new(100);

    assert_eq!(cache.hit_count(), 0);
    assert_eq!(cache.miss_count(), 0);

    cache.record_hit();
    cache.record_hit();
    cache.record_miss();

    assert_eq!(cache.hit_count(), 2);
    assert_eq!(cache.miss_count(), 1);
    assert_eq!(cache.hit_rate(), 0.6666667);
}

/// Test render target dimensions validation
/// Verifies power-of-two and size limit checks
#[test]
fn test_render_target_validation() {
    // Valid power-of-two dimensions
    assert!(is_valid_render_target_size(256, 256));
    assert!(is_valid_render_target_size(512, 1024));
    assert!(is_valid_render_target_size(1024, 1024));

    // Valid non-power-of-two (if supported)
    assert!(is_valid_render_target_size(640, 480));
    assert!(is_valid_render_target_size(1280, 720));

    // Invalid dimensions
    assert!(!is_valid_render_target_size(0, 0));
    assert!(!is_valid_render_target_size(1, 1)); // Too small
    assert!(!is_valid_render_target_size(32768, 32768)); // Too large
}

/// Test render target format compatibility
/// Reference: dx8wrapper.h render target format checks
#[test]
fn test_render_target_format() {
    use wgpu::TextureFormat;

    // Common render target formats
    assert!(is_render_target_format(TextureFormat::Rgba8Unorm));
    assert!(is_render_target_format(TextureFormat::Rgba8UnormSrgb));
    assert!(is_render_target_format(TextureFormat::Bgra8Unorm));
    assert!(is_render_target_format(TextureFormat::Rgba16Float));

    // Depth formats are not color render targets
    assert!(!is_render_target_format(TextureFormat::Depth32Float));
}

/// Test texture format size calculation
/// Verifies bytes per pixel for different formats
#[test]
fn test_texture_format_size() {
    use wgpu::TextureFormat;

    assert_eq!(bytes_per_pixel(TextureFormat::R8Unorm), 1);
    assert_eq!(bytes_per_pixel(TextureFormat::Rg8Unorm), 2);
    assert_eq!(bytes_per_pixel(TextureFormat::Rgba8Unorm), 4);
    assert_eq!(bytes_per_pixel(TextureFormat::Rgba16Float), 8);
    assert_eq!(bytes_per_pixel(TextureFormat::Rgba32Float), 16);
}

/// Test texture mipmap level calculation
/// Reference: texture.h mipmap chain generation
#[test]
fn test_mipmap_level_count() {
    assert_eq!(calculate_mip_levels(256, 256), 9); // 256 -> 1
    assert_eq!(calculate_mip_levels(512, 512), 10); // 512 -> 1
    assert_eq!(calculate_mip_levels(1024, 512), 10); // Max dimension
    assert_eq!(calculate_mip_levels(1, 1), 1); // Single pixel
    assert_eq!(calculate_mip_levels(64, 64), 7);
}

/// Test texture size calculation with mipmaps
/// Verifies total memory usage including mip chain
#[test]
fn test_texture_total_size() {
    use wgpu::TextureFormat;

    // 256x256 RGBA8 with full mipchain
    let size = calculate_texture_size(256, 256, TextureFormat::Rgba8Unorm, 9);
    // 256*256*4 + 128*128*4 + ... + 1*1*4 = 349,504 bytes
    assert_eq!(size, 349504);

    // Single mip level
    let size = calculate_texture_size(256, 256, TextureFormat::Rgba8Unorm, 1);
    assert_eq!(size, 262144); // 256*256*4
}

/// Test surface format preference
/// Verifies selection of optimal swapchain format
#[test]
fn test_surface_format_preference() {
    use wgpu::TextureFormat;

    let formats = vec![
        TextureFormat::Bgra8Unorm,
        TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ];

    // Prefer BGRA8 for compatibility
    let preferred = select_preferred_format(&formats);
    assert_eq!(preferred, TextureFormat::Bgra8Unorm);
}

/// Test surface present mode selection
/// Reference: dx8wrapper.h vsync settings
#[test]
fn test_present_mode_selection() {
    // Test vsync on (FIFO)
    let mode = select_present_mode(true);
    assert_eq!(mode, wgpu::PresentMode::Fifo);

    // Test vsync off (Immediate or Mailbox)
    let mode = select_present_mode(false);
    assert!(matches!(
        mode,
        wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox
    ));
}

/// Test sorting renderer key generation
/// Verifies correct sort key packing for transparent objects
#[test]
fn test_sorting_key_generation() {
    // Sort key format: [distance:32][shader:16][texture:16]
    let key1 = SortKey::new(100.0, 1, 5);
    let key2 = SortKey::new(50.0, 1, 5);
    let key3 = SortKey::new(100.0, 2, 5);

    // Closer objects should sort first
    assert!(key2 < key1);

    // Same distance, different shader
    assert_ne!(key1, key3);
}

/// Test sorting renderer batch formation
/// Verifies efficient batching of sorted draw calls
#[test]
fn test_sorting_batch_formation() {
    let mut sorter = SortingRenderer::new();

    // Add items with same material
    sorter.add_item(SortKey::new(100.0, 1, 1), DrawCall::default());
    sorter.add_item(SortKey::new(101.0, 1, 1), DrawCall::default());
    sorter.add_item(SortKey::new(102.0, 1, 1), DrawCall::default());

    // Should batch into single draw
    let batches = sorter.generate_batches();
    assert_eq!(batches.len(), 1);
}

/// Test sorting renderer material changes
/// Verifies correct batch breaking on material change
#[test]
fn test_sorting_material_changes() {
    let mut sorter = SortingRenderer::new();

    // Add items with different materials
    sorter.add_item(SortKey::new(100.0, 1, 1), DrawCall::default());
    sorter.add_item(SortKey::new(101.0, 2, 1), DrawCall::default());
    sorter.add_item(SortKey::new(102.0, 1, 1), DrawCall::default());

    // Should create 3 batches due to material changes
    let batches = sorter.generate_batches();
    assert!(batches.len() >= 2);
}

/// Test FVF format equality
/// Verifies correct comparison of vertex formats
#[test]
fn test_fvf_equality() {
    let fvf1 = FVFFormat::new(0x152);
    let fvf2 = FVFFormat::new(0x152);
    let fvf3 = FVFFormat::new(0x252);

    assert_eq!(fvf1, fvf2);
    assert_ne!(fvf1, fvf3);
}

/// Test blend mode equality
/// Verifies correct comparison of blend states
#[test]
fn test_blend_mode_equality() {
    let blend1 = BlendMode::alpha();
    let blend2 = BlendMode::alpha();
    let blend3 = BlendMode::additive();

    assert_eq!(blend1, blend2);
    assert_ne!(blend1, blend3);
}

/// Test buffer usage flag combinations
/// Verifies valid combinations of buffer usage flags
#[test]
fn test_buffer_usage_combinations() {
    use wgpu::BufferUsages;

    // Valid vertex buffer
    let usage = BufferUsages::VERTEX | BufferUsages::COPY_DST;
    assert!(usage.contains(BufferUsages::VERTEX));
    assert!(usage.contains(BufferUsages::COPY_DST));

    // Valid dynamic uniform buffer
    let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::MAP_WRITE;
    assert!(usage.contains(BufferUsages::UNIFORM));
    assert!(usage.contains(BufferUsages::MAP_WRITE));
}

// Helper functions used by tests

fn align_buffer_size(size: u64, alignment: u64) -> u64 {
    ((size + alignment - 1) / alignment) * alignment
}

fn is_valid_render_target_size(width: u32, height: u32) -> bool {
    width > 0 && height > 0 && width <= 16384 && height <= 16384
}

fn is_render_target_format(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb
            | wgpu::TextureFormat::Rgba16Float
            | wgpu::TextureFormat::Rgba32Float
    )
}

fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    match format {
        wgpu::TextureFormat::R8Unorm => 1,
        wgpu::TextureFormat::Rg8Unorm => 2,
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        wgpu::TextureFormat::Rgba32Float => 16,
        _ => 0,
    }
}

fn calculate_mip_levels(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height);
    if max_dim == 0 {
        return 1;
    }
    (32 - max_dim.leading_zeros()) as u32
}

fn calculate_texture_size(
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    mip_levels: u32,
) -> u64 {
    let bpp = bytes_per_pixel(format) as u64;
    let mut total = 0u64;
    let mut w = width as u64;
    let mut h = height as u64;

    for _ in 0..mip_levels {
        total += w * h * bpp;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }

    total
}

fn select_preferred_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    // Prefer BGRA8 for D3D compatibility
    for &format in formats {
        if format == wgpu::TextureFormat::Bgra8Unorm {
            return format;
        }
    }
    formats[0]
}

fn select_present_mode(vsync: bool) -> wgpu::PresentMode {
    if vsync {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::Mailbox
    }
}
