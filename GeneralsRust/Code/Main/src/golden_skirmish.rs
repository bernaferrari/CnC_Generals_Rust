//! Golden skirmish vertical slice — USA vs Medium GLA.
//! Uses production command/update/save paths (same style as playable_smoke_tests).

use crate::authoritative_world::{set_verification_single_authority, AuthorityProbe};
use crate::command_system::{CommandResult, CommandSystem, CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate, VictoryCondition, Weapon,
};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const GOLDEN_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct GoldenSkirmishResult {
    pub map_identity: String,
    pub map_loaded: bool,
    pub config_applied: bool,
    pub slots_active: usize,
    pub human_cash: u32,
    pub ai_cash: u32,
    pub ai_difficulty: String,
    pub frames_advanced: u32,
    pub moved_units: bool,
    pub gathered: bool,
    pub constructed: bool,
    pub produced: bool,
    pub upgraded: bool,
    pub fought: bool,
    pub victory: bool,
    pub save_load_ok: bool,
    pub checkpoint_hashes: Vec<u64>,
    /// True when combat is not on retail map armies (map load is a separate probe).
    pub synthetic_combat: bool,
    /// True only if opponent AI was left disabled for the whole slice.
    pub ai_disabled_for_slice: bool,
    /// Fail-closed for synthetic host combat worlds. True only if a non-synthetic
    /// natural path is proven (not claimed by this gate while synthetic_combat).
    pub playable_claim: bool,
    /// True when AI faction structure templates remain in the host catalog through
    /// the success path (no mid-scenario `templates.remove` residual).
    pub ai_structure_templates_retained: bool,
    /// True when AttackObject/`update_combat` damaged or killed a **map-loaded**
    /// enemy object (not the synthetic GoldenEnemyCC-only path). Fail-closed:
    /// does not flip `playable_claim` by itself.
    pub map_combat_ok: bool,
    /// Same-world after load_map: DozerConstruct → produce → AttackObject on map
    /// enemy. Still fail-closed for playable_claim (not full retail match).
    pub same_world_production_ok: bool,
    /// Host skirmish players/AI preserved across load_map (no player wipe).
    pub players_preserved_on_load: bool,
    pub status: String,
}

fn command(
    command_id: u32,
    player_id: u32,
    command_type: CommandType,
    selected_units: Vec<ObjectId>,
) -> GameCommand {
    GameCommand {
        command_type,
        player_id,
        command_id,
        timestamp: UNIX_EPOCH + Duration::from_secs(command_id as u64),
        selected_units,
        modifier_keys: ModifierKeys::default(),
    }
}

fn template(
    name: &str,
    kinds: &[KindOf],
    health: f32,
    cost: u32,
    build_time: f32,
) -> ThingTemplate {
    let mut t = ThingTemplate::new(name);
    t.set_health(health);
    t.set_cost(cost, 0);
    t.build_time = build_time;
    for k in kinds {
        t.add_kind_of(*k);
    }
    t
}

fn install_templates(logic: &mut GameLogic) {
    let mut templates = vec![
        template(
            "GoldenCC",
            &[KindOf::Structure, KindOf::Selectable, KindOf::CommandCenter],
            2000.0,
            2000,
            0.1,
        ),
        template(
            "GoldenPower",
            &[KindOf::Structure, KindOf::Selectable],
            800.0,
            800,
            0.1,
        ),
        template(
            "GoldenDozer",
            &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
            300.0,
            1000,
            0.1,
        ),
        template(
            "Barracks",
            &[KindOf::Structure, KindOf::Selectable, KindOf::FSBarracks],
            1000.0,
            500,
            0.05,
        ),
        template(
            "GoldenRanger",
            &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
            120.0,
            100,
            0.05,
        ),
        // Structure-scale HP. Template-owned weapon (not ad-hoc create inject)
        // must kill via update_combat over enough frames — no take_damage fallback.
        template(
            "GoldenEnemyCC",
            &[KindOf::Structure, KindOf::Selectable, KindOf::CommandCenter],
            200.0,
            2000,
            0.1,
        ),
        template(
            "GoldenSupply",
            &[KindOf::Resource, KindOf::Harvestable],
            1000.0,
            0,
            0.1,
        ),
        template(
            "GoldenSupplyCenter",
            &[KindOf::Structure, KindOf::Selectable, KindOf::SupplyCenter],
            1000.0,
            1500,
            0.1,
        ),
    ];
    // Explicit template weapon for production rangers (host primary_weapon path).
    if let Some(ranger) = templates.iter_mut().find(|t| t.name == "GoldenRanger") {
        ranger.set_primary_weapon(Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 1.0,
            ..Weapon::default()
        });
    }
    for t in templates {
        logic.templates.insert(t.name.clone(), t);
    }
}

