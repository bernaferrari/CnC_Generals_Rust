//! C++ engine services separated from the standalone Core AI runtime.
//!
//! Main owns `WorldServices`: content upgrades, shroud and client visuals.
//! Constructing it never constructs Core AI or selects a world. Core's
//! reference runtime owns `EngineStores`, which adds mandatory AI and AIData.
//! The existing publication stack remains a bounded migration aid for legacy
//! presentation callbacks. It carries services independently of Core runtime;
//! asking for Core AI inside a Main service scope is an explicit error.
use crate::ai::AI;
use crate::helpers::ClientVisualState;
use crate::system::shroud_manager::ShroudManager;
use crate::upgrade::center::UpgradeCenter;
use game_engine::common::ini::ini_ai_data::{self, AIDataStore};
use game_engine::common::system::upgrade as common_upgrade;
use std::cell::RefCell;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

/// Service values shared by Main and the standalone Core runtime. No Core AI.
pub struct WorldServices {
    upgrade_center: Arc<RwLock<UpgradeCenter>>,
    shroud: Arc<Mutex<ShroudManager>>,
    client_visuals: ClientVisualState,
    terrain: Arc<RwLock<crate::terrain::TerrainLogic>>,
}
impl WorldServices {
    fn engine_defaults() -> Self {
        Self {
            upgrade_center: common_upgrade::process_lifetime_upgrade_center(),
            shroud: Arc::new(Mutex::new(ShroudManager::new())),
            client_visuals: ClientVisualState::default(),
            terrain: Arc::clone(&crate::terrain::THE_TERRAIN_LOGIC),
        }
    }
    /// Snapshot engine content; fresh mutable per-world services. Inert.
    pub fn new_for_world() -> Self {
        let upgrades = common_upgrade::process_lifetime_upgrade_center()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        Self {
            upgrade_center: Arc::new(RwLock::new(upgrades)),
            // Only definitions are inherited. Visibility, cells and queued
            // reveals belong to this match (CPP PartitionManager.cpp:2515).
            shroud: Arc::new(Mutex::new(ShroudManager::new())),
            client_visuals: ClientVisualState::default(),
            terrain: Arc::new(RwLock::new(crate::terrain::TerrainLogic::new())),
        }
    }
    pub fn terrain(&self) -> &Arc<RwLock<crate::terrain::TerrainLogic>> {
        &self.terrain
    }
    pub fn upgrade_center(&self) -> &Arc<RwLock<UpgradeCenter>> {
        &self.upgrade_center
    }
    pub fn shroud(&self) -> &Arc<Mutex<ShroudManager>> {
        &self.shroud
    }
    pub(crate) fn client_visuals(&self) -> &ClientVisualState {
        &self.client_visuals
    }
}
/// Standalone Core runtime; AI is mandatory, never optional or deferred.
pub struct EngineStores {
    services: Arc<WorldServices>,
    ai: Arc<RwLock<AI>>,
    ai_data: Arc<RwLock<AIDataStore>>,
}
impl EngineStores {
    fn engine_defaults() -> Self {
        Self {
            services: Arc::clone(&PROCESS_SERVICES),
            ai: Arc::new(RwLock::new(AI::new())),
            ai_data: ini_ai_data::process_lifetime_ai_data_store(),
        }
    }
    pub fn new_for_world() -> Self {
        let snapshot = ini_ai_data::process_lifetime_ai_data_store()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        Self {
            services: new_world_services(),
            ai: Arc::new(RwLock::new(AI::new())),
            ai_data: Arc::new(RwLock::new(snapshot)),
        }
    }
    pub fn services(&self) -> &Arc<WorldServices> {
        &self.services
    }
    pub fn upgrade_center(&self) -> &Arc<RwLock<UpgradeCenter>> {
        self.services.upgrade_center()
    }
    pub fn shroud(&self) -> &Arc<Mutex<ShroudManager>> {
        self.services.shroud()
    }
    pub fn ai(&self) -> &Arc<RwLock<AI>> {
        &self.ai
    }
    pub fn ai_data(&self) -> &Arc<RwLock<AIDataStore>> {
        &self.ai_data
    }
    pub(crate) fn client_visuals(&self) -> &ClientVisualState {
        self.services.client_visuals()
    }
}
static PROCESS_SERVICES: LazyLock<Arc<WorldServices>> =
    LazyLock::new(|| Arc::new(WorldServices::engine_defaults()));
