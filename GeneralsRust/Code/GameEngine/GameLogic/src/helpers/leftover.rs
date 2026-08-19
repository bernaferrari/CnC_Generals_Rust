// UI, radar, EVA, script-engine, and victory leftover helpers
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// TheMessageStream singleton - message handling system (matching C++ TheMessageStream)
pub struct TheMessageStream;

impl TheMessageStream {
    /// Append a message - returns a builder that queues on drop.
    pub fn append_message(msg_type: MessageType) -> crate::messages::MessageBuilder {
        crate::messages::append_message(msg_type)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FloatingTextEntry {
    text: String,
    position: Coord3D,
    color: crate::common::Color,
    created_frame: UnsignedInt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimationEntry {
    pub animation_name: String,
    pub position: Coord3D,
    pub fade_on_expire: bool,
    pub duration_seconds: Real,
    pub z_rise_per_second: Real,
    pub created_frame: UnsignedInt,
}

#[derive(Debug, Default)]
struct InGameUIState {
    displayed_max_warning: bool,
    floating_texts: Vec<FloatingTextEntry>,
    world_animations: Vec<WorldAnimationEntry>,
    messages: Vec<String>,
    idle_worker_additions: Vec<(ObjectID, Int)>,
    idle_worker_removals: Vec<(ObjectID, Int)>,
    last_selection_frame: UnsignedInt,
    superweapons: Vec<SuperweaponEntry>,
    /// C++ parity: InGameUI popup message clear request flag.
    /// Set by GameLogic's ClearInGamePopupMessage command handler,
    /// consumed by GameClient's popup message handler.
    popup_clear_requested: bool,
}

#[derive(Debug, Clone)]
struct SuperweaponEntry {
    player_index: Int,
    power_name: String,
    object_id: ObjectID,
    template_id: u32,
}

static IN_GAME_UI_STATE: Lazy<RwLock<InGameUIState>> =
    Lazy::new(|| RwLock::new(InGameUIState::default()));

/// TheInGameUI singleton - in-game user interface (matching C++ TheInGameUI)
pub struct TheInGameUI;

impl TheInGameUI {
    /// Select drawable object.
    pub fn select_drawable(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) {
        if let Ok(mut guard) = drawable.write() {
            guard.set_selected(true);
            guard.flash_as_selected();
        }
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.last_selection_frame = TheGameLogic::get_frame();
        }

        let object_id = drawable.get_object_id();
        if object_id != INVALID_ID {
            if let Ok(list) = crate::player::player_list().read() {
                let local_index = list.get_local_player_index();
                if local_index != crate::player::PLAYER_INDEX_INVALID {
                    let selection_manager = crate::commands::selection::get_selection_manager();
                    let manager_lock = selection_manager.write();
                    if let Ok(mut manager) = manager_lock {
                        if let Some(selection) = manager.get_player_selection(local_index) {
                            let _ = selection.select_objects(
                                vec![object_id],
                                crate::commands::selection::SelectionType::Add,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Deselect drawable object.
    pub fn deselect_drawable(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) {
        if let Ok(mut guard) = drawable.write() {
            guard.set_selected(false);
        }
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.last_selection_frame = TheGameLogic::get_frame();
        }

        let object_id = drawable.get_object_id();
        if object_id != INVALID_ID {
            if let Ok(list) = crate::player::player_list().read() {
                let local_index = list.get_local_player_index();
                if local_index != crate::player::PLAYER_INDEX_INVALID {
                    let selection_manager = crate::commands::selection::get_selection_manager();
                    let manager_lock = selection_manager.write();
                    if let Ok(mut manager) = manager_lock {
                        if let Some(selection) = manager.get_player_selection(local_index) {
                            let _ = selection.select_objects(
                                vec![object_id],
                                crate::commands::selection::SelectionType::Remove,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Set displayed maximum warning.
    pub fn set_displayed_max_warning(show: bool) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.displayed_max_warning = show;
        }
    }

    /// Display floating combat text.
    pub fn add_floating_text(
        text: &str,
        position: &Coord3D,
        color: crate::common::Color,
    ) -> Result<(), GameError> {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.floating_texts.push(FloatingTextEntry {
                text: text.to_string(),
                position: *position,
                color,
                created_frame: TheGameLogic::get_frame(),
            });
        }
        Ok(())
    }

    pub fn add_world_animation(
        animation_name: &str,
        position: &Coord3D,
        fade_on_expire: bool,
        duration_seconds: Real,
        z_rise_per_second: Real,
    ) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.world_animations.push(WorldAnimationEntry {
                animation_name: animation_name.to_string(),
                position: *position,
                fade_on_expire,
                duration_seconds,
                z_rise_per_second,
                created_frame: TheGameLogic::get_frame(),
            });
        }
    }

    pub fn take_world_animations() -> Vec<WorldAnimationEntry> {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            return std::mem::take(&mut state.world_animations);
        }
        Vec::new()
    }

    /// Display a message to the player.
    /// Matches C++ InGameUI message display functionality
    pub fn display_message(message: &str) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.messages.push(message.to_string());
        }
        log::info!("UI Message: {}", message);
    }

    pub fn add_superweapon(
        player_index: Int,
        power_name: String,
        object_id: ObjectID,
        template: &SpecialPowerTemplate,
    ) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.superweapons.retain(|entry| {
                !(entry.player_index == player_index
                    && entry.object_id == object_id
                    && entry.template_id == template.get_id())
            });
            state.superweapons.push(SuperweaponEntry {
                player_index,
                power_name,
                object_id,
                template_id: template.get_id(),
            });
        }
    }

    pub fn remove_superweapon(
        player_index: Int,
        power_name: String,
        object_id: ObjectID,
        template: &SpecialPowerTemplate,
    ) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.superweapons.retain(|entry| {
                !(entry.player_index == player_index
                    && entry.object_id == object_id
                    && entry.template_id == template.get_id()
                    && entry.power_name == power_name)
            });
        }
    }

    /// Remove a worker from the idle worker UI list.
    pub fn remove_idle_worker(object: &crate::object::Object, player_index: Int) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state
                .idle_worker_removals
                .push((object.get_id(), player_index));
        }
    }

    /// Add a worker to the idle worker UI list.
    pub fn add_idle_worker(object: &crate::object::Object, player_index: Int) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state
                .idle_worker_additions
                .push((object.get_id(), player_index));
        }
    }

    /// Drain pending idle worker add/remove events (matches InGameUI idle worker bookkeeping).
    pub fn take_idle_worker_events() -> (Vec<(ObjectID, Int)>, Vec<(ObjectID, Int)>) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            let additions = std::mem::take(&mut state.idle_worker_additions);
            let removals = std::mem::take(&mut state.idle_worker_removals);
            return (additions, removals);
        }
        (Vec::new(), Vec::new())
    }

    /// C++ parity: InGameUI::clearPopupMessageData().
    /// Sets a flag consumed by GameClient's popup handler.
    pub fn request_popup_message_clear() {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.popup_clear_requested = true;
        }
    }

    /// Consume and return whether a popup clear was requested.
    /// Called by GameClient's popup handler each frame.
    pub fn consume_popup_clear_request() -> bool {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            return std::mem::take(&mut state.popup_clear_requested);
        }
        false
    }
}

