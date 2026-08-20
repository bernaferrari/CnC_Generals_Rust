// Weapon store, LOD, new-game/pause/load/audio hooks, scene, GlobalData
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// TheWeaponStore singleton - weapon management system (matching C++ TheWeaponStore)
pub struct TheWeaponStore;

impl TheWeaponStore {
    /// Get the weapon store instance
    pub fn get() -> Option<Self> {
        Some(Self)
    }

    /// Create and fire a temporary weapon at a position
    pub fn create_and_fire_temp_weapon_at_pos(
        &self,
        weapon_template: &Arc<crate::weapon::WeaponTemplate>,
        source_id: ObjectID,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let result = crate::weapon::with_weapon_store(|store| {
            store.create_and_fire_temp_weapon(weapon_template, source_id, None, Some(position))
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(format!("{:?}", err).into()),
            Err(err) => Err(format!("{:?}", err).into()),
        }
    }

    /// Create and fire a temporary weapon
    pub fn create_and_fire_temp_weapon(
        weapon_name: &str,
        source: &crate::object::Object,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let template = crate::weapon::with_weapon_store(|store| {
            store.find_weapon_template(weapon_name).cloned()
        })
        .map_err(|err| format!("{:?}", err))?;

        let Some(template) = template else {
            return Err(format!("Weapon template '{}' not found", weapon_name).into());
        };

        let source_id = source.get_id();
        let result = crate::weapon::with_weapon_store(|store| {
            store.create_and_fire_temp_weapon(&template, source_id, None, Some(position))
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(format!("{:?}", err).into()),
            Err(err) => Err(format!("{:?}", err).into()),
        }
    }
}

/// TheGameLODManager singleton - level of detail management (matching C++ TheGameLODManager)
pub struct TheGameLODManager;

impl TheGameLODManager {
    /// Get slow death scale factor (matches GameLOD.ini DynamicGameLOD::SlowDeathScale)
    pub fn get_slow_death_scale() -> Real {
        game_engine::common::game_lod::get_slow_death_scale() as Real
    }

    /// C++ `TheGameLODManager->isDebrisSkipped()`.
    pub fn is_debris_skipped() -> bool {
        game_engine::common::ini::is_debris_skipped()
    }

    /// Set the runtime debris skip mask used by `isDebrisSkipped`.
    pub fn set_dynamic_debris_skip_mask(mask: i32) {
        game_engine::common::ini::set_dynamic_debris_skip_mask(mask);
    }
}

/// Hooks for GameClient integration during prepareNewGame.
pub trait PrepareNewGameHooks: Send + Sync {
    fn ensure_background_window(&self);
    fn hide_shell(&self);
}

static PREPARE_NEW_GAME_HOOKS: OnceLock<Arc<dyn PrepareNewGameHooks>> = OnceLock::new();

/// Hooks provided by GameClient so pause transitions can mirror the original
/// C++ cursor/input restore behavior.
pub trait GamePauseHooks: Send + Sync {
    fn on_game_pause_state_changed(&self, paused: Bool);
}

static GAME_PAUSE_HOOKS: OnceLock<Arc<dyn GamePauseHooks>> = OnceLock::new();

/// Hooks provided by GameClient so GameLogic can drive the visible C++
/// LoadScreen lifecycle while the map initialization path is running.
pub trait LoadScreenHooks: Send + Sync {
    fn begin_load_screen(&self, game_mode: Int, loading_save_game: Bool);
    fn update_load_screen(&self, progress: Int);
    fn run_load_screen_completion_transition(&self, _loading_save_game: Bool) {}
    fn end_load_screen(&self);
}

fn load_screen_hooks_slot() -> &'static Mutex<Option<Arc<dyn LoadScreenHooks>>> {
    static LOAD_SCREEN_HOOKS: OnceLock<Mutex<Option<Arc<dyn LoadScreenHooks>>>> = OnceLock::new();
    LOAD_SCREEN_HOOKS.get_or_init(|| Mutex::new(None))
}

/// Hooks provided by GameClient so gameplay-side audio locality can query
/// observer camera focus when the local player is dead/spectating.
pub trait ObserverAudioLocalityHooks: Send + Sync {
    fn get_observer_look_at_player_index(&self) -> Option<Int>;
}

/// Hooks provided by GameClient so gameplay-side audio view resolver can pull
/// tactical-view/camera state instead of using zeroed placeholders.
pub trait ObserverAudioViewHooks: Send + Sync {
    fn get_tactical_view_position(&self) -> Option<(Real, Real, Real)>;
    fn get_tactical_view_angle(&self) -> Option<Real>;
    fn get_3d_camera_position(&self) -> Option<(Real, Real, Real)>;
}

static OBSERVER_AUDIO_LOCALITY_HOOKS: OnceLock<Arc<dyn ObserverAudioLocalityHooks>> =
    OnceLock::new();
static OBSERVER_AUDIO_VIEW_HOOKS: OnceLock<Arc<dyn ObserverAudioViewHooks>> = OnceLock::new();

pub fn register_prepare_new_game_hooks(hooks: Arc<dyn PrepareNewGameHooks>) -> bool {
    PREPARE_NEW_GAME_HOOKS.set(hooks).is_ok()
}

fn prepare_new_game_hooks() -> Option<&'static Arc<dyn PrepareNewGameHooks>> {
    PREPARE_NEW_GAME_HOOKS.get()
}

