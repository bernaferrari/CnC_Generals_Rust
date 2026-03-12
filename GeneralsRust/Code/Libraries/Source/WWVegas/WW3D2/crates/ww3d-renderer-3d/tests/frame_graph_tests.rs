//! Frame Graph Tests
//!
//! Comprehensive tests for frame graph construction, pass management, and rendering orchestration.

use glam::{Mat4, Vec2, Vec3};
use ww3d_renderer_3d::rendering::frame_graph::{
    FrameGraphPass, FrameGraphPassContext, FrameGraphPassMask, FrameGraphPreparedQueues,
    FrameGraphQueue, PipelineHint,
};

#[test]
fn test_frame_graph_pass_main() {
    let pass = FrameGraphPass::Main;

    // Should create main pass successfully
    assert_eq!(pass, FrameGraphPass::Main);
}

#[test]
fn test_frame_graph_pass_shadow() {
    let pass = FrameGraphPass::Shadow(0);

    // Should create shadow pass for light 0
    assert_eq!(pass, FrameGraphPass::Shadow(0));
}

#[test]
fn test_frame_graph_pass_shadow_multiple() {
    let pass1 = FrameGraphPass::Shadow(0);
    let pass2 = FrameGraphPass::Shadow(1);
    let pass3 = FrameGraphPass::Shadow(2);

    // All should be distinct
    assert_ne!(pass1, pass2);
    assert_ne!(pass2, pass3);
    assert_ne!(pass1, pass3);
}

#[test]
fn test_frame_graph_pass_reflection() {
    let pass = FrameGraphPass::Reflection;

    // Should create reflection pass
    assert_eq!(pass, FrameGraphPass::Reflection);
}

#[test]
fn test_frame_graph_pass_custom() {
    let pass = FrameGraphPass::Custom(42);

    // Should create custom pass with ID
    assert_eq!(pass, FrameGraphPass::Custom(42));
}

#[test]
fn test_frame_graph_queue_opaque() {
    let queue = FrameGraphQueue::Opaque;

    // Should create opaque queue
    assert_eq!(queue, FrameGraphQueue::Opaque);
}

#[test]
fn test_frame_graph_queue_alpha() {
    let queue = FrameGraphQueue::Alpha;

    // Should create alpha queue
    assert_eq!(queue, FrameGraphQueue::Alpha);
}

#[test]
fn test_frame_graph_queue_additive() {
    let queue = FrameGraphQueue::Additive;

    // Should create additive queue
    assert_eq!(queue, FrameGraphQueue::Additive);
}

#[test]
fn test_frame_graph_queue_decal() {
    let queue = FrameGraphQueue::Decal;

    // Should create decal queue
    assert_eq!(queue, FrameGraphQueue::Decal);
}

#[test]
fn test_frame_graph_queue_shadow_caster() {
    let queue = FrameGraphQueue::ShadowCaster;

    // Should create shadow caster queue
    assert_eq!(queue, FrameGraphQueue::ShadowCaster);
}