// Only standalone Core access constructs this runtime. Main service snapshots
// must never touch it, including indirectly through content/shroud helpers.
static PROCESS_LIFETIME: LazyLock<Arc<EngineStores>> =
    LazyLock::new(|| Arc::new(EngineStores::engine_defaults()));
#[derive(Clone)]
enum PublishedOwner {
    Core(Arc<EngineStores>),
    Services(Arc<WorldServices>),
}
impl PublishedOwner {
    fn services(&self) -> &Arc<WorldServices> {
        match self {
            Self::Core(core) => core.services(),
            Self::Services(services) => services,
        }
    }
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Core(a), Self::Core(b)) => Arc::ptr_eq(a, b),
            (Self::Services(a), Self::Services(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    fn install_common(&self) {
        common_upgrade::install_upgrade_center(Arc::clone(self.services().upgrade_center()));
        if let Self::Core(core) = self {
            ini_ai_data::install_ai_data_store(Arc::clone(core.ai_data()));
        }
    }
    fn uninstall_common(&self) {
        common_upgrade::uninstall_upgrade_center_if_current(self.services().upgrade_center());
        if let Self::Core(core) = self {
            ini_ai_data::uninstall_ai_data_store_if_current(core.ai_data());
        }
    }
}
static ACTIVE: RwLock<Vec<PublishedOwner>> = RwLock::new(Vec::new());
thread_local! {
    // Reuses the existing bounded context stack; no new TLS publication.
    static SCOPED_ACTIVE: RefCell<Vec<PublishedOwner>> = const { RefCell::new(Vec::new()) };
}
fn published_owner() -> Option<PublishedOwner> {
    SCOPED_ACTIVE
        .with(|stack| stack.borrow().last().cloned())
        .or_else(|| {
            ACTIVE
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .last()
                .cloned()
        })
}
pub fn active() -> Arc<EngineStores> {
    match published_owner() {
        Some(PublishedOwner::Core(core)) => core,
        Some(PublishedOwner::Services(_)) => {
            panic!("Core AI runtime requested from a Main world-services boundary")
        }
        None => Arc::clone(&PROCESS_LIFETIME),
    }
}
pub fn active_services() -> Arc<WorldServices> {
    published_owner()
        .map(|owner| Arc::clone(owner.services()))
        .unwrap_or_else(|| Arc::clone(&PROCESS_SERVICES))
}
fn is_published(owner: &PublishedOwner) -> bool {
    ACTIVE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .last()
        .is_some_and(|head| head.same(owner))
}
pub fn is_active(core: &Arc<EngineStores>) -> bool {
    is_published(&PublishedOwner::Core(Arc::clone(core)))
}
pub fn is_services_active(services: &Arc<WorldServices>) -> bool {
    is_published(&PublishedOwner::Services(Arc::clone(services)))
}
fn install(owner: PublishedOwner) -> Option<PublishedOwner> {
    owner.install_common();
    let mut active = ACTIVE.write().unwrap_or_else(|e| e.into_inner());
    let prior = active.last().cloned();
    active.push(owner);
    prior
}
pub fn install_active(core: Arc<EngineStores>) -> Option<Arc<EngineStores>> {
    install(PublishedOwner::Core(core)).and_then(|owner| match owner {
        PublishedOwner::Core(core) => Some(core),
        PublishedOwner::Services(_) => None,
    })
}
pub fn install_services(services: Arc<WorldServices>) {
    let _ = install(PublishedOwner::Services(services));
}
fn uninstall(owner: &PublishedOwner) -> bool {
    let (was_head, restored) = {
        let mut active = ACTIVE.write().unwrap_or_else(|e| e.into_inner());
        if active.last().is_some_and(|head| head.same(owner)) {
            active.pop();
            (true, active.last().cloned())
        } else {
            active.retain(|entry| !entry.same(owner));
            (false, None)
        }
    };
    if was_head {
        owner.uninstall_common();
        if let Some(restored) = restored {
            restored.install_common();
            // Main services do not own the Common AI parser slot. If a Core
            // owner was popped above them, restore the still-installed Core
            // parser below them without selecting/constructing a runtime.
            if matches!(owner, PublishedOwner::Core(_))
                && matches!(restored, PublishedOwner::Services(_))
            {
                let prior_data = ACTIVE
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .rev()
                    .find_map(|entry| match entry {
                        PublishedOwner::Core(core) => Some(Arc::clone(core.ai_data())),
                        PublishedOwner::Services(_) => None,
                    });
                if let Some(prior_data) = prior_data {
                    ini_ai_data::install_ai_data_store(prior_data);
                }
            }
        }
    }
    was_head
}
pub fn uninstall_active_if_current(core: &Arc<EngineStores>) -> bool {
    uninstall(&PublishedOwner::Core(Arc::clone(core)))
}
pub fn uninstall_services(services: &Arc<WorldServices>) -> bool {
    uninstall(&PublishedOwner::Services(Arc::clone(services)))
}
pub(crate) fn restore_active(
    expected: &Arc<EngineStores>,
    previous: Option<Arc<EngineStores>>,
) -> bool {
    let owner = PublishedOwner::Core(Arc::clone(expected));
    if !is_published(&owner) {
        return false;
    }
    let removed = uninstall(&owner);
    if let Some(previous) = previous {
        if !is_active(&previous) {
            install_active(previous);
        }
    }
    removed
}
fn with_owner<R>(owner: PublishedOwner, f: impl FnOnce() -> R) -> R {
    struct RestoreScope {
        owner: PublishedOwner,
        prior_upgrade: Option<Arc<RwLock<UpgradeCenter>>>,
        prior_ai_data: Option<Arc<RwLock<AIDataStore>>>,
    }
    impl Drop for RestoreScope {
        fn drop(&mut self) {
            SCOPED_ACTIVE.with(|stack| {
                let mut stack = stack.borrow_mut();
                if stack.last().is_some_and(|head| head.same(&self.owner)) {
                    stack.pop();
                }
            });
            let upgrade = self.owner.services().upgrade_center();
            if Arc::ptr_eq(&common_upgrade::get_upgrade_center(), upgrade) {
                if let Some(previous) = self.prior_upgrade.take() {
                    common_upgrade::install_upgrade_center(previous);
                } else {
                    common_upgrade::uninstall_upgrade_center_if_current(upgrade);
                }
            }
            if let PublishedOwner::Core(core) = &self.owner {
                if Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), core.ai_data()) {
                    if let Some(previous) = self.prior_ai_data.take() {
                        ini_ai_data::install_ai_data_store(previous);
                    } else {
                        ini_ai_data::uninstall_ai_data_store_if_current(core.ai_data());
                    }
                }
            }
        }
    }
    let prior_upgrade =
        common_upgrade::install_upgrade_center(Arc::clone(owner.services().upgrade_center()));
    let prior_ai_data = match &owner {
        PublishedOwner::Core(core) => {
            ini_ai_data::install_ai_data_store(Arc::clone(core.ai_data()))
        }
        PublishedOwner::Services(_) => None,
    };
    SCOPED_ACTIVE.with(|stack| stack.borrow_mut().push(owner.clone()));
    let _restore = RestoreScope {
        owner,
        prior_upgrade,
        prior_ai_data,
    };
    f()
}
pub fn with_active_stores<R>(core: &Arc<EngineStores>, f: impl FnOnce() -> R) -> R {
    with_owner(PublishedOwner::Core(Arc::clone(core)), f)
}
/// Temporary synchronous service-only publication for legacy presentation.
/// This never publishes an AI owner or writes the Common AIData parser slot.
pub fn with_world_services<R>(services: &Arc<WorldServices>, f: impl FnOnce() -> R) -> R {
    with_owner(PublishedOwner::Services(Arc::clone(services)), f)
}
pub fn new_for_world() -> Arc<EngineStores> {
    Arc::new(EngineStores::new_for_world())
}
pub fn new_world_services() -> Arc<WorldServices> {
    Arc::new(WorldServices::new_for_world())
}
pub fn upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    Arc::clone(active_services().upgrade_center())
}
pub fn shroud_manager() -> Arc<Mutex<ShroudManager>> {
    Arc::clone(active_services().shroud())
}