pub fn register_game_pause_hooks(hooks: Arc<dyn GamePauseHooks>) -> bool {
    GAME_PAUSE_HOOKS.set(hooks).is_ok()
}

fn game_pause_hooks() -> Option<&'static Arc<dyn GamePauseHooks>> {
    GAME_PAUSE_HOOKS.get()
}

pub fn register_load_screen_hooks(hooks: Arc<dyn LoadScreenHooks>) -> bool {
    let mut slot = load_screen_hooks_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *slot = Some(hooks);
    true
}

#[cfg(test)]
pub fn clear_load_screen_hooks() {
    let mut slot = load_screen_hooks_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *slot = None;
}

fn with_load_screen_hooks<F>(f: F) -> bool
where
    F: FnOnce(&dyn LoadScreenHooks),
{
    let hooks = {
        let slot = load_screen_hooks_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        slot.clone()
    };
    if let Some(hooks) = hooks {
        f(hooks.as_ref());
        true
    } else {
        false
    }
}

pub fn register_observer_audio_locality_hooks(hooks: Arc<dyn ObserverAudioLocalityHooks>) -> bool {
    OBSERVER_AUDIO_LOCALITY_HOOKS.set(hooks).is_ok()
}

fn observer_audio_locality_hooks() -> Option<&'static Arc<dyn ObserverAudioLocalityHooks>> {
    OBSERVER_AUDIO_LOCALITY_HOOKS.get()
}

pub fn register_observer_audio_view_hooks(hooks: Arc<dyn ObserverAudioViewHooks>) -> bool {
    OBSERVER_AUDIO_VIEW_HOOKS.set(hooks).is_ok()
}

fn observer_audio_view_hooks() -> Option<&'static Arc<dyn ObserverAudioViewHooks>> {
    OBSERVER_AUDIO_VIEW_HOOKS.get()
}

use game_engine::common::system::scene_submission::{
    SceneLineDesc, SceneLineId, SceneModelDesc, SceneProjectileStreamDesc, SceneSubmission,
};

static SCENE_SUBMISSION: OnceLock<Arc<dyn SceneSubmission>> = OnceLock::new();

pub fn register_scene_submission(impl_: Arc<dyn SceneSubmission>) -> bool {
    SCENE_SUBMISSION.set(impl_).is_ok()
}

fn get_scene_submission() -> Option<&'static Arc<dyn SceneSubmission>> {
    SCENE_SUBMISSION.get()
}