pub struct TheRadar;

impl TheRadar {
    pub fn get() -> Option<&'static Self> {
        static RADAR: OnceLock<TheRadar> = OnceLock::new();
        Some(RADAR.get_or_init(|| TheRadar))
    }

    pub fn create_event(
        &self,
        position: &Coord3D,
        event_type: game_engine::common::system::radar::RadarEventType,
        seconds_to_live: Real,
    ) {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.create_event(&world_loc, event_type, seconds_to_live);
        }
    }

    pub fn try_infiltration_event(target: Arc<RwLock<Object>>) -> Result<(), GameError> {
        let Ok(target_guard) = target.read() else {
            return Err(GameError::LockError);
        };
        Self::try_infiltration_event_for_object(&target_guard)
    }

    /// Borrow-first infiltration radar event (no Arc at the call site).
    pub fn try_infiltration_event_for_object(target: &Object) -> Result<(), GameError> {
        if target.is_destroyed() {
            return Ok(());
        }
        if !target.is_locally_controlled() {
            return Ok(());
        }

        let position = *target.get_position();

        let radar = get_radar_system();
        if let Ok(mut guard) = radar.write() {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.try_infiltration_event(&world_loc);
        }
        Ok(())
    }

    /// Borrow-first ID variant.
    pub fn try_infiltration_event_id(target_id: ObjectID) -> Result<(), GameError> {
        // Wave 281: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |target| {
                Self::try_infiltration_event_for_object(target)
            })
            .unwrap_or(Ok(()))
    }

    pub fn refresh_terrain(&self) {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            guard.refresh_terrain();
        }
    }
}

