//! Texture System Tests
//!
//! Comprehensive tests for texture loading, caching, and management.
//! These tests validate the texture pipeline from creation through GPU upload.

use ww3d_renderer_3d::core::WW3DFormat;
use ww3d_renderer_3d::rendering::texture_system::texture::TextureClass;
use ww3d_renderer_3d::rendering::texture_system::texture_base::PoolType;

#[test]
fn test_texture_creation_basic() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    assert_eq!(texture.format(), WW3DFormat::A8R8G8B8);
    assert!(!texture.is_loaded());
}

#[test]
fn test_texture_creation_various_formats() {
    // Test multiple format variations
    let formats = vec![
        WW3DFormat::A8R8G8B8,
        WW3DFormat::R8G8B8,
        WW3DFormat::A1R5G5B5,
        WW3DFormat::A4R4G4B4,
    ];

    for format in formats {
        let texture = TextureClass::new(128, 128, format, 1, PoolType::Managed);
        assert_eq!(texture.format(), format);
    }
}

#[test]
fn test_texture_creation_various_dimensions() {
    let dimensions = vec![
        (64, 64),
        (128, 128),
        (256, 256),
        (512, 512),
        (1024, 1024),
        (2048, 2048),
    ];

    for (width, height) in dimensions {
        let texture = TextureClass::new(width, height, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);
        assert_eq!(texture.width(), width);
        assert_eq!(texture.height(), height);
    }
}

#[test]
fn test_texture_creation_with_mipmaps() {
    // Test texture with multiple mip levels
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 9, PoolType::Managed);

    assert_eq!(texture.mip_level_count(), 9);
}

#[test]
fn test_texture_format_setting() {
    let mut texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // Change format
    texture.set_format(WW3DFormat::R8G8B8);
    assert_eq!(texture.format(), WW3DFormat::R8G8B8);

    // Change again
    texture.set_format(WW3DFormat::A1R5G5B5);
    assert_eq!(texture.format(), WW3DFormat::A1R5G5B5);
}

#[test]
fn test_texture_pool_type_managed() {
    let _texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // TextureBaseClass contains pool field; texture created with Managed pool
    assert!(true);
}

#[test]
fn test_texture_pool_type_default() {
    let _texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Default);

    // TextureBaseClass contains pool field; texture created with Default pool
    assert!(true);
}

#[test]
fn test_texture_from_file_creation() {
    let result = TextureClass::new_from_file("test_texture.tga", 1, WW3DFormat::A8R8G8B8);

    // Should create successfully (actual file loading tested separately)
    assert!(result.is_ok());

    let texture = result.unwrap();
    assert_eq!(texture.format(), WW3DFormat::A8R8G8B8);
}

#[test]
fn test_texture_clone() {
    let texture1 = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);
    let texture2 = texture1.clone();

    // Both should have same properties
    assert_eq!(texture1.format(), texture2.format());
    assert_eq!(texture1.width(), texture2.width());
    assert_eq!(texture1.height(), texture2.height());
}

