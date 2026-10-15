//! Unchanged public entry witnesses also run against the pre-fix implementation.
use super::*;

#[test]
fn ordinary_client_updates_do_not_advance_material_residual() {
    let mut receiver = BasicDrawable::new(DrawableId(42));
    receiver.set_second_material_pass_opacity(1.0);
    for frame in [1, 2, 2, 10] {
        receiver.update_after_client_modules(frame);
    }
    assert_eq!(
        receiver.capture_xfer_visuals().heat_vision_opacity,
        1.0,
        "ordinary CU cannot advance a Draw-owned residual"
    );
}

#[test]
fn core_hidden_draw_advances_before_visibility_return() {
    let mut receiver = BasicDrawable::new(DrawableId(42));
    receiver.set_visible(false);
    receiver.set_second_material_pass_opacity(1.0);
    receiver.render(&Matrix4::identity(), &Matrix4::identity());
    assert_eq!(
        receiver.capture_xfer_visuals().heat_vision_opacity,
        0.8,
        "actual hidden Draw advances before its visibility return"
    );
    receiver.render(&Matrix4::identity(), &Matrix4::identity());
    assert_eq!(
        receiver.capture_xfer_visuals().heat_vision_opacity,
        0.8 * 0.8
    );
}
