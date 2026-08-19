// Live HLOD child snapshot captured at the W3DModelDraw commit boundary.
// C++ W3DGhostObject.cpp:98-128 clones the live RenderObj after this frame's
// anim/bone application. Included into w3d_model_draw; parent already imports
// Matrix3D / ObjectID / bone visibility helpers.

use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex, RwLock};

/// One HLOD child after this frame's live hierarchy walk.
///
/// `uv_animations_disabled` is ghost-route state only (C++ `disableUVAnimations`
/// `MAPPER_ID_LINEAR_OFFSET` pin). It must not mutate the shared live asset.
#[derive(Clone, Debug, PartialEq)]
pub struct HlodLiveChildState {
    pub name: String,
    pub hidden: bool,
    pub local_transform: Matrix3D,
    pub uv_animations_disabled: bool,
}

/// Which reconstruction served an exact HLOD ghost snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HlodGhostChildCapturePath {
    LiveHierarchy,
    BoneOverridesFallback,
}

/// Inputs the optional renderer-owned capture adapter needs at commit time.
pub struct HlodLiveChildCaptureRequest<'a> {
    pub object_id: ObjectID,
    pub model_name: &'a str,
    pub bone_overrides: &'a [BoneOverrideState],
    pub sub_object_visibility: &'a [SubObjectVisibilityState],
    pub animation_name: Option<&'a str>,
    pub animation_time: f32,
}

pub type HlodLiveChildCaptureHook =
    Arc<dyn Fn(&HlodLiveChildCaptureRequest<'_>) -> Option<Vec<HlodLiveChildState>> + Send + Sync>;

static CAPTURE_HOOK: Lazy<RwLock<Option<HlodLiveChildCaptureHook>>> =
    Lazy::new(|| RwLock::new(None));
static COMMITTED_LIVE_STATES: Lazy<Mutex<HashMap<ObjectID, Vec<HlodLiveChildState>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_hlod_live_child_capture_hook(hook: Option<HlodLiveChildCaptureHook>) {
    if let Ok(mut slot) = CAPTURE_HOOK.write() {
        *slot = hook;
    }
}

/// Ask the renderer-owned adapter for a live child walk. `None` means no W3D
/// backend / headless / unresolved instance — fail closed at this seam.
pub fn try_capture_hlod_live_child_states(
    request: &HlodLiveChildCaptureRequest<'_>,
) -> Option<Vec<HlodLiveChildState>> {
    let hook = CAPTURE_HOOK.read().ok()?.clone()?;
    hook(request)
}

pub fn publish_hlod_live_child_states(object_id: ObjectID, states: Vec<HlodLiveChildState>) {
    if let Ok(mut store) = COMMITTED_LIVE_STATES.lock() {
        store.insert(object_id, states);
    }
}

pub fn take_hlod_live_child_states(object_id: ObjectID) -> Option<Vec<HlodLiveChildState>> {
    COMMITTED_LIVE_STATES
        .lock()
        .ok()
        .and_then(|mut store| store.remove(&object_id))
}

pub fn peek_hlod_live_child_states(object_id: ObjectID) -> Option<Vec<HlodLiveChildState>> {
    COMMITTED_LIVE_STATES
        .lock()
        .ok()
        .and_then(|store| store.get(&object_id).cloned())
}

    #[cfg(test)]
mod hlod_live_child_tests {
    use super::*;

    #[test]
    fn store_round_trips_committed_live_states() {
        let object_id = 4242;
        let live = HlodLiveChildState {
            name: "Body".to_string(),
            hidden: false,
            local_transform: Matrix3D::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
            uv_animations_disabled: false,
        };
        publish_hlod_live_child_states(object_id, vec![live.clone()]);
        let taken = take_hlod_live_child_states(object_id).expect("published states");
        assert_eq!(taken, vec![live]);
        assert!(take_hlod_live_child_states(object_id).is_none());
    }
}
