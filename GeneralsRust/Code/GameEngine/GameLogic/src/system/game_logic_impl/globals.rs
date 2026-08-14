fn map_collision_geometry(
    info: &crate::common::GeometryInfo,
    template_type: Option<game_engine::system::geometry::GeometryType>,
) -> CollisionGeometryInfo {
    let dx = info.bounds.max.x - info.bounds.min.x;
    let dy = info.bounds.max.y - info.bounds.min.y;
    let dz = info.bounds.max.z - info.bounds.min.z;
    let radius = (dx.max(dy) * 0.5).max(0.01);
    let height = dz.max(0.01);
    let is_small = radius < 1.0;
    match template_type {
        Some(game_engine::system::geometry::GeometryType::Sphere) => {
            CollisionGeometryInfo::new_sphere(radius, is_small)
        }
        Some(game_engine::system::geometry::GeometryType::Box) => {
            CollisionGeometryInfo::new_box(dx.max(0.01), dy.max(0.01), is_small)
        }
        Some(game_engine::system::geometry::GeometryType::Cylinder) => {
            CollisionGeometryInfo::new_cylinder(radius, height, is_small)
        }
        None => {
            if height <= radius * 0.5 {
                CollisionGeometryInfo::new_sphere(radius, is_small)
            } else {
                CollisionGeometryInfo::new_cylinder(radius, height, is_small)
            }
        }
    }
}

fn build_global_weapon_bonus_set() -> WeaponBonusSet {
    let mut set = WeaponBonusSet::new();
    let Some(global_data) = game_engine::common::ini::get_global_data() else {
        return set;
    };

    let data = global_data.read();
    for entry in &data.weapon_bonus_entries {
        let Some(condition) = parse_bonus_condition(&entry.condition) else {
            continue;
        };
        let Some(field) = parse_bonus_field(&entry.field) else {
            continue;
        };

        let mut bonus = set
            .get_bonus(condition)
            .cloned()
            .unwrap_or_else(WeaponBonus::new);
        bonus.set_field(field, entry.value);
        set.set_bonus(condition, bonus);
    }

    set
}

fn parse_bonus_condition(value: &str) -> Option<WeaponBonusConditionType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GARRISONED" => Some(WeaponBonusConditionType::Garrisoned),
        "HORDE" => Some(WeaponBonusConditionType::Horde),
        "CONTINUOUS_FIRE_MEAN" => Some(WeaponBonusConditionType::ContinuousFireMean),
        "CONTINUOUS_FIRE_FAST" => Some(WeaponBonusConditionType::ContinuousFireFast),
        "NATIONALISM" => Some(WeaponBonusConditionType::Nationalism),
        "PLAYER_UPGRADE" => Some(WeaponBonusConditionType::PlayerUpgrade),
        "DRONE_SPOTTING" => Some(WeaponBonusConditionType::DroneSpotting),
        "DEMORALIZED" => Some(WeaponBonusConditionType::Demoralized),
        "DEMORALIZED_OBSOLETE" => Some(WeaponBonusConditionType::Demoralized),
        "ENTHUSIASTIC" => Some(WeaponBonusConditionType::Enthusiastic),
        "VETERAN" => Some(WeaponBonusConditionType::Veteran),
        "ELITE" => Some(WeaponBonusConditionType::Elite),
        "HERO" => Some(WeaponBonusConditionType::Hero),
        "BATTLEPLAN_BOMBARDMENT" => Some(WeaponBonusConditionType::BattleplanBombardment),
        "BATTLEPLAN_HOLDTHELINE" => Some(WeaponBonusConditionType::BattleplanHoldtheLine),
        "BATTLEPLAN_SEARCHANDDESTROY" => Some(WeaponBonusConditionType::BattleplanSearchAndDestroy),
        "SUBLIMINAL" => Some(WeaponBonusConditionType::Subliminal),
        "SOLO_HUMAN_EASY" => Some(WeaponBonusConditionType::SoloHumanEasy),
        "SOLO_HUMAN_NORMAL" => Some(WeaponBonusConditionType::SoloHumanNormal),
        "SOLO_HUMAN_HARD" => Some(WeaponBonusConditionType::SoloHumanHard),
        "SOLO_AI_EASY" => Some(WeaponBonusConditionType::SoloAiEasy),
        "SOLO_AI_NORMAL" => Some(WeaponBonusConditionType::SoloAiNormal),
        "SOLO_AI_HARD" => Some(WeaponBonusConditionType::SoloAiHard),
        "TARGET_FAERIE_FIRE" => Some(WeaponBonusConditionType::TargetFaerieFire),
        "FANATICISM" => Some(WeaponBonusConditionType::Fanaticism),
        "FRENZY_ONE" => Some(WeaponBonusConditionType::FrenzyOne),
        "FRENZY_TWO" => Some(WeaponBonusConditionType::FrenzyTwo),
        "FRENZY_THREE" => Some(WeaponBonusConditionType::FrenzyThree),
        "DRONE_SPOT_FOR_STRIKE" => Some(WeaponBonusConditionType::DroneSpotting),
        _ => None,
    }
}

