//! Owned fixtures for unit tests. No process-wide UI / window manager state.

use crate::core::subsystems::InGameUISubsystem;
use crate::message_stream::ReceivingUi;

/// Run `f` with a stack-owned InGameUISubsystem exposed as ReceivingUi.
pub fn with_receiving_ui<R>(f: impl FnOnce(&mut ReceivingUi<'_>) -> R) -> R {
    #[cfg(test)]
    ensure_ambient_host();
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, 0, 0);
    f(&mut ui)
}

/// Same as [`with_receiving_ui`], with explicit logic/client frames.
pub fn with_receiving_ui_at<R>(
    logic_frame: u32,
    client_frame: u32,
    f: impl FnOnce(&mut ReceivingUi<'_>) -> R,
) -> R {
    #[cfg(test)]
    ensure_ambient_host();
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, logic_frame, client_frame);
    f(&mut ui)
}

/// No-op outside tests.
#[cfg(not(test))]
pub fn ensure_ambient_host() {}

#[cfg(test)]
pub use ambient_host::ensure_ambient_host;

/// Construct a `GameClient` whose terrain visual is published the way
/// standalone `GameClient::init` publishes it.
#[cfg(test)]
pub fn park_live_game_client() -> crate::core::game_client::GameClient {
    ensure_ambient_host();
    let client = crate::core::game_client::GameClient::new().expect("GameClient");
    crate::terrain::terrain_visual::publish_terrain_visual(std::sync::Arc::clone(
        &client.terrain_visual_gpu,
    ));
    client
}


#[cfg(test)]
mod ambient_host {
    use game_engine::common::common_services::{self, CommonServices};
    use gamelogic::system::engine_stores::{install_active, is_active, new_for_world, EngineStores};
    use std::cell::RefCell;
    use std::sync::Arc;

    thread_local! {
        static DEFAULT_CORE: RefCell<Option<Arc<EngineStores>>> = const { RefCell::new(None) };
    }

    /// Seed a per-thread default Core world when none is published.
    ///
    /// Ambient hooks were deleted; the transitional stack lives in
    /// `gamelogic::system::engine_stores`. Prefer
    /// [`gamelogic::system::engine_stores::with_active_stores`] for new tests.
    ///
    /// `EngineStores::new_for_world` clones the process UpgradeCenter, whose
    /// LazyLock init needs an active CommonServices (NameKey lookups). Bootstrap
    /// a scratch Common bind before constructing the Core, then let
    /// `install_active` publish the world's Common on top.
    pub fn ensure_ambient_host() {
        DEFAULT_CORE.with(|slot| {
            if let Some(core) = slot.borrow().as_ref() {
                if is_active(core) {
                    return;
                }
            }
            let bootstrap = Arc::new(CommonServices::new());
            let _ = common_services::install_common_services(bootstrap);
            let core = new_for_world();
            install_active(Arc::clone(&core));
            *slot.borrow_mut() = Some(core);
        });
    }
}