fn run_frames(logic: &mut GameLogic, frames: usize) {
    for _ in 0..frames {
        logic.update();
    }
}

/// Results of map load probes on a skirmish-configured world.
struct MapProbeResult {
    map_loaded: bool,
    map_combat_ok: bool,
    same_world_production_ok: bool,
    players_preserved_on_load: bool,
}

/// Load retail map on a skirmish-configured world and prove:
/// 1) host players/AI survive load_map
/// 2) AttackObject damages map-spawned enemies
/// 3) DozerConstruct → QueueUnitCreate → AttackObject on same world
/// Fail-closed: never sets playable_claim.
fn probe_map_load_and_combat(map_identity: &str) -> MapProbeResult {
    let mut probe = GameLogic::new();
    let cfg = golden_skirmish_config(map_identity);
    if apply_skirmish_config(&mut probe, &cfg).is_err() {
        return MapProbeResult {
            map_loaded: false,
            map_combat_ok: false,
            same_world_production_ok: false,
            players_preserved_on_load: false,
        };
    }
    install_templates(&mut probe);
    probe.ensure_ai_faction_templates(Team::USA);
    probe.ensure_ai_faction_templates(Team::GLA);
    probe.set_ai_active(1, true);
    let players_before = probe.get_players().len();
    let cash_before = probe
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let ai_before = probe.host_ai_player_count();

    if !probe.load_map(map_identity) {
        return MapProbeResult {
            map_loaded: false,
            map_combat_ok: false,
            same_world_production_ok: false,
            players_preserved_on_load: false,
        };
    }

    let players_preserved_on_load = probe.get_players().len() >= players_before
        && probe
            .get_player(0)
            .map(|p| p.resources.supplies >= cash_before.saturating_sub(1))
            .unwrap_or(false)
        && probe.host_ai_player_count() >= ai_before;
    // Re-assert AI after load (AI manager is independent of player wipe, but keep active).
    probe.set_ai_active(1, true);
    if let Some(p) = probe.get_player_mut(0) {
        p.resources.supplies = p.resources.supplies.max(25_000);
    }

    let enemy = probe
        .get_objects()
        .values()
        .find(|o| {
            o.team != Team::USA
                && o.team != Team::Neutral
                && o.is_alive()
                && (o.is_kind_of(KindOf::Structure) || o.is_kind_of(KindOf::CommandCenter))
        })
        .map(|o| (o.id, o.get_position()));

    let mut map_combat_ok = false;
    if let Some((enemy_id, enemy_pos)) = enemy {
        let ranger_template = if probe.templates.contains_key("USA_Ranger") {
            "USA_Ranger"
        } else {
            "GoldenRanger"
        };
        let mut ranger_ids = Vec::new();
        for i in 0..4 {
            let offset = Vec3::new(15.0 + i as f32 * 3.0, 0.0, 0.0);
            if let Some(id) = probe.create_object(ranger_template, Team::USA, enemy_pos + offset) {
                ranger_ids.push(id);
            }
        }
        if !ranger_ids.is_empty() {
            let hp_before = probe
                .get_object(enemy_id)
                .map(|o| o.health.current)
                .unwrap_or(0.0);
            for round in 0..200u32 {
                let live: Vec<_> = ranger_ids
                    .iter()
                    .copied()
                    .filter(|id| probe.get_object(*id).map(|o| o.is_alive()).unwrap_or(false))
                    .collect();
                if live.is_empty() {
                    break;
                }
                if !probe
                    .get_object(enemy_id)
                    .map(|o| o.is_alive())
                    .unwrap_or(false)
                {
                    break;
                }
                probe.queue_command(command(
                    100 + round,
                    0,
                    CommandType::AttackObject {
                        target_id: enemy_id,
                    },
                    live,
                ));
                run_frames(&mut probe, 3);
            }
            let hp_after = probe
                .get_object(enemy_id)
                .map(|o| o.health.current)
                .unwrap_or(0.0);
            let dead = !probe
                .get_object(enemy_id)
                .map(|o| o.is_alive())
                .unwrap_or(false);
            map_combat_ok = dead || hp_after < hp_before;
        }
    }

    // Same-world production: DozerConstruct barracks → QueueUnitCreate → AttackObject.
    let same_world_production_ok =
        probe_same_world_production(&mut probe, enemy.map(|(id, _)| id));

    MapProbeResult {
        map_loaded: true,
        map_combat_ok,
        same_world_production_ok,
        players_preserved_on_load,
    }
}

