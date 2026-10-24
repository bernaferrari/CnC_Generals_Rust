//! Receiving client's RadiusDecal resources. Construction is inert; handles
//! remain module-owned and are invalidated when this client lifetime ends.
#[derive(Default)]
pub(super) struct NativeProjectedDecals {
    pub(super) manager: crate::radius_decal::ProjectedShadowManager,
}
impl Drop for NativeProjectedDecals {
    fn drop(&mut self) {
        self.manager.clear();
    }
}