pub fn the_ai() -> Arc<RwLock<AI>> {
    Arc::clone(active().ai())
}

/// Move `bundle`'s AI contents out for a whole-world restore transaction
/// while preserving the lock identity aliases hold (contents swap, C++
/// AI.cpp:280 wrapper semantics). The runtime world transaction owns the
/// only raw use of this boundary API and passes the explicit bundle it
/// captured at `begin`, so the swap never depends on ambient resolution.
pub(crate) fn take_ai_for_world_boundary(bundle: &Arc<EngineStores>) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, AI::new())
}

/// Install AI contents into `bundle` at a whole-world restore boundary and
/// return the contents they replaced. See [`take_ai_for_world_boundary`].
pub(crate) fn replace_ai_for_world_boundary(bundle: &Arc<EngineStores>, next: AI) -> AI {
    let mut ai = bundle
        .ai()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *ai, next)
}

#[cfg(test)]
fn active_stack_depth() -> usize {
    ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_visuals_follow_world_rollback_and_commit() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let client = crate::helpers::TheGameClient::get().expect("logic client bridge");
        let drawable_id = 9_700_101;
        let object_id = 9_700_102;

        let live = new_for_world();
        // Constructing a candidate must not publish its empty visual maps.
        let candidate = new_for_world();
        assert_eq!(active_stack_depth(), 0);
        install_active(Arc::clone(&live));
        client.seed_drawable_pose_for_test(drawable_id, crate::common::Coord3D::ZERO, 0.25);
        client.begin_object_model_draw_frame(object_id);
        client.set_object_wheel_info(object_id, crate::helpers::DrawWheelInfo::default());
        crate::helpers::ClientVisualHandle::new(Arc::clone(live.services()))
            .note_weapon_recoil(object_id, 2.0, 0.5);
        let live_light = crate::helpers::create_scene_point_light();
        client.add_tree(
            drawable_id,
            &crate::common::Coord3D::ZERO,
            1.0,
            0.0,
            0.0,
            &crate::object::draw::w3d_tree_draw::W3DTreeDrawModuleData::new(),
        );

        // Map/save staging works on its own captured bundle. Reusing IDs in
        // the candidate cannot overwrite any live-world presentation data.
        with_active_stores(&candidate, || {
            assert!(client.find_drawable_by_id(drawable_id).is_none());
            assert!(client.get_object_wheel_info(object_id).is_none());
            assert!(client.get_registered_tree(drawable_id).is_none());
            assert!(
                crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()))
                    .take_weapon_recoils()
                    .is_empty()
            );
            assert!(crate::helpers::scene_point_lights().is_empty());
            client.seed_drawable_pose_for_test(
                drawable_id,
                crate::common::Coord3D::new(3.0, 4.0, 5.0),
                0.75,
            );
            assert_eq!(crate::helpers::create_scene_point_light(), 1);
            crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()))
                .note_weapon_recoil(object_id, 4.0, 0.75);
        });
        assert_eq!(live_light, 1);
        let candidate_visuals =
            crate::helpers::ClientVisualHandle::new(Arc::clone(candidate.services()));
        let captured = candidate_visuals.snapshot_objectless_drawables();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].1.orientation, 0.75);
        assert_eq!(
            candidate_visuals.take_weapon_recoils(),
            vec![(object_id, 4.0, 0.75)]
        );
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        assert_eq!(
            crate::helpers::ClientVisualHandle::new(Arc::clone(live.services()))
                .take_weapon_recoils(),
            vec![(object_id, 2.0, 0.5)]
        );
        assert_eq!(
            live.client_visuals()
                .model_draw_frames
                .lock()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(live.client_visuals().terrain_trees.lock().unwrap().len(), 1);

        // Failed candidate: dropping it leaves the installed live world and
        // its visual state intact. Successful candidate: the candidate's
        // bundle becomes the head and the buried old world can be dropped.
        install_active(Arc::clone(&candidate));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.75
        );
        assert!(uninstall_active_if_current(&candidate));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        candidate_visuals.clear_visual_state_for_reset();
        with_active_stores(&candidate, || {
            assert!(client.find_drawable_by_id(drawable_id).is_none());
            assert!(crate::helpers::scene_point_lights().is_empty());
            client.seed_drawable_pose_for_test(
                drawable_id,
                crate::common::Coord3D::new(3.0, 4.0, 5.0),
                0.75,
            );
        });
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.25
        );
        install_active(Arc::clone(&candidate));
        assert!(!uninstall_active_if_current(&live));
        assert_eq!(
            client.find_drawable_by_id(drawable_id).unwrap().orientation,
            0.75
        );
        assert!(uninstall_active_if_current(&candidate));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn drawable_recoil_uses_bound_world_even_when_another_is_active() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let a = new_for_world();
        let b = new_for_world();
        let a_weak = Arc::downgrade(&a);
        let a_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(a.services()));
        let b_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(b.services()));
        let mut drawable = crate::object::drawable::Drawable::new(
            71,
            81,
            "RecoilOwnerTest".to_string(),
            crate::object::drawable::DrawableType::Animated,
        );
        drawable.bind_visual_owner(a_visuals.downgrade());

        install_active(Arc::clone(&b));
        drawable.apply_weapon_recoil(2.5, 0.75);
        assert!(b_visuals.take_weapon_recoils().is_empty());
        assert_eq!(a_visuals.take_weapon_recoils(), vec![(81, 2.5, 0.75)]);
        assert!(uninstall_active_if_current(&b));

        drop(a_visuals);
        drop(a);
        assert!(a_weak.upgrade().is_none(), "drawable must not retain world");
        drawable.apply_weapon_recoil(1.0, 0.25);
        assert!(b_visuals.take_weapon_recoils().is_empty());
    }

    #[test]
    fn client_created_drawable_keeps_its_world_after_active_switch() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let a = new_for_world();
        let b = new_for_world();
        let a_weak = Arc::downgrade(&a);
        let a_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(a.services()));
        let b_visuals = crate::helpers::ClientVisualHandle::new(Arc::clone(b.services()));
        install_active(Arc::clone(&a));
        let template = crate::common::DefaultThingTemplate::new("VisualOwnerTest".to_string());
        let id = crate::helpers::TheGameClient::get()
            .expect("client bridge")
            .create_drawable(&template);
        let drawable = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .and_then(|state| state.drawable.clone())
            .expect("created drawable");
        let mut saved = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .expect("saved drawable");
        saved.position = crate::common::Coord3D::new(4.0, 5.0, 6.0);
        saved.orientation = 0.5;
        saved.beam_start = Some(crate::common::Coord3D::ZERO);
        saved.beam_end = Some(crate::common::Coord3D::new(1.0, 2.0, 3.0));
        install_active(Arc::clone(&b));
        let b_id = crate::helpers::TheGameClient::get()
            .expect("client bridge")
            .create_drawable(&template);
        drawable.write().unwrap().apply_weapon_recoil(1.5, 0.25);
        assert_eq!(
            a_visuals.take_weapon_recoils(),
            vec![(crate::common::INVALID_ID, 1.5, 0.25)]
        );
        assert!(b_visuals.take_weapon_recoils().is_empty());
        a_visuals.restore_objectless_drawable(id, &saved);
        let restored = a
            .client_visuals()
            .drawables
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .unwrap();
        assert_eq!(restored.position, saved.position);
        assert_eq!(restored.orientation, saved.orientation);
        assert_eq!(restored.beam_end, saved.beam_end);
        assert_eq!(
            b.client_visuals()
                .drawables
                .lock()
                .unwrap()
                .get(&b_id)
                .unwrap()
                .position,
            crate::common::Coord3D::ZERO
        );
        a_visuals.clear_objectless_drawables();
        assert!(a.client_visuals().drawables.lock().unwrap().is_empty());
        assert!(
            b.client_visuals()
                .drawables
                .lock()
                .unwrap()
                .contains_key(&b_id)
        );
        assert!(uninstall_active_if_current(&b));
        assert!(uninstall_active_if_current(&a));
        drop(a_visuals);
        drop(saved);
        drop(restored);
        drop(a);
        assert!(a_weak.upgrade().is_none(), "drawable must not retain world");
    }

    #[test]
    fn dropping_newer_world_restores_previous_as_active() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Construct A, construct B (both at their start boundaries), drop B:
        // the audit's headline ordering. Resolution must fail closed to A,
        // never to the engine-lifetime fallback.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();
        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(Arc::ptr_eq(&active(), &b));

        uninstall_active_if_current(&b); // GameLogic::drop for B
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&upgrade_center(), a.upgrade_center()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        // The Common INI funnels mirror the reinstated head.
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            a.upgrade_center()
        ));

        let services = new_world_services();
        install_services(Arc::clone(&services));
        assert!(Arc::ptr_eq(&active_services(), &services));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        install_active(Arc::clone(&b));
        assert!(uninstall_active_if_current(&b));
        assert!(Arc::ptr_eq(&active_services(), &services));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(uninstall_services(&services));
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn staged_restore_rollback_never_touches_process_lifetime() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // Live world A with content worth protecting.
        let a = new_for_world();
        install_active(Arc::clone(&a));
        let shroud_before = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        let process_shroud_before = PROCESS_LIFETIME
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();

        // RuntimeWorldStage::begin: capture the live bundle explicitly and
        // take the live AI/shroud contents out (fresh defaults stay behind
        // as the staging scratch the candidate map writes into).
        let live_bundle = active();
        assert!(Arc::ptr_eq(&live_bundle, &a));
        let live_ai = take_ai_for_world_boundary(&live_bundle);
        let live_shroud = with_active_stores(&live_bundle, || {
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::replace(&mut *guard, ShroudManager::new())
        });

        // The candidate installs at its start_new_game boundary.
        let b = new_for_world();
        install_active(Arc::clone(&b));

        // Early Err: locals drop newest-declared-first, so the staged
        // GameLogic B drops *before* the RuntimeWorldStage. B's drop pops it
        // and reinstates A as head...
        uninstall_active_if_current(&b);
        assert!(Arc::ptr_eq(&active(), &a));

        // ...then the stage's Drop reinstalls the live contents through the
        // CAPTURED bundle handle (pinned via with_active_stores), never
        // through ambient/engine-lifetime resolution. This is exactly the
        // ordering that used to install A's AI/shroud into the
        // engine-lifetime bundle.
        let pinned_bundle = Arc::clone(&live_bundle);
        with_active_stores(&live_bundle, move || {
            let scratch_ai = take_ai_for_world_boundary(&pinned_bundle);
            drop(scratch_ai);
            let replaced = replace_ai_for_world_boundary(&pinned_bundle, live_ai);
            drop(replaced);
            // Shroud rides the same pinned ambient funnel.
            let manager = shroud_manager();
            let mut guard = manager
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch_shroud = std::mem::replace(&mut *guard, live_shroud);
            drop(scratch_shroud);
        });

        // A serves its own bundle and contents again...
        assert!(Arc::ptr_eq(&the_ai(), a.ai()));
        assert!(Arc::ptr_eq(&shroud_manager(), a.shroud()));
        let shroud_after = a
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        assert_eq!(shroud_before, shroud_after);
        // ...and the engine-lifetime bundle is untouched (the pollution the
        // audit found landed there because ambient resolution fell back to
        // it after B's drop cleared the slot).
        let process_shroud_after = PROCESS_LIFETIME
            .shroud()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot_state();
        assert_eq!(process_shroud_before, process_shroud_after);

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn world_construction_does_not_touch_the_active_slot() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        let before = active();

        let _bundled = new_for_world();
        let _plain = EngineStores::new_for_world();

        assert_eq!(active_stack_depth(), 0);
        assert!(Arc::ptr_eq(&active(), &before));
    }

    #[test]
    fn one_upgrade_center_per_bundle_shared_with_common() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);
        // Engine-lifetime bundle == Common's engine-lifetime center.
        assert!(Arc::ptr_eq(
            PROCESS_LIFETIME.upgrade_center(),
            &common_upgrade::process_lifetime_upgrade_center()
        ));
        // A world snapshots it under a fresh lock (same bits, own store).
        let world = new_for_world();
        assert!(!Arc::ptr_eq(
            world.upgrade_center(),
            PROCESS_LIFETIME.upgrade_center()
        ));
        // (Other tests may only add to the engine center after the snapshot.)
        let engine = PROCESS_LIFETIME.upgrade_center().read().unwrap().clone();
        for template in world.upgrade_center().read().unwrap().get_all_upgrades() {
            let engine_template = engine.find_upgrade(template.get_name().as_str()).unwrap();
            assert_eq!(engine_template.get_mask(), template.get_mask());
        }
        // Installed, GameLogic and Common resolve the same instance.
        install_active(Arc::clone(&world));
        assert!(Arc::ptr_eq(&upgrade_center(), world.upgrade_center()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            world.upgrade_center()
        ));
        assert!(uninstall_active_if_current(&world));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn with_active_stores_pins_and_restores_resolution() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        install_active(Arc::clone(&a));
        let b = new_for_world();

        let scoped_saw_bundle = with_active_stores(&b, || {
            assert!(Arc::ptr_eq(&active(), &b));
            assert!(Arc::ptr_eq(&the_ai(), b.ai()));
            assert!(Arc::ptr_eq(&shroud_manager(), b.shroud()));
            assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), b.ai_data()));
            // Nested scopes: innermost wins, outer restored after.
            let nested = with_active_stores(&a, || Arc::ptr_eq(&active(), &a));
            assert!(nested);
            assert!(Arc::ptr_eq(&active(), &b));
            true
        });
        assert!(scoped_saw_bundle);
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(Arc::ptr_eq(&ini_ai_data::get_ai_data_store(), a.ai_data()));
        assert!(Arc::ptr_eq(
            &common_upgrade::get_upgrade_center(),
            a.upgrade_center()
        ));

        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn with_active_stores_restores_on_unwind() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let b = new_for_world();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_active_stores(&b, || panic!("scoped active-stores unwind probe"));
        }))
        .is_err();
        assert!(panicked);

        // Scoped publication and the Common INI slots are restored even on
        // unwind: resolution falls back to the engine-lifetime store.
        assert!(!Arc::ptr_eq(&active(), &b));
        assert!(Arc::ptr_eq(&active(), &PROCESS_LIFETIME));
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &ini_ai_data::process_lifetime_ai_data_store()
        ));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn restore_active_reinstates_previous_only_while_head() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        let a = new_for_world();
        let b = new_for_world();
        let c = new_for_world();
        install_active(Arc::clone(&a));

        let displaced_by_b = install_active(Arc::clone(&b));
        assert!(displaced_by_b.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(restore_active(&b, displaced_by_b));
        assert!(Arc::ptr_eq(&active(), &a));

        // A newer install between install and restore must not be clobbered.
        let displaced_by_c = install_active(Arc::clone(&c));
        assert!(displaced_by_c.as_ref().is_some_and(|p| Arc::ptr_eq(p, &a)));
        assert!(!restore_active(&b, Some(Arc::clone(&a))));
        assert!(Arc::ptr_eq(&active(), &c));

        assert!(uninstall_active_if_current(&c));
        assert!(Arc::ptr_eq(&active(), &a));
        assert!(uninstall_active_if_current(&a));
        assert_eq!(active_stack_depth(), 0);
    }

    #[test]
    fn dropping_buried_world_cannot_be_reinstated_by_later_pop() {
        let _serial = crate::test_sync::lock();
        assert_eq!(active_stack_depth(), 0);

        // The staged-commit ordering: A live, B staged on top, A dies while
        // B is the newer world (host drops the old world after install).
        let a = new_for_world();
        let b = new_for_world();
        install_active(Arc::clone(&a));
        install_active(Arc::clone(&b));

        assert!(!uninstall_active_if_current(&a)); // buried: head unchanged
        assert!(Arc::ptr_eq(&active(), &b));
        assert_eq!(active_stack_depth(), 1);

        // The newer world dying later must fall back to the engine-lifetime
        // store, never reinstate the dead world A.
        assert!(uninstall_active_if_current(&b));
        assert_eq!(active_stack_depth(), 0);
        assert!(Arc::ptr_eq(&active(), &PROCESS_LIFETIME));
    }
}