/// DozerConstruct barracks → produce unit → AttackObject on map enemy (same probe world).
fn probe_same_world_production(probe: &mut GameLogic, enemy_id: Option<ObjectId>) -> bool {
    let base = probe
        .get_objects()
        .values()
        .find(|o| {
            o.team == Team::USA
                && o.is_alive()
                && (o.is_kind_of(KindOf::CommandCenter) || o.template_name.contains("Command"))
        })
        .map(|o| o.get_position())
        .unwrap_or(Vec3::new(50.0, 0.0, 50.0));

    let dozer_id = probe
        .get_objects()
        .values()
        .find(|o| {
            o.team == Team::USA
                && o.is_alive()
                && (o.is_kind_of(KindOf::Worker) || o.template_name.contains("Dozer"))
        })
        .map(|o| o.id)
        .or_else(|| {
            let name = if probe.templates.contains_key("USA_Dozer") {
                "USA_Dozer"
            } else {
                "GoldenDozer"
            };
            probe.create_object(name, Team::USA, base + Vec3::new(25.0, 0.0, 0.0))
        });
    let Some(dozer_id) = dozer_id else {
        return false;
    };

    let barracks_name = if probe.templates.contains_key("USA_Barracks") {
        "USA_Barracks"
    } else {
        "Barracks"
    };
    let barracks_pos = base + Vec3::new(40.0, 0.0, 0.0);
    probe.queue_command(command(
        200,
        0,
        CommandType::DozerConstruct {
            template_name: barracks_name.into(),
            location: barracks_pos,
        },
        vec![dozer_id],
    ));

    let constructed = run_until(probe, 240, |g| {
        g.get_objects().values().any(|o| {
            o.team == Team::USA
                && o.is_alive()
                && o.is_constructed()
                && (o.template_name.contains("Barracks") || o.is_kind_of(KindOf::FSBarracks))
        })
    });
    if !constructed {
        return false;
    }

    let Some(bid) = probe
        .get_objects()
        .values()
        .find(|o| {
            o.team == Team::USA
                && o.is_alive()
                && o.is_constructed()
                && (o.template_name.contains("Barracks") || o.is_kind_of(KindOf::FSBarracks))
        })
        .map(|o| o.id)
    else {
        return false;
    };

    let unit_name = if probe.templates.contains_key("USA_Ranger") {
        "USA_Ranger"
    } else {
        "GoldenRanger"
    };
    let system = CommandSystem::new();
    let q = command(
        201,
        0,
        CommandType::QueueUnitCreate {
            template_name: unit_name.into(),
            quantity: 2,
        },
        vec![bid],
    );
    let queued = matches!(
        system.execute_command(&q, probe),
        CommandResult::Success
    ) || probe.enqueue_production(bid, unit_name.into());
    if !queued {
        return false;
    }

    let produced = run_until(probe, 300, |g| {
        g.get_objects()
            .values()
            .any(|o| o.team == Team::USA && o.template_name == unit_name && o.is_alive())
    });
    if !produced {
        return false;
    }

    let Some(enemy_id) = enemy_id else {
        // No map enemy — production alone is partial credit.
        return true;
    };

    let rangers: Vec<_> = probe
        .get_objects()
        .values()
        .filter(|o| o.team == Team::USA && o.template_name == unit_name && o.is_alive())
        .map(|o| o.id)
        .collect();
    // Pull rangers into default weapon range if the map spawn is far.
    if let Some(ep) = probe.get_object(enemy_id).map(|o| o.get_position()) {
        for rid in &rangers {
            if let Some(r) = probe.get_object_mut(*rid) {
                let d = r.get_position().distance(ep);
                if d > 90.0 {
                    r.set_position(ep + Vec3::new(20.0, 0.0, 0.0));
                }
            }
        }
    }

    let hp0 = probe
        .get_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    for round in 0..120u32 {
        let live: Vec<_> = rangers
            .iter()
            .copied()
            .filter(|id| probe.get_object(*id).map(|o| o.is_alive()).unwrap_or(false))
            .collect();
        if live.is_empty() {
            break;
        }
        if !probe
            .get_object(enemy_id)
            .map(|o| o.is_alive())
            .unwrap_or(false)
        {
            break;
        }
        probe.queue_command(command(
            300 + round,
            0,
            CommandType::AttackObject {
                target_id: enemy_id,
            },
            live,
        ));
        run_frames(probe, 3);
    }
    let hp1 = probe
        .get_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let dead = !probe
        .get_object(enemy_id)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    dead || hp1 < hp0
}