fn parse_bonus_field(value: &str) -> Option<WeaponBonusField> {
    match value.trim().to_ascii_uppercase().as_str() {
        "DAMAGE" => Some(WeaponBonusField::Damage),
        "RADIUS" => Some(WeaponBonusField::Radius),
        "RANGE" => Some(WeaponBonusField::Range),
        "RATE_OF_FIRE" => Some(WeaponBonusField::RateOfFire),
        "PRE_ATTACK" | "PREATTACK" => Some(WeaponBonusField::PreAttack),
        _ => None,
    }
}

struct GameLogicEnergyLookup;

impl EnergyObjectLookup for GameLogicEnergyLookup {
    fn energy_production(&self, obj: ObjectHandle) -> i32 {
        // Wave 344: dual_world_registry_unavailable is not an early skip;
        // try OBJECT_REGISTRY then GameLogic.objects, else 0.
        let object_id = obj.value() as ObjectID;
        if let Some(value) = OBJECT_REGISTRY.with_object(object_id, |guard| {
            guard.get_template().get_energy_production()
        }) {
            return value;
        }
        get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(object_id))
            .and_then(|arc| {
                arc.read()
                    .ok()
                    .map(|guard| guard.get_template().get_energy_production())
            })
            .unwrap_or(0)
    }

    fn energy_bonus(&self, obj: ObjectHandle) -> i32 {
        // Wave 344: dual_world_registry_unavailable is not an early skip;
        // try OBJECT_REGISTRY then GameLogic.objects, else 0.
        let object_id = obj.value() as ObjectID;
        if let Some(value) =
            OBJECT_REGISTRY.with_object(object_id, |guard| guard.get_template().get_energy_bonus())
        {
            return value;
        }
        get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(object_id))
            .and_then(|arc| {
                arc.read()
                    .ok()
                    .map(|guard| guard.get_template().get_energy_bonus())
            })
            .unwrap_or(0)
    }
}

struct GameLogicEnergyOwnerCallbacks;

impl EnergyOwnerCallbacks for GameLogicEnergyOwnerCallbacks {
    fn on_power_brown_out_change(&self, player: PlayerHandle, brown_out: bool) {
        let player_id = player.value() as PlayerIndex;
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id) {
                if let Ok(mut guard) = player_arc.write() {
                    let _ = guard.on_power_brown_out_change(brown_out);
                }
            }
        }
    }
}

fn install_energy_integration() {
    let _ = set_energy_object_lookup(Arc::new(GameLogicEnergyLookup));
    let _ = set_energy_owner_callbacks(Arc::new(GameLogicEnergyOwnerCallbacks));
}