/// C++ `TheTacticalView` camera constraint hook used by SWITCH_BORDER.
pub struct TheTacticalView;

static FORCE_CAMERA_CONSTRAINT_RECALC: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();

impl TheTacticalView {
    pub fn set_force_camera_constraint_recalc(hook: fn()) {
        let _ = FORCE_CAMERA_CONSTRAINT_RECALC.set(hook);
    }

    pub fn force_camera_constraint_recalc() {
        if let Some(hook) = FORCE_CAMERA_CONSTRAINT_RECALC.get() {
            hook();
        }
    }
}


/// Terrain visual effects bridge (matching C++ TheTerrainVisual).
pub struct TheTerrainVisual;

static TERRAIN_VISUAL_RAW_HEIGHT: std::sync::LazyLock<Mutex<Option<fn(i32, i32, i32)>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
static TERRAIN_VISUAL_LIGHTING_CHANGED: std::sync::LazyLock<Mutex<Option<fn()>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
static TERRAIN_VISUAL_ADD_PROP: std::sync::LazyLock<
    Mutex<Option<fn(u32, [f32; 3], f32, f32, &str)>>,
> = std::sync::LazyLock::new(|| Mutex::new(None));
static TERRAIN_VISUAL_PENDING_PROPS: std::sync::LazyLock<
    Mutex<Vec<(u32, [f32; 3], f32, f32, String)>>,
> = std::sync::LazyLock::new(|| Mutex::new(Vec::new()));

/// Register the live GameClient `W3DTerrainVisual::setRawMapHeight` hook.
pub fn register_terrain_visual_raw_height_hook(hook: Option<fn(i32, i32, i32)>) {
    if let Ok(mut slot) = TERRAIN_VISUAL_RAW_HEIGHT.lock() {
        *slot = hook;
    }
}

/// Register the live GameClient `staticLightingChanged` hook.
pub fn register_terrain_visual_lighting_changed_hook(hook: Option<fn()>) {
    if let Ok(mut slot) = TERRAIN_VISUAL_LIGHTING_CHANGED.lock() {
        *slot = hook;
    }
}

/// Register the live GameClient `TheTerrainRenderObject->addProp` hook.
pub fn register_terrain_visual_add_prop_hook(
    hook: Option<fn(u32, [f32; 3], f32, f32, &str)>,
) {
    if let Ok(mut slot) = TERRAIN_VISUAL_ADD_PROP.lock() {
        *slot = hook;
    }
}