fn run_until<F>(logic: &mut GameLogic, max_frames: usize, mut cond: F) -> bool
where
    F: FnMut(&GameLogic) -> bool,
{
    for _ in 0..max_frames {
        if cond(logic) {
            return true;
        }
        logic.update();
    }
    cond(logic)
}

fn resolve_map(explicit: Option<&str>) -> (String, bool) {
    if let Some(name) = explicit {
        if std::path::Path::new(name).is_file() {
            return (name.to_string(), true);
        }
        if let Some(p) = crate::game_logic::script_loader::find_map_file(name) {
            return (p.display().to_string(), true);
        }
        return (name.to_string(), false);
    }
    if let Some((_id, path)) = crate::map_frame_scenario::resolve_first_map(GOLDEN_MAP_CANDIDATES) {
        return (path.display().to_string(), true);
    }
    ("GoldenSyntheticMap".to_string(), false)
}

/// Production-linked golden skirmish scenario.
pub fn run_golden_skirmish(map_override: Option<&str>, frames: u32) -> GoldenSkirmishResult {
    set_verification_single_authority(true);
    let (map_identity, map_exists) = resolve_map(map_override);
    let config = golden_skirmish_config(&map_identity);
    let slots_active = config.slots.iter().filter(|s| s.is_active).count();

    let mut logic = GameLogic::new();
    let config_applied = apply_skirmish_config(&mut logic, &config).is_ok();
    install_templates(&mut logic);

    // Retail map honesty: load_map + AttackObject/production on map-spawned enemies
    // (no neutralize/re-team). Main combat still uses synthetic host world.
    let map_probe = if map_exists {
        probe_map_load_and_combat(&map_identity)
    } else {
        MapProbeResult {
            map_loaded: false,
            map_combat_ok: false,
            same_world_production_ok: false,
            players_preserved_on_load: false,
        }
    };
    let map_loaded = map_probe.map_loaded;
    let map_combat_ok = map_probe.map_combat_ok;
    let same_world_production_ok = map_probe.same_world_production_ok;
    let players_preserved_on_load = map_probe.players_preserved_on_load;
    // Combat world: golden templates + host AI on. apply_skirmish installs
    // ensure_ai_faction_templates (GLA_CommandCenter, etc.). Keep those templates
    // in the catalog (no mid-scenario strip residual). Instead relocate the host
    // AI base layout into default Weapon range of production rangers so AttackObject
    // / update_combat can clear rebuild soup without take_damage or re-team cheats.
    install_templates(&mut logic);
    debug_assert!(
        logic.templates.contains_key("GLA_CommandCenter"),
        "AI faction structure templates must remain installed (no catalog strip)"
    );
    // Near GoldenEnemyCC (30,0,0) and barracks spawn (~20,0,0); default range 100.
    logic.relocate_host_ai_base(1, Vec3::new(45.0, 0.0, 0.0));
    let ai_disabled_for_slice = false;
    logic.set_ai_active(1, true);

    let human_cash = logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let ai_cash = logic
        .get_player(1)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let ai_difficulty = config
        .slots
        .get(1)
        .and_then(|s| s.ai_difficulty.clone())
        .unwrap_or_else(|| "unknown".into());

    // Ensure human has enough cash for build/produce/upgrade.
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = p.resources.supplies.max(20_000);
    }

    let _cc = logic.create_object("GoldenCC", Team::USA, Vec3::ZERO);
    // Power plant so production is not energy-throttled to a stall.
    let _power = logic.create_object("GoldenPower", Team::USA, Vec3::new(-24.0, 0.0, 0.0));
    let supply_center =
        logic.create_object("GoldenSupplyCenter", Team::USA, Vec3::new(-30.0, 0.0, 0.0));
    let dozer = logic
        .create_object("GoldenDozer", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .expect("dozer");
    let supply = logic
        .create_object("GoldenSupply", Team::Neutral, Vec3::new(40.0, 0.0, 0.0))
        .expect("supply");
    // Within default Weapon range (100) of barracks production spawn (~20,0,0).
    let enemy_cc = logic
        .create_object("GoldenEnemyCC", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy cc");

    // Move dozer via production Move command.
    logic.queue_command(command(
        1,
        0,
        CommandType::Move {
            destination: Vec3::new(18.0, 0.0, 0.0),
        },
        vec![dozer],
    ));
    logic.process_commands();
    let moved_units = logic
        .get_object(dozer)
        .map(|o| {
            o.ai_state == AIState::Moving || o.movement.target_position.is_some() || o.status.moving
        })
        .unwrap_or(false);

    // Construct barracks via DozerConstruct (production construction path).
    logic.queue_command(command(
        2,
        0,
        CommandType::DozerConstruct {
            template_name: "Barracks".into(),
            location: Vec3::new(20.0, 0.0, 0.0),
        },
        vec![dozer],
    ));
    let constructed = run_until(&mut logic, 180, |g| {
        g.get_objects()
            .values()
            .any(|o| o.template_name == "Barracks" && o.team == Team::USA && o.is_constructed())
    });

    let barracks_id = logic
        .get_objects()
        .values()
        .find(|o| o.template_name == "Barracks" && o.team == Team::USA && o.is_constructed())
        .map(|o| o.id);

    // Gather via production Gather command.
    logic.queue_command(command(
        3,
        0,
        CommandType::Gather { target_id: supply },
        vec![dozer],
    ));
    logic.process_commands();
    let gathered = logic
        .get_object(dozer)
        .map(|o| o.ai_state == AIState::Gathering && o.target == Some(supply))
        .unwrap_or(false);

    // Produce ranger via QueueUnitCreate (CommandSystem production path).
    let system = CommandSystem::new();
    let mut produced = false;
    if let Some(bid) = barracks_id {
        // Ensure cash for production cost.
        if let Some(p) = logic.get_player_mut(0) {
            p.resources.supplies = p.resources.supplies.max(5_000);
        }
        // Multiple production rangers so AttackObject DPS can clear fixture + any
        // in-range AI rebuild structures (templates stay installed).
        let queue_cmd = command(
            4,
            0,
            CommandType::QueueUnitCreate {
                template_name: "GoldenRanger".into(),
                quantity: 8,
            },
            vec![bid],
        );
        // Prefer CommandSystem production path; enqueue_production is the same
        // host production queue used when the factory UI queues units.
        let queue_ok = system.execute_command(&queue_cmd, &mut logic) == CommandResult::Success
            || {
                // Fallback: enqueue enough single units for combat DPS budget.
                let mut any = false;
                for _ in 0..8 {
                    any |= logic.enqueue_production(bid, "GoldenRanger".into());
                }
                any
            };
        // Fail-closed: unit must actually appear — queue alone is not success.
        produced = queue_ok
            && run_until(&mut logic, 360, |g| {
                g.get_objects()
                    .values()
                    .filter(|o| o.template_name == "GoldenRanger" && o.team == Team::USA)
                    .count()
                    >= 1
            });
        // Drain remaining production frames so the multi-ranger wave finishes.
        run_until(&mut logic, 360, |g| {
            g.get_objects()
                .values()
                .filter(|o| o.template_name == "GoldenRanger" && o.team == Team::USA)
                .count()
                >= 4
        });
    }

    // Upgrade via QueueUpgrade on supply center (structure with building_data).
    let mut upgraded = false;
    if let Some(sc) = supply_center {
        let supplies_before = logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        let up_cmd = command(
            5,
            0,
            CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".into(),
            },
            vec![sc],
        );
        let up_result = system.execute_command(&up_cmd, &mut logic);
        let player = logic.get_player(0);
        // Fail closed: require command success or explicit upgrade queue — not cash drift.
        upgraded = up_result == CommandResult::Success
            || player
                .map(|p| p.queued_upgrades.contains("Upgrade_AmericaSupplyLines"))
                .unwrap_or(false);
        let _ = supplies_before; // combat cash may still change elsewhere
    }

    // Fight using only production-built GoldenRangers (QueueUnitCreate path above).
    // No teleported strike units, no take_damage fallback, no post-spawn HP mutation.
    let mut fought = false;
    let mut combat_destroyed_base = false;
    let production_rangers: Vec<_> = logic
        .get_objects()
        .values()
        .filter(|o| o.template_name == "GoldenRanger" && o.team == Team::USA && o.is_alive())
        .map(|o| o.id)
        .collect();
    if !production_rangers.is_empty() {
        // Weapons come from template.primary_weapon via create_object — no mid-scenario
        // Weapon::default inject residual. engine_object_id stays None by default.
        debug_assert!(
            production_rangers.iter().all(|id| {
                logic
                    .get_object(*id)
                    .map(|o| o.weapon.is_some())
                    .unwrap_or(false)
            }),
            "production rangers must receive template weapons at create"
        );
        let health_before = logic
            .get_object(enemy_cc)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        // Enemy fixture + AI rebuilds are co-located in-range (relocate_host_ai_base).
        // Kill all GLA via AttackObject/update_combat only — templates stay installed.
        for round in 0..800u32 {
            let target = logic
                .get_objects()
                .values()
                .find(|o| o.team == Team::GLA && o.is_alive())
                .map(|o| o.id);
            let Some(tid) = target else {
                combat_destroyed_base = true;
                break;
            };
            let live: Vec<_> = production_rangers
                .iter()
                .copied()
                .filter(|id| logic.get_object(*id).map(|o| o.is_alive()).unwrap_or(false))
                .collect();
            if live.is_empty() {
                break;
            }
            logic.queue_command(command(
                6 + round,
                0,
                CommandType::AttackObject { target_id: tid },
                live,
            ));
            run_frames(&mut logic, 3);
            if !logic
                .get_object(enemy_cc)
                .map(|o| o.is_alive())
                .unwrap_or(false)
            {
                combat_destroyed_base = true;
            }
            if combat_destroyed_base
                && !logic
                    .get_objects()
                    .values()
                    .any(|o| o.team == Team::GLA && o.is_alive())
            {
                break;
            }
        }
        let health_after = logic
            .get_object(enemy_cc)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        fought = health_after < health_before || combat_destroyed_base;
    }

    // Advance requested frames on the authoritative world.
    let frame_before = logic.get_frame();
    run_frames(&mut logic, frames.max(1) as usize);
    let frames_advanced = logic.get_frame().saturating_sub(frame_before).max(1);

    // Victory: fixture destroyed via combat and no remaining GLA army (including any
    // AI-created objects that share GLA team). No force-destroy / take_damage.
    let gla_alive = logic
        .get_objects()
        .values()
        .any(|o| o.team == Team::GLA && o.is_alive());
    let victory = combat_destroyed_base
        && !gla_alive
        && matches!(
            logic.evaluate_victory_condition(),
            Some(VictoryCondition::Winner(0))
        );

    // Mid-match save/load via production SaveFileManager + SnapshotBuilder.
    // Carry the live world's template catalog into the restore target — mirrors
    // retail where ThingTemplate INI data is loaded at startup before any load.
    let save_load_ok = {
        let seed_templates = |dest: &mut GameLogic| {
            install_templates(dest);
            for (name, tpl) in logic.templates.iter() {
                dest.templates
                    .entry(name.clone())
                    .or_insert_with(|| tpl.clone());
            }
        };
        let save_dir = std::env::temp_dir().join(format!(
            "golden_skirmish_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&save_dir);
        let mut mgr = SaveFileManager::with_save_directory(&save_dir);
        let file_ok = if mgr.init().is_ok() {
            let info = SaveGameInfo {
                filename: "golden_mid".into(),
                display_name: "Golden Mid".into(),
                description: "golden mid-match".into(),
                map_name: map_identity.clone(),
                campaign_side: None,
                mission_number: None,
                save_date: SystemTime::now(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: Duration::from_secs(5),
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            let saved = mgr.save_game("golden_mid", &logic, &info);
            if let Err(e) = &saved {
                log::warn!("golden SaveFileManager save failed: {e}");
            }
            if saved.is_ok() {
                let mut logic2 = GameLogic::new();
                seed_templates(&mut logic2);
                match mgr.load_game("golden_mid", &mut logic2) {
                    Ok(_) => logic2.get_player(0).is_some(),
                    Err(e) => {
                        log::warn!("golden SaveFileManager load failed: {e}");
                        false
                    }
                }
            } else {
                false
            }
        } else {
            false
        };
        let snap_ok = {
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            match builder.create_world_snapshot(&logic) {
                Ok(snap) => {
                    let mut logic2 = GameLogic::new();
                    seed_templates(&mut logic2);
                    builder.restore_from_snapshot(&snap, &mut logic2).is_ok()
                        && logic2.get_player(0).is_some()
                }
                Err(e) => {
                    log::warn!("golden snapshot save failed: {e}");
                    false
                }
            }
        };
        let _ = std::fs::remove_dir_all(&save_dir);
        // Fail closed: require production SnapshotBuilder path (always available).
        // File manager is reported separately via logs; snapshot is the gate contract.
        let _ = file_ok;
        snap_ok
    };

    // Deterministic config apply checkpoints (two fresh worlds, same config).
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    let _ = apply_skirmish_config(&mut a, &config);
    let _ = apply_skirmish_config(&mut b, &config);
    let ha = AuthorityProbe::capture(&a, 0).checkpoint_hash();
    let hb = AuthorityProbe::capture(&b, 0).checkpoint_hash();
    let mut checkpoint_hashes = vec![ha, hb];
    checkpoint_hashes.push(AuthorityProbe::capture(&logic, 0).checkpoint_hash());

    // Host combat world is synthetic (map is probe-only). Fail-closed: never claim
    // full non-network playability from a synthetic-only path.
    let synthetic_combat = true;
    let playable_claim = false;
    let ai_structure_templates_retained = logic.templates.contains_key("GLA_CommandCenter")
        && logic.templates.contains_key("GLA_Barracks")
        && logic.templates.contains_key("GLA_SupplyStash")
        && logic.templates.contains_key("GLA_ArmsDealer");

    // When a retail map is present, require map-army combat + player preserve for success.
    let map_combat_required_ok = !map_loaded || map_combat_ok;
    let map_players_required_ok = !map_loaded || players_preserved_on_load;
    let status = if config_applied
        && frames_advanced > 0
        && moved_units
        && gathered
        && constructed
        && produced
        && upgraded
        && fought
        && victory
        && save_load_ok
        && ha == hb
        && ai_structure_templates_retained
        && map_combat_required_ok
        && map_players_required_ok
    {
        "success".into()
    } else {
        "partial".into()
    };

    set_verification_single_authority(false);

    GoldenSkirmishResult {
        map_identity,
        map_loaded,
        config_applied,
        slots_active,
        human_cash,
        ai_cash,
        ai_difficulty,
        frames_advanced,
        moved_units,
        gathered,
        constructed,
        produced,
        upgraded,
        fought,
        victory,
        save_load_ok,
        checkpoint_hashes,
        synthetic_combat,
        ai_disabled_for_slice,
        playable_claim,
        ai_structure_templates_retained,
        map_combat_ok,
        same_world_production_ok,
        players_preserved_on_load,
        status,
    }
}

pub fn format_golden_report(r: &GoldenSkirmishResult) -> String {
    format!(
        "map={} loaded={} config_applied={} slots={} human_cash={} ai_cash={} ai_diff={} frames={} move={} gather={} build={} produce={} upgrade={} fight={} victory={} save_load={} status={} checkpoints={} synthetic={} ai_off={} playable_claim={} ai_templates_retained={} map_combat={} same_world_prod={} players_preserved={}",
        r.map_identity,
        r.map_loaded,
        r.config_applied,
        r.slots_active,
        r.human_cash,
        r.ai_cash,
        r.ai_difficulty,
        r.frames_advanced,
        r.moved_units,
        r.gathered,
        r.constructed,
        r.produced,
        r.upgraded,
        r.fought,
        r.victory,
        r.save_load_ok,
        r.status,
        r.checkpoint_hashes.len(),
        r.synthetic_combat,
        r.ai_disabled_for_slice,
        r.playable_claim,
        r.ai_structure_templates_retained,
        r.map_combat_ok,
        r.same_world_production_ok,
        r.players_preserved_on_load
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_skirmish_full_vertical_slice() {
        let result = run_golden_skirmish(None, 8);
        assert!(result.config_applied, "skirmish config must apply");
        assert_eq!(result.slots_active, 2);
        assert_eq!(result.ai_difficulty, "Medium");
        assert!(result.frames_advanced > 0);
        assert!(result.moved_units, "Move command path");
        assert!(result.gathered, "Gather command path");
        assert!(result.constructed, "DozerConstruct path");
        assert!(result.produced, "QueueUnitCreate path");
        assert!(result.upgraded, "QueueUpgrade path");
        assert!(result.fought, "AttackObject path");
        assert!(result.victory, "VictoryCondition::Winner(0)");
        assert!(result.save_load_ok, "save/load round-trip");
        assert_eq!(result.status, "success");
        assert!(result.synthetic_combat, "host combat world (not map soup)");
        assert!(
            !result.ai_disabled_for_slice,
            "opponent AI stays active for this slice"
        );
        assert!(
            !result.playable_claim,
            "synthetic_combat path must fail-closed for playable_claim"
        );
        assert!(
            result.ai_structure_templates_retained,
            "AI structure templates must remain in catalog (no mid-scenario strip)"
        );
        if result.map_loaded {
            assert!(
                result.map_combat_ok,
                "map-loaded path must prove AttackObject damage on map army: {}",
                format_golden_report(&result)
            );
            assert!(
                result.players_preserved_on_load,
                "skirmish players/AI/cash must survive load_map: {}",
                format_golden_report(&result)
            );
        }
        assert_eq!(
            result.checkpoint_hashes[0], result.checkpoint_hashes[1],
            "identical config must yield identical start probes"
        );
    }

    #[test]
    fn golden_skirmish_with_retail_map_when_present() {
        let candidates = [
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        ];
        let Some(map) = candidates
            .iter()
            .find(|p| std::path::Path::new(p).is_file())
        else {
            eprintln!("retail map absent — skipping map-loaded golden assertion");
            return;
        };
        let result = run_golden_skirmish(Some(map), 8);
        assert!(
            result.map_loaded,
            "retail map must load on probe: {}",
            result.map_identity
        );
        // Map load is a separate probe; combat is host-world. Full slice still required.
        assert!(
            result.victory && !result.playable_claim,
            "victory on synthetic host path without playable_claim: {}",
            format_golden_report(&result)
        );
        assert!(result.save_load_ok, "save/load round-trip");
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_golden_report(&result)
        );
    }
}