#[test]
fn test_frame_graph_pass_mask_opaque() {
    let mask = FrameGraphPassMask::OPAQUE;

    // Should have opaque bit set
    assert!(mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(!mask.contains(FrameGraphPassMask::ALPHA));
}

#[test]
fn test_frame_graph_pass_mask_alpha() {
    let mask = FrameGraphPassMask::ALPHA;

    // Should have alpha bit set
    assert!(mask.contains(FrameGraphPassMask::ALPHA));
    assert!(!mask.contains(FrameGraphPassMask::OPAQUE));
}

#[test]
fn test_frame_graph_pass_mask_combination() {
    let mask = FrameGraphPassMask::OPAQUE | FrameGraphPassMask::ALPHA;

    // Both bits should be set
    assert!(mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(mask.contains(FrameGraphPassMask::ALPHA));
}

#[test]
fn test_frame_graph_pass_mask_all() {
    let mask = FrameGraphPassMask::OPAQUE
        | FrameGraphPassMask::ALPHA
        | FrameGraphPassMask::ADDITIVE
        | FrameGraphPassMask::DECAL
        | FrameGraphPassMask::SHADOW_CASTER;

    // All bits should be set
    assert!(mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(mask.contains(FrameGraphPassMask::ALPHA));
    assert!(mask.contains(FrameGraphPassMask::ADDITIVE));
    assert!(mask.contains(FrameGraphPassMask::DECAL));
    assert!(mask.contains(FrameGraphPassMask::SHADOW_CASTER));
}

#[test]
fn test_frame_graph_pass_mask_empty() {
    let mask = FrameGraphPassMask::empty();

    // All bits should be unset
    assert!(!mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(!mask.contains(FrameGraphPassMask::ALPHA));
}

#[test]
fn test_pipeline_hint_creation() {
    let hint = PipelineHint {
        shader_signature: 0x12345678,
        pass_count: 3,
        is_skinned: true,
    };

    assert_eq!(hint.shader_signature, 0x12345678);
    assert_eq!(hint.pass_count, 3);
    assert!(hint.is_skinned);
}

#[test]
fn test_pipeline_hint_default() {
    let hint = PipelineHint::default();

    // Default should be zero values
    assert_eq!(hint.shader_signature, 0);
    assert_eq!(hint.pass_count, 0);
    assert!(!hint.is_skinned);
}

#[test]
fn test_frame_graph_pass_context_creation() {
    let view = Mat4::IDENTITY;
    let projection = Mat4::IDENTITY;
    let jitter = Vec2::ZERO;

    let context = FrameGraphPassContext::new(FrameGraphPass::Main, view, projection, jitter);

    assert_eq!(context.pass, FrameGraphPass::Main);
    assert_eq!(context.view, view);
    assert_eq!(context.projection, projection);
    assert_eq!(context.jitter, jitter);
    assert_eq!(context.view_projection, projection * view);
}

#[test]
fn test_frame_graph_pass_context_view_projection_computation() {
    let view = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let projection = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));

    let context = FrameGraphPassContext::new(FrameGraphPass::Main, view, projection, Vec2::ZERO);

    // Verify view_projection is computed correctly
    let expected = projection * view;
    assert_eq!(context.view_projection, expected);
}

#[test]
fn test_frame_graph_pass_context_shadow_pass() {
    let view = Mat4::IDENTITY;
    let projection = Mat4::IDENTITY;

    let context =
        FrameGraphPassContext::new(FrameGraphPass::Shadow(0), view, projection, Vec2::ZERO);

    assert_eq!(context.pass, FrameGraphPass::Shadow(0));
}

#[test]
fn test_frame_graph_pass_context_with_jitter() {
    let jitter = Vec2::new(0.5, 0.5);

    let context =
        FrameGraphPassContext::new(FrameGraphPass::Main, Mat4::IDENTITY, Mat4::IDENTITY, jitter);

    assert_eq!(context.jitter, jitter);
}

#[test]
fn test_frame_graph_prepared_queues_creation() {
    let queues = FrameGraphPreparedQueues::default();

    // All queues should be empty initially
    assert_eq!(queues.opaque.len(), 0);
    assert_eq!(queues.alpha.len(), 0);
    assert_eq!(queues.additive.len(), 0);
    assert_eq!(queues.decals.len(), 0);
    assert_eq!(queues.shadow_casters.len(), 0);
}

#[test]
fn test_frame_graph_prepared_queues_combined_translucent() {
    let mut queues = FrameGraphPreparedQueues::default();

    // Prepare some alpha meshes (would normally be Arc<MeshClass>)
    // For testing, we just verify the method works with empty queues
    let combined = queues.combined_translucent();
    assert_eq!(combined.len(), 0);
}

#[test]
fn test_frame_graph_pass_equality() {
    let pass1 = FrameGraphPass::Main;
    let pass2 = FrameGraphPass::Main;

    // Same passes should be equal
    assert_eq!(pass1, pass2);
}

#[test]
fn test_frame_graph_pass_inequality() {
    let pass1 = FrameGraphPass::Main;
    let pass2 = FrameGraphPass::Reflection;

    // Different passes should not be equal
    assert_ne!(pass1, pass2);
}

#[test]
fn test_frame_graph_queue_equality() {
    let queue1 = FrameGraphQueue::Opaque;
    let queue2 = FrameGraphQueue::Opaque;

    assert_eq!(queue1, queue2);
}

#[test]
fn test_frame_graph_queue_inequality() {
    let queue1 = FrameGraphQueue::Opaque;
    let queue2 = FrameGraphQueue::Alpha;

    assert_ne!(queue1, queue2);
}

#[test]
fn test_frame_graph_pass_hash() {
    use std::collections::HashSet;

    let mut passes = HashSet::new();
    passes.insert(FrameGraphPass::Main);
    passes.insert(FrameGraphPass::Reflection);
    passes.insert(FrameGraphPass::Shadow(0));

    // All should be distinct in hash set
    assert_eq!(passes.len(), 3);
}

#[test]
fn test_frame_graph_queue_hash() {
    use std::collections::HashSet;

    let mut queues = HashSet::new();
    queues.insert(FrameGraphQueue::Opaque);
    queues.insert(FrameGraphQueue::Alpha);
    queues.insert(FrameGraphQueue::Additive);

    // All should be distinct
    assert_eq!(queues.len(), 3);
}

#[test]
fn test_frame_graph_pass_mask_bits_independent() {
    let opaque = FrameGraphPassMask::OPAQUE;
    let alpha = FrameGraphPassMask::ALPHA;

    // Bits should not overlap
    let combined = opaque | alpha;
    assert_ne!(combined, opaque);
    assert_ne!(combined, alpha);
}

#[test]
fn test_frame_graph_pass_context_clone() {
    let context1 = FrameGraphPassContext::new(
        FrameGraphPass::Main,
        Mat4::IDENTITY,
        Mat4::IDENTITY,
        Vec2::ZERO,
    );

    let context2 = context1.clone();

    assert_eq!(context1.pass, context2.pass);
    assert_eq!(context1.view, context2.view);
}

#[test]
fn test_frame_graph_pass_mask_iteration() {
    let mask =
        FrameGraphPassMask::OPAQUE | FrameGraphPassMask::ALPHA | FrameGraphPassMask::ADDITIVE;

    // Should be able to check each flag
    assert!(mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(mask.contains(FrameGraphPassMask::ALPHA));
    assert!(mask.contains(FrameGraphPassMask::ADDITIVE));
    assert!(!mask.contains(FrameGraphPassMask::DECAL));
}

#[test]
fn test_pipeline_hint_clone() {
    let hint1 = PipelineHint {
        shader_signature: 0xABCD,
        pass_count: 5,
        is_skinned: true,
    };

    let hint2 = hint1;

    assert_eq!(hint1.shader_signature, hint2.shader_signature);
    assert_eq!(hint1.pass_count, hint2.pass_count);
    assert_eq!(hint1.is_skinned, hint2.is_skinned);
}

#[test]
fn test_frame_graph_multiple_shadow_passes() {
    let passes: Vec<FrameGraphPass> = (0..8).map(|i| FrameGraphPass::Shadow(i)).collect();

    // Should create 8 distinct shadow passes
    assert_eq!(passes.len(), 8);
    for i in 0..8 {
        assert_eq!(passes[i], FrameGraphPass::Shadow(i as u32));
    }
}

#[test]
fn test_frame_graph_pass_mask_clear() {
    let mut mask = FrameGraphPassMask::OPAQUE | FrameGraphPassMask::ALPHA;

    // Clear alpha
    mask &= !FrameGraphPassMask::ALPHA;

    assert!(mask.contains(FrameGraphPassMask::OPAQUE));
    assert!(!mask.contains(FrameGraphPassMask::ALPHA));
}

#[test]
fn test_frame_graph_prepared_queues_default() {
    let queues = FrameGraphPreparedQueues::default();

    // Should match default initialization
    assert_eq!(queues.opaque.len(), 0);
    assert_eq!(queues.alpha.len(), 0);
    assert_eq!(queues.additive.len(), 0);
    assert_eq!(queues.decals.len(), 0);
    assert_eq!(queues.shadow_casters.len(), 0);
}