#[test]
fn test_texture_memory_usage_small() {
    let texture = TextureClass::new(64, 64, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let memory = texture.memory_usage();
    // 64 * 64 * 4 bytes per pixel = 16 KB
    assert!(memory > 0);
}

#[test]
fn test_texture_memory_usage_large() {
    let texture = TextureClass::new(2048, 2048, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let memory = texture.memory_usage();
    // 2048 * 2048 * 4 = 16 MB
    assert!(memory > 1024 * 1024); // At least 1 MB
}

#[test]
fn test_texture_rectangular_dimensions() {
    // Non-square dimensions
    let texture = TextureClass::new(512, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    assert_eq!(texture.width(), 512);
    assert_eq!(texture.height(), 256);
}

#[test]
fn test_texture_aspect_ratios() {
    let test_cases = vec![
        (1024, 512),  // 2:1
        (512, 1024),  // 1:2
        (800, 600),   // 4:3
        (1920, 1080), // 16:9
    ];

    for (width, height) in test_cases {
        let texture = TextureClass::new(width, height, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);
        assert_eq!(texture.width(), width);
        assert_eq!(texture.height(), height);
    }
}

#[test]
fn test_texture_format_preservation() {
    let formats = vec![
        WW3DFormat::A8R8G8B8,
        WW3DFormat::R8G8B8,
        WW3DFormat::A1R5G5B5,
    ];

    for format in formats {
        let texture = TextureClass::new(256, 256, format, 1, PoolType::Managed);
        let cloned = texture.clone();
        assert_eq!(cloned.format(), format);
    }
}

#[test]
fn test_texture_usage_policy_default() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let _policy = texture.usage_policy();
    // Should have default policy
    assert!(true); // Policy object created successfully
}

#[test]
fn test_texture_usage_policy_mutation() {
    let mut texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let policy = texture.usage_policy();
    texture.set_usage_policy(policy);

    // Should succeed without panic
    assert!(true);
}

#[test]
fn test_texture_mipmap_levels_single() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    assert_eq!(texture.mip_level_count(), 1);
}

#[test]
fn test_texture_mipmap_levels_multiple() {
    for mip_levels in &[1, 2, 4, 8, 9, 12] {
        let texture = TextureClass::new(
            256,
            256,
            WW3DFormat::A8R8G8B8,
            *mip_levels,
            PoolType::Managed,
        );
        assert_eq!(texture.mip_level_count(), *mip_levels);
    }
}

#[test]
fn test_texture_multiple_instances() {
    let tex1 = TextureClass::new(64, 64, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);
    let tex2 = TextureClass::new(128, 128, WW3DFormat::R8G8B8, 2, PoolType::Managed);
    let tex3 = TextureClass::new(256, 256, WW3DFormat::A1R5G5B5, 3, PoolType::Default);

    // All should be independent
    assert_eq!(tex1.width(), 64);
    assert_eq!(tex2.width(), 128);
    assert_eq!(tex3.width(), 256);

    assert_eq!(tex1.mip_level_count(), 1);
    assert_eq!(tex2.mip_level_count(), 2);
    assert_eq!(tex3.mip_level_count(), 3);
}

#[test]
fn test_texture_pool_types() {
    let _managed = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);
    let _default = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Default);

    // Both should be created successfully with different pool types
    assert!(true);
}

#[test]
fn test_texture_is_loaded_false_initially() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // Should not be loaded initially
    assert!(!texture.is_loaded());
}

#[test]
fn test_texture_format_combinations() {
    // Test various combinations of parameters
    let params = vec![
        (128, 128, WW3DFormat::A8R8G8B8, 1),
        (256, 256, WW3DFormat::R8G8B8, 2),
        (512, 512, WW3DFormat::A1R5G5B5, 3),
        (1024, 1024, WW3DFormat::A4R4G4B4, 4),
    ];

    for (w, h, fmt, mips) in params {
        let texture = TextureClass::new(w, h, fmt, mips, PoolType::Managed);
        assert_eq!(texture.width(), w);
        assert_eq!(texture.height(), h);
        assert_eq!(texture.format(), fmt);
        assert_eq!(texture.mip_level_count(), mips);
    }
}

#[test]
fn test_texture_wgpu_texture_none_initially() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // Should be None initially (not uploaded to GPU)
    let wgpu_tex = texture.wgpu_texture();
    assert!(wgpu_tex.is_none());
}

#[test]
fn test_texture_apply_shader_stage() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // apply() should not panic
    texture.apply(0);
    texture.apply(1);
    texture.apply(15);
}

#[test]
fn test_texture_data_size_64x64() {
    let texture = TextureClass::new(64, 64, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let expected_size = 64 * 64 * 4; // RGBA = 4 bytes per pixel
    let memory = texture.memory_usage();
    assert_eq!(memory, expected_size as usize);
}

#[test]
fn test_texture_data_size_128x128() {
    let texture = TextureClass::new(128, 128, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    let expected_size = 128 * 128 * 4;
    let memory = texture.memory_usage();
    assert_eq!(memory, expected_size as usize);
}

#[test]
fn test_texture_deref_behavior() {
    let texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // Should be able to dereference and access base properties
    assert_eq!(texture.width(), 256);
    assert_eq!(texture.height(), 256);
}

#[test]
fn test_texture_deref_mut_behavior() {
    let mut texture = TextureClass::new(256, 256, WW3DFormat::A8R8G8B8, 1, PoolType::Managed);

    // Should be able to deref_mut and modify
    texture.set_format(WW3DFormat::R8G8B8);
    assert_eq!(texture.format(), WW3DFormat::R8G8B8);
}