fn install_save_game_counter_integration() {
    register_object_id_counter_hooks(
        Some(Arc::new(|| {
            game_logic_mutex()
                .lock()
                .map(|logic| logic.get_object_id_counter())
                .unwrap_or(1)
        })),
        Some(Arc::new(|next_id| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_object_id_counter(next_id);
            }
        })),
    );

    register_save_load_lifecycle_hooks(
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.clear_all_objects();
                logic.rebuild_spatial_index();
                logic.set_loading_map(true);
            }
        })),
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_map(false);
            }
        })),
        Some(Arc::new(|loading| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_save(loading);
            }
        })),
        Some(Arc::new(|| {
            game_logic_mutex()
                .lock()
                .map(|logic| logic.get_game_mode())
                .unwrap_or(GAME_NONE)
        })),
        Some(Arc::new(|game_mode| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_game_mode(game_mode);
            }
        })),
        Some(Arc::new(|| {
            let map_name = game_engine::common::ini::get_global_data()
                .map(|data| data.read().map_name.clone())
                .unwrap_or_default();
            if !map_name.is_empty() {
                if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
                    if terrain.load_map(AsciiString::from(map_name.as_str()), false) {
                        terrain.new_map(true);
                    }
                }
            }
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.set_loading_map(false);
            }
        })),
        Some(Arc::new(|| {
            if let Ok(mut logic) = game_logic_mutex().lock() {
                logic.rebuild_spatial_index();
                let _ = logic.update_partition_manager();
                logic.rebuild_selection_cache();
            }
            let _ = with_ai_integration_mut(|ai| {
                let _ = ai.new_map();
            });
        })),
    );

    register_save_load_mission_hooks(
        Some(Arc::new(|| {
            let _ = crate::helpers::TheGameLogic::clear_game_data();
        })),
        None,
    );

    register_save_lock_ghost_objects_hook(Some(Arc::new(|enable| {
        if let Ok(mut manager) = THE_GHOST_OBJECT_MANAGER.write() {
            manager.save_lock_ghost_objects(enable);
        }
        if let Ok(mut manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
            manager.set_save_lock_ghost_objects(enable);
        }
    })));

    register_game_logic_snapshot_block();
}

// =============================================================================
// Global Singleton Access
// =============================================================================

static GAME_LOGIC: OnceLock<Mutex<GameLogic>> = OnceLock::new();
static SAVE_GAME_COUNTER_HOOKS: OnceLock<()> = OnceLock::new();

fn game_logic_mutex() -> &'static Mutex<GameLogic> {
    SAVE_GAME_COUNTER_HOOKS.get_or_init(|| {
        install_save_game_counter_integration();
    });
    GAME_LOGIC.get_or_init(|| Mutex::new(GameLogic::default()))
}

/// Get the global GameLogic singleton
pub fn get_game_logic() -> &'static Mutex<GameLogic> {
    crate::system::object_data_provider::ensure_object_data_provider();
    game_logic_mutex()
}

/// Try to fetch the current simulation frame from the global GameLogic instance.
pub fn try_current_frame() -> Result<UnsignedInt, String> {
    game_logic_mutex()
        .lock()
        .map(|logic| logic.get_frame())
        .map_err(|_| "GameLogic mutex poisoned".to_string())
}

/// Convenience helper that returns the current frame, defaulting to 0 if unavailable.
pub fn current_frame() -> UnsignedInt {
    try_current_frame().unwrap_or(0)
}

/// Initialize the GameLogic singleton
pub fn init_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    guard.init();
    Ok(())
}

/// Reset GameLogic state
pub fn reset_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    guard.reset();
    Ok(())
}

/// Step the GameLogic singleton
pub fn update_game_logic() -> Result<(), String> {
    let mut guard = game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())?;
    let frame = guard.get_frame();
    guard
        .update(frame)
        .map_err(|e| format!("GameLogic update failed: {}", e))?;
    if guard.last_update_was_empty_noop() {
        debug!(
            "GameLogic crate tick was empty-world no-op (count={}); not a C++ GameLogic.cpp frame",
            guard.empty_world_tick_count()
        );
    }
    Ok(())
}

/// Convenience helper for callers that need a mutable guard
pub fn lock_game_logic() -> Result<MutexGuard<'static, GameLogic>, String> {
    game_logic_mutex()
        .lock()
        .map_err(|_| "GameLogic mutex poisoned".to_string())
}