pub fn submit_scene_line(drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId> {
    get_scene_submission().and_then(|s| s.submit_line(drawable_id, desc))
}

pub fn update_scene_line(id: SceneLineId, desc: &SceneLineDesc) {
    if let Some(s) = get_scene_submission() {
        s.update_line(id, desc);
    }
}

pub fn remove_scene_line(id: SceneLineId) {
    if let Some(s) = get_scene_submission() {
        s.remove_line(id);
    }
}

pub fn submit_scene_model(desc: SceneModelDesc) {
    if let Some(s) = get_scene_submission() {
        s.submit_model(desc);
    }
}

pub fn submit_scene_projectile_stream(desc: SceneProjectileStreamDesc) {
    if let Some(s) = get_scene_submission() {
        s.submit_projectile_stream(desc);
    }
}

pub fn begin_scene_logic_frame() {
    if let Some(s) = get_scene_submission() {
        s.begin_logic_frame();
    }
}

pub fn end_scene_logic_frame() {
    if let Some(s) = get_scene_submission() {
        s.end_logic_frame();
    }
}

/// Global data singleton (matches C++ TheGlobalData)
pub struct TheGlobalData;

impl TheGlobalData {
    pub fn get() -> Option<&'static Self> {
        let _ = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        static GLOBAL: TheGlobalData = TheGlobalData;
        Some(&GLOBAL)
    }

    pub fn get_max_tunnel_capacity(&self) -> i32 {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.max_tunnel_capacity
    }

    pub fn get_base_regen_health_percent_per_second(&self) -> Real {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.base_regen_health_percent_per_second
    }

    pub fn get_base_regen_delay(&self) -> UnsignedInt {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.base_regen_delay
    }

    pub fn get_special_power_view_object_name(&self) -> String {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.special_power_view_object_name.clone()
    }

    /// Check if special powers use delay (matches C++ TheGlobalData->m_specialPowerUsesDelay)
    /// When false (debug/cheat mode), all special powers are instantly ready
    pub fn get_special_power_uses_delay(&self) -> bool {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.special_power_uses_delay
    }

    /// Prison bounty multiplier (matches GlobalData::m_prisonBountyMultiplier).
    pub fn get_prison_bounty_multiplier(&self) -> Real {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.prison_bounty_multiplier
    }

    /// Prison bounty floating text color (matches GlobalData::m_prisonBountyTextColor).
    pub fn get_prison_bounty_text_color(&self) -> crate::common::Color {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let color = data.read().prison_bounty_text_color;
        crate::common::Color::rgb(
            (color.r.clamp(0.0, 1.0) * 255.0) as u8,
            (color.g.clamp(0.0, 1.0) * 255.0) as u8,
            (color.b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    pub fn get_shroud_alpha(&self) -> u8 {
        get_engine_global_data()
            .map(|data| data.read().shroud_alpha)
            .unwrap_or(0)
    }

    pub fn solo_player_health_bonus(&self, player_type: usize, difficulty: usize) -> f32 {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard
            .solo_player_health_bonus_for_difficulty
            .get(player_type)
            .and_then(|row| row.get(difficulty))
            .copied()
            .unwrap_or(1.0)
    }

    pub fn get_clear_alpha(&self) -> u8 {
        get_engine_global_data()
            .map(|data| data.read().clear_alpha)
            .unwrap_or(255)
    }

    pub fn get_time_of_day(&self) -> TimeOfDay {
        if let Some(data) = get_engine_global_data() {
            return map_time_of_day(data.read().time_of_day);
        }

        let guard = GLOBAL_TIME_OF_DAY.lock().unwrap();
        *guard
    }

    pub fn set_time_of_day(&self, value: TimeOfDay) {
        if let Some(data) = get_engine_global_data() {
            let mut guard = data.write();
            let mapped = match value {
                TimeOfDay::Morning => IniTimeOfDay::Morning,
                TimeOfDay::Evening => IniTimeOfDay::Evening,
                TimeOfDay::Night => IniTimeOfDay::Night,
                TimeOfDay::Day => IniTimeOfDay::Afternoon,
            };
            guard.set_time_of_day(mapped);
        }

        let mut guard = GLOBAL_TIME_OF_DAY.lock().unwrap();
        *guard = value;

        // Host path: dual-world factory empty — Main presentation owns drawable TOD.
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(obj_guard) = obj_arc.write() else {
                continue;
            };
            if let Some(drawable) = obj_guard.get_drawable() {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.set_time_of_day(value);
                    draw_guard.changed_team(&obj_guard);
                }
            }
        }
    }
}

static GLOBAL_TIME_OF_DAY: Lazy<Mutex<TimeOfDay>> = Lazy::new(|| Mutex::new(TimeOfDay::Day));

fn map_time_of_day(value: IniTimeOfDay) -> TimeOfDay {
    match value {
        IniTimeOfDay::Invalid => TimeOfDay::Day,
        IniTimeOfDay::Morning => TimeOfDay::Morning,
        IniTimeOfDay::Evening => TimeOfDay::Evening,
        IniTimeOfDay::Night => TimeOfDay::Night,
        IniTimeOfDay::Afternoon => TimeOfDay::Day,
    }
}

fn map_audio_time_of_day(value: TimeOfDay) -> EngineTimeOfDay {
    match value {
        TimeOfDay::Morning => EngineTimeOfDay::Morning,
        TimeOfDay::Evening => EngineTimeOfDay::Evening,
        TimeOfDay::Night => EngineTimeOfDay::Night,
        TimeOfDay::Day => EngineTimeOfDay::Day,
    }
}
