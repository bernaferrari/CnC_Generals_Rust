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
    /// Pending DISABLE_INPUT / ENABLE_INPUT cinematic lock (None = no request).
    cinematic_input_lock: Option<bool>,
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
    /// Drain displayed UI messages (C++ `TheInGameUI->message` output; the HUD
    /// consumes these, tests and hosts use this to observe/clear them).
    pub fn drain_displayed_messages() -> Vec<String> {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            std::mem::take(&mut state.messages)
        } else {
            Vec::new()
        }
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

    /// C++ ScriptActions::doDisableInput / doEnableInput.
    /// `true` = disable (teardown), `false` = enable (restore mouse).
    pub fn request_cinematic_input_lock(disable: bool) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.cinematic_input_lock = Some(disable);
        }
    }

    /// Consume a pending cinematic input-lock request.
    pub fn take_cinematic_input_lock() -> Option<bool> {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            return state.cinematic_input_lock.take();
        }
        None
    }

    /// C++ `InGameUI::objectChangedTeam` — move superweapon timers with the object.
    pub fn object_changed_team(object_id: ObjectID, old_player: i32, new_player: i32) {
        if old_player < 0 || new_player < 0 {
            return;
        }
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            for entry in &mut state.superweapons {
                if entry.object_id == object_id && entry.player_index == old_player {
                    entry.player_index = new_player;
                }
            }
        }
    }
}

pub struct TheRadar;

impl TheRadar {
    pub fn get() -> Option<&'static Self> {
        static RADAR: OnceLock<TheRadar> = OnceLock::new();
        let radar = RADAR.get_or_init(|| TheRadar);
        ensure_radar_event_feedback_registered();
        Some(radar)
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
    /// C++ `Radar::tryEvent` — throttled creation (10s map-wide same-type window).
    pub fn try_event(
        &self,
        event_type: game_engine::common::system::radar::RadarEventType,
        position: &Coord3D,
    ) -> bool {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            return guard.try_event(event_type, &world_loc);
        }
        false
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
        // C++ Radar.cpp:1243 — only warn if the victim is the local player.
        if !target.is_locally_controlled() {
            return Ok(());
        }

        let position = *target.get_position();
        let player_index = target
            .get_controlling_player()
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
            .unwrap_or(-1);
        let victim = game_engine::common::system::radar::RadarVictimInfo {
            is_local_player: true,
            player_index,
            ..Default::default()
        };

        let radar = get_radar_system();
        if let Ok(mut guard) = radar.write() {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.try_infiltration_event_for(&world_loc, Some(&victim));
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

    /// C++ `Radar::tryUnderAttackEvent` — ping + glow + per-kind UI/audio/EVA.
    pub fn try_under_attack_event(target: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        let Ok(target_guard) = target.read() else {
            return Err(GameError::LockError);
        };
        Self::try_under_attack_event_for_object(&target_guard)
    }

    pub fn try_under_attack_event_for_object(target: &Object) -> Result<bool, GameError> {
        // C++ TheRadar/TheInGameUI/TheAudio/TheEva are always live; the Rust
        // client-side hooks must be registered before the pipeline fires.
        ensure_radar_event_feedback_registered();
        if target.is_destroyed() {
            return Ok(false);
        }
        let position = *target.get_position();
        let player_index = target
            .get_controlling_player()
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
            .unwrap_or(-1);
        let victim = game_engine::common::system::radar::RadarVictimInfo {
            is_infantry: target.is_kind_of(KindOf::Infantry),
            is_vehicle: target.is_kind_of(KindOf::Vehicle),
            is_harvester: target.is_kind_of(KindOf::Harvester),
            is_structure: target.is_kind_of(KindOf::Structure),
            is_mp_count_for_victory: target.is_kind_of(KindOf::CountsForVictory),
            is_local_player: target.is_locally_controlled(),
            // C++ Radar.cpp:1199 ally EVA needs the local player's relationship;
            // the damage gate (Object.cpp:1853) only admits local victims, so the
            // ally branch is unreachable through this engine path.
            is_ally: false,
            player_index,
        };
        let radar = get_radar_system();
        let created = if let Ok(mut guard) = radar.write() {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.try_under_attack_event_for(&world_loc, Some(&victim))
        } else {
            false
        };
        Ok(created)
    }


    pub fn refresh_terrain(&self) {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            guard.refresh_terrain();
        }
    }

    pub fn queue_terrain_refresh(&self) {
        let radar = get_radar_system();
        if let Ok(mut guard) = radar.write() {
            guard.queue_terrain_refresh();
        }
    }
}