impl TheTerrainVisual {
    pub fn get() -> Option<&'static Self> {
        static VISUAL: OnceLock<TheTerrainVisual> = OnceLock::new();
        Some(VISUAL.get_or_init(|| TheTerrainVisual))
    }

    /// C++ `W3DTerrainVisual::setRawMapHeight` — playable-grid coords.
    pub fn set_raw_map_height(&self, x: i32, y: i32, height: i32) {
        if let Ok(slot) = TERRAIN_VISUAL_RAW_HEIGHT.lock() {
            if let Some(hook) = *slot {
                hook(x, y, height);
            }
        }
    }

    /// C++ `HeightMapRenderObjClass::staticLightingChanged`.
    pub fn static_lighting_changed(&self) {
        if let Ok(slot) = TERRAIN_VISUAL_LIGHTING_CHANGED.lock() {
            if let Some(hook) = *slot {
                hook();
            }
        }
    }

    /// C++ `TheTerrainRenderObject->addProp(drawID, pos, orientation, scale, modelName)`.
    pub fn add_prop(
        &self,
        drawable_id: u32,
        position: Coord3D,
        angle: Real,
        scale: Real,
        model_name: &str,
    ) {
        if model_name.is_empty() {
            return;
        }
        let location = [position.x, position.y, position.z];
        let mut forwarded = false;
        if let Ok(slot) = TERRAIN_VISUAL_ADD_PROP.lock() {
            if let Some(hook) = *slot {
                hook(drawable_id, location, angle, scale, model_name);
                forwarded = true;
            }
        }
        if !forwarded {
            if let Ok(mut pending) = TERRAIN_VISUAL_PENDING_PROPS.lock() {
                pending.push((drawable_id, location, angle, scale, model_name.to_string()));
            }
        }
    }

    /// Drain props queued before the GameClient terrain hook was registered.
    pub fn take_pending_props(&self) -> Vec<(u32, [f32; 3], f32, f32, String)> {
        TERRAIN_VISUAL_PENDING_PROPS
            .lock()
            .map(|mut pending| pending.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn add_water_velocity(&self, _x: Real, _y: Real, _velocity: Real, _preferred_height: Real) {
        let frame = TheGameLogic::get_frame();
        let mut impulses = WATER_VELOCITY_IMPULSES
            .lock()
            .expect("Water impulse lock poisoned");
        if impulses.len() >= MAX_WATER_VELOCITY_IMPULSES {
            impulses.remove(0);
        }
        impulses.push(WaterVelocityImpulse {
            x: _x,
            y: _y,
            velocity: _velocity,
            preferred_height: _preferred_height,
            frame,
        });

        log::debug!(
            "Water velocity impulse at ({:.1}, {:.1}) v={:.2} h={:.2} frame={}",
            _x,
            _y,
            _velocity,
            _preferred_height,
            frame
        );
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct WaterVelocityImpulse {
    x: Real,
    y: Real,
    velocity: Real,
    preferred_height: Real,
    frame: UnsignedInt,
}

const MAX_WATER_VELOCITY_IMPULSES: usize = 128;
static WATER_VELOCITY_IMPULSES: Lazy<Mutex<Vec<WaterVelocityImpulse>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaEvent {
    LowPower,
    InsufficientFunds,
    SuperweaponDetectedOwnParticleCannon,
    SuperweaponDetectedOwnNuke,
    SuperweaponDetectedOwnScudStorm,
    SuperweaponDetectedAllyParticleCannon,
    SuperweaponDetectedAllyNuke,
    SuperweaponDetectedAllyScudStorm,
    SuperweaponDetectedEnemyParticleCannon,
    SuperweaponDetectedEnemyNuke,
    SuperweaponDetectedEnemyScudStorm,
    SuperweaponLaunchedOwnParticleCannon,
    SuperweaponLaunchedOwnNuke,
    SuperweaponLaunchedOwnScudStorm,
    SuperweaponLaunchedAllyParticleCannon,
    SuperweaponLaunchedAllyNuke,
    SuperweaponLaunchedAllyScudStorm,
    SuperweaponLaunchedEnemyParticleCannon,
    SuperweaponLaunchedEnemyNuke,
    SuperweaponLaunchedEnemyScudStorm,
    SuperweaponReadyOwnParticleCannon,
    SuperweaponReadyOwnNuke,
    SuperweaponReadyOwnScudStorm,
    SuperweaponReadyAllyParticleCannon,
    SuperweaponReadyAllyNuke,
    SuperweaponReadyAllyScudStorm,
    SuperweaponReadyEnemyParticleCannon,
    SuperweaponReadyEnemyNuke,
    SuperweaponReadyEnemyScudStorm,
    BuildingLost,
    BaseUnderAttack,
    AllyUnderAttack,
    BeaconDetected,
    EnemyBlackLotusDetected,
    EnemyJarmenKellDetected,
    EnemyColonelBurtonDetected,
    OwnBlackLotusDetected,
    OwnJarmenKellDetected,
    OwnColonelBurtonDetected,
    UnitLost,
    GeneralLevelUp,
    VehicleStolen,
    BuildingStolen,
    CashStolen,
    UpgradeComplete,
    BuildingBeingStolen,
    BuildingSabotaged,
    SuperweaponLaunchedOwnGpsScrambler,
    SuperweaponLaunchedAllyGpsScrambler,
    SuperweaponLaunchedEnemyGpsScrambler,
    SuperweaponLaunchedOwnSneakAttack,
    SuperweaponLaunchedAllySneakAttack,
    SuperweaponLaunchedEnemySneakAttack,
}

#[derive(Debug, Default)]
struct EvaState {
    enabled: bool,
    queued: Vec<EvaEvent>,
}

pub struct TheEva;

impl TheEva {
    fn state() -> &'static Mutex<EvaState> {
        static EVA_STATE: OnceLock<Mutex<EvaState>> = OnceLock::new();
        EVA_STATE.get_or_init(|| {
            Mutex::new(EvaState {
                enabled: true,
                queued: Vec::new(),
            })
        })
    }

    pub fn set_should_play(event: EvaEvent) -> Result<(), GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        if guard.enabled {
            guard.queued.push(event);
        }
        Ok(())
    }

    pub fn set_enabled(enabled: bool) -> Result<(), GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        guard.enabled = enabled;
        if !enabled {
            guard.queued.clear();
        }
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, GameError> {
        let state = Self::state();
        let guard = state.lock().map_err(|_| GameError::LockError)?;
        Ok(guard.enabled)
    }

    pub fn drain_events() -> Result<Vec<EvaEvent>, GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        let drained = guard.queued.drain(..).collect();
        Ok(drained)
    }
}

