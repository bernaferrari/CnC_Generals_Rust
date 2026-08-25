//! W3DView shim → GameClient tactical `View::shake`.

pub use game_client::display::view::{
    CameraShakeType, Point3, with_tactical_view, with_tactical_view_ref,
};

/// C++ `W3DView::shake` / `TheTacticalView->shake`.
pub fn shake(epicenter: &Point3, shake_type: CameraShakeType) {
    with_tactical_view(|view| {
        view.shake(epicenter, shake_type);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake_forwards_to_tactical_view() {
        with_tactical_view(|view| {
            view.set_position(&Point3::new(0.0, 0.0, 0.0));
            view.reset_camera_shake();
        });
        shake(&Point3::new(0.0, 0.0, 0.0), CameraShakeType::Normal);
        let intensity = with_tactical_view_ref(|view| view.camera_shake_intensity());
        assert!(intensity > 0.0);
        with_tactical_view(|view| view.reset_camera_shake());
    }
}