/// C++ `TheControlBar->triggerRadarAttackGlow()` leftover + live consume hook.
pub struct TheControlBar;

static RADAR_ATTACK_GLOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl TheControlBar {
    pub fn trigger_radar_attack_glow() {
        RADAR_ATTACK_GLOW.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Consume a pending glow request (ControlBar::update).
    pub fn take_radar_attack_glow() -> bool {
        RADAR_ATTACK_GLOW.swap(false, std::sync::atomic::Ordering::SeqCst)
    }
}

struct LeftoverRadarEventFeedback;

impl game_engine::common::system::radar::RadarEventFeedback for LeftoverRadarEventFeedback {
    fn trigger_radar_attack_glow(&self) {
        TheControlBar::trigger_radar_attack_glow();
    }

    fn show_radar_message(&self, message_key: &str) {
        TheInGameUI::display_message(message_key);
    }

    fn play_radar_audio(&self, event_name: &str, player_index: i32) {
        let Some(audio) = TheAudio::get() else {
            return;
        };
        let mapped = match event_name {
            "RadarHarvesterUnderAttackSound" => "RadarNotifyHarvesterUnderAttackSound",
            "RadarStructureUnderAttackSound" => "RadarNotifyStructureUnderAttackSound",
            "RadarInfiltrationSound" => "RadarNotifyInfiltrationSound",
            other => other,
        };
        let mut event = AudioEventRts::new(mapped);
        if player_index >= 0 {
            event.set_player_index(player_index as u32);
        }
        audio.add_audio_event(&event);
    }

    fn set_eva_should_play(&self, eva_name: &str) {
        let event = match eva_name {
            "EVA_BaseUnderAttack" => EvaEvent::BaseUnderAttack,
            "EVA_AllyUnderAttack" => EvaEvent::AllyUnderAttack,
            _ => return,
        };
        let _ = TheEva::set_should_play(event);
    }
}

fn ensure_radar_event_feedback_registered() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    let _ = REGISTERED.get_or_init(|| {
        let _ = game_engine::common::system::radar::register_radar_event_feedback(Arc::new(
            LeftoverRadarEventFeedback,
        ));
    });
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

    /// Drain leftover `addWaterVelocity` impulses into live `TerrainVisualImpl`.
    pub fn take_water_velocity_impulses(&self) -> Vec<(Real, Real, Real, Real)> {
        WATER_VELOCITY_IMPULSES
            .lock()
            .map(|mut impulses| {
                impulses
                    .drain(..)
                    .map(|impulse| {
                        (
                            impulse.x,
                            impulse.y,
                            impulse.velocity,
                            impulse.preferred_height,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// C++ `TheTerrainVisual->enableWaterGrid`.
    pub fn enable_water_grid(&self, enable: bool) {
        crate::terrain_water::visual_enable_water_grid(enable);
    }

    /// C++ `TheTerrainVisual->setWaterGridHeightClamps`.
    pub fn set_water_grid_height_clamps(&self, low: Real, high: Real) {
        crate::terrain_water::visual_set_height_clamps(low, high);
    }

    /// C++ `TheTerrainVisual->setWaterTransform(NULL, angle, x, y, z)`.
    pub fn set_water_transform(&self, angle: Real, x: Real, y: Real, z: Real) {
        crate::terrain_water::visual_set_transform(angle, x, y, z);
    }

    /// C++ `TheTerrainVisual->setWaterGridResolution`.
    pub fn set_water_grid_resolution(&self, cells_x: Real, cells_y: Real, cell_size: Real) {
        crate::terrain_water::visual_set_resolution(cells_x, cells_y, cell_size);
    }

    /// C++ `TheTerrainVisual->setWaterAttenuationFactors`.
    pub fn set_water_attenuation_factors(&self, a: Real, b: Real, c: Real, range: Real) {
        crate::terrain_water::visual_set_attenuation(a, b, c, range);
    }

    /// C++ `TheTerrainVisual->getWaterGridHeight`.
    pub fn get_water_grid_height(&self, x: Real, y: Real) -> Option<Real> {
        crate::terrain_water::get_water_grid_height(x, y)
    }

    /// C++ `getWaterTransform` Z translation.
    pub fn get_water_transform_z(&self) -> Real {
        crate::terrain_water::get_transform_z()
    }

    /// C++ `transform.Set_Z_Translation` + `setWaterTransform`.
    pub fn set_water_transform_z(&self, height: Real) {
        crate::terrain_water::set_transform_z(height);
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

    pub fn set_local_allied_defeat(defeat: Bool) {
        LOCAL_ALLIED_DEFEAT.store(defeat, Ordering::Relaxed);
    }

    pub fn set_single_alliance_remaining(remaining: Bool) {
        SINGLE_ALLIANCE_REMAINING.store(remaining, Ordering::Relaxed);
    }

    pub fn is_single_alliance_remaining() -> Bool {
        SINGLE_ALLIANCE_REMAINING.load(Ordering::Relaxed)
    }

    pub fn set_victory_flags_from_live(from_live: Bool) {
        VICTORY_FLAGS_FROM_LIVE.store(from_live, Ordering::Relaxed);
    }

    pub fn set_local_player_defeated(defeated: Bool) {
        LOCAL_PLAYER_DEFEATED.store(defeated, Ordering::Relaxed);
    }


    /// C++ `VictoryConditions::isLocalAlliedDefeat`: last alliance standing
    /// (or observer when that latch is set). Not "all my allies are dead".
    pub fn is_local_allied_defeat() -> Bool {
        if LOCAL_ALLIED_VICTORY.load(Ordering::Relaxed) {
            return false;
        }
        if VICTORY_FLAGS_FROM_LIVE.load(Ordering::Relaxed) {
            return LOCAL_ALLIED_DEFEAT.load(Ordering::Relaxed);
        }
        leftover_player_list_is_local_allied_defeat()
    }

    /// C++ `VictoryConditions::isLocalDefeat`: observer never; else latch.
    pub fn is_local_defeat() -> Bool {
        if leftover_local_player_is_observer() {
            return false;
        }
        if LOCAL_PLAYER_DEFEATED.load(Ordering::Relaxed) {
            return true;
        }
        leftover_local_player_is_individually_defeated()
    }

}


fn leftover_player_is_playable_living(player: &crate::player::Player) -> bool {
    !player.is_player_observer()
        && player.get_player_type() != crate::player::PlayerType::Neutral
        && !player.is_defeated()
}

/// C++ `areAllies`: mutual ALLIES and not the same player.
fn leftover_players_are_allies(
    a: &crate::player::Player,
    b: &crate::player::Player,
) -> bool {
    if a.get_player_index() == b.get_player_index() {
        return false;
    }
    a.is_allied_with_player(b) && b.is_allied_with_player(a)
}

/// C++ `isLocalAlliedDefeat` from leftover PlayerList `is_defeated`.
fn leftover_player_list_is_local_allied_defeat() -> bool {
    let Ok(list) = crate::player::ThePlayerList().read() else {
        return false;
    };
    let Some(local_arc) = list.get_local_player().cloned() else {
        return false;
    };
    let Ok(local) = local_arc.read() else {
        return false;
    };

    let mut first_living: Option<std::sync::Arc<std::sync::RwLock<crate::player::Player>>> = None;
    let mut multiple_alliances = false;
    for player_arc in list.iter() {
        let Ok(player) = player_arc.read() else {
            continue;
        };
        if !leftover_player_is_playable_living(&player) {
            continue;
        }
        if let Some(first_arc) = &first_living {

            let Ok(first) = first_arc.read() else {
                continue;
            };
            if !leftover_players_are_allies(&first, &player) {
                multiple_alliances = true;
                break;
            }
        } else {
            first_living = Some(std::sync::Arc::clone(player_arc));
        }
    }
    drop(list);

    if multiple_alliances {
        return false;
    }
    if local.is_player_observer() {
        return true;
    }
    let Some(first_arc) = first_living else {
        // Everyone playable is dead: C++ m_singleAllianceRemaining + !hasAchievedVictory.
        return true;
    };
    let Ok(alive) = first_arc.read() else {
        return true;
    };
    // Defeat only when the remaining alliance is not the local player's.
    local.get_player_index() != alive.get_player_index()
        && !leftover_players_are_allies(&local, &alive)
}

fn leftover_local_player_is_observer() -> bool {
    let Ok(list) = crate::player::ThePlayerList().read() else {
        return false;
    };
    list.get_local_player()
        .and_then(|player| player.read().ok().map(|guard| guard.is_player_observer()))
        .unwrap_or(false)
}

fn leftover_local_player_is_individually_defeated() -> bool {
    let Ok(list) = crate::player::ThePlayerList().read() else {
        return false;
    };
    list.get_local_player()
        .and_then(|player| {
            player.read().ok().map(|guard| guard.is_defeated() || guard.is_player_dead())
        })
        .unwrap_or(false)
}

