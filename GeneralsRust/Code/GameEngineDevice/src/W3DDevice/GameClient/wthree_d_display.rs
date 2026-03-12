//! W3D display system (port of W3DDisplay).
//!
//! Owns the global scene used by draw modules and provides access to it.

use crate::W3DDevice::GameClient::wthree_d_scene::W3DScene;
use crate::W3DDevice::GameClient::wthree_d_view::W3DView;
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};

pub struct W3DDisplay {
    scene: Arc<RwLock<W3DScene>>,
    view: RwLock<Option<W3DView>>,
}

impl W3DDisplay {
    pub fn new() -> Self {
        Self {
            scene: Arc::new(RwLock::new(W3DScene::new())),
            view: RwLock::new(None),
        }
    }

    pub fn scene(&self) -> Arc<RwLock<W3DScene>> {
        Arc::clone(&self.scene)
    }

    pub fn set_view(&self, view: W3DView) {
        *self.view.write() = Some(view);
    }

    pub fn render_frame(&self) -> anyhow::Result<()> {
        let mut view_guard = self.view.write();
        let Some(view) = view_guard.as_mut() else {
            return Ok(());
        };
        let mut scene_guard = self.scene.write();
        view.render_scene(&mut scene_guard)?;
        Ok(())
    }

    pub fn global() -> Arc<RwLock<W3DDisplay>> {
        static DISPLAY: OnceLock<Arc<RwLock<W3DDisplay>>> = OnceLock::new();
        DISPLAY
            .get_or_init(|| Arc::new(RwLock::new(W3DDisplay::new())))
            .clone()
    }

    pub fn global_scene() -> Arc<RwLock<W3DScene>> {
        Self::global().read().scene()
    }
}

impl Default for W3DDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for W3DDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("W3DDisplay").finish()
    }
}