/// TheScriptEngine singleton facade for minimal global script state.
pub struct TheScriptEngine;

impl TheScriptEngine {
    pub fn is_game_ending() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_game_ending()))
            .unwrap_or(false)
    }

    pub fn set_global_difficulty(difficulty: Int) {
        GLOBAL_DIFFICULTY.store(difficulty, Ordering::Relaxed);
        let mapped = match difficulty {
            0 => crate::player::GameDifficulty::Easy,
            1 => crate::player::GameDifficulty::Normal,
            2 => crate::player::GameDifficulty::Hard,
            3 => crate::player::GameDifficulty::Brutal,
            _ => crate::player::GameDifficulty::Normal,
        };
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_global_difficulty(mapped);
            }
        }
    }

    pub fn get_global_difficulty() -> Int {
        GLOBAL_DIFFICULTY.load(Ordering::Relaxed)
    }

    pub fn signal_ui_interact(hook_name: &str) {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.signal_ui_interact(hook_name);
            }
        }
    }

    pub fn notify_of_object_creation_or_destruction() {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.notify_of_object_creation_or_destruction();
            }
        }
    }

    pub fn notify_of_completed_video(video_name: &str) {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.notify_of_completed_video(video_name);
            }
        }
    }

    pub fn is_video_complete(video_name: &str, remove_from_list: bool) -> bool {
        crate::scripting::engine::get_script_engine()
            .write()
            .ok()
            .and_then(|mut guard| {
                guard
                    .as_mut()
                    .map(|engine| engine.is_video_complete(video_name, remove_from_list))
            })
            .unwrap_or(false)
    }

    pub fn is_time_frozen_script() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen_script()))
            .unwrap_or(false)
    }

    pub fn is_time_frozen_debug() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen_debug()))
            .unwrap_or(false)
    }

    pub fn is_time_frozen() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen()))
            .unwrap_or(false)
    }
}

/// TheVictoryConditions singleton facade for minimal victory state.
pub struct TheVictoryConditions;

impl TheVictoryConditions {
    pub fn set_local_allied_victory(victory: Bool) {
        LOCAL_ALLIED_VICTORY.store(victory, Ordering::Relaxed);
    }

    pub fn is_local_allied_victory() -> Bool {
        LOCAL_ALLIED_VICTORY.load(Ordering::Relaxed)
    }
}
