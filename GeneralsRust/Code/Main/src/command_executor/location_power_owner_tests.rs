//! Actual command gates read the driving map services, even under foreign Core scope.
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, PowerTarget, SpecialPowerType};
use crate::game_logic::{
    GameLogic, KindOf, ObjectId, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
    ThingTemplate,
};
use gamelogic::common::{AsciiString, ICoord2D, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;
use gamelogic::system::map_loader::MapData;
use gamelogic::system::shroud_manager::ShroudState;
use glam::Vec3;

fn location_world(water: bool, extent: i32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    world.add_player(Player::new(1, Team::China, "China", false));
    let mut caster = ThingTemplate::new("HqLocationOwnerCaster");
    caster.set_health(5000.0);
    for (power, template) in [
        (
            SpecialPowerType::Airstrike,
            "SuperweaponA10ThunderboltMissileStrike",
        ),
        (SpecialPowerType::Paradrop, "SuperweaponParadropAmerica"),
        (SpecialPowerType::SpySatellite, "SpecialPowerSpySatellite"),
    ] {
        caster
            .special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: Some(format!("ModuleTag_{template}")),
                module_kind: SpecialPowerModuleKind::OclSpecialPower,
                special_power_template: template.into(),
                special_power_template_id: 1,
                command_power: Some(power),
                reload_time_frames: 300,
                required_science: None,
                public_timer: false,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
                cash_bounty: None,
            });
    }
    world
        .templates
        .insert("HqLocationOwnerCaster".into(), caster);
    let mut target = ThingTemplate::new("HqLocationOwnerTarget");
    target.set_health(200.0);
    target.add_kind_of(KindOf::Vehicle);
    world
        .templates
        .insert("HqLocationOwnerTarget".into(), target);
    let caster = world
        .create_object_for_player("HqLocationOwnerCaster", 0, Vec3::ZERO)
        .unwrap();
    let target = world
        .create_object_for_player("HqLocationOwnerTarget", 1, Vec3::new(80.0, 0.0, 40.0))
        .unwrap();
    for power in [
        SpecialPowerType::Airstrike,
        SpecialPowerType::Paradrop,
        SpecialPowerType::SpySatellite,
    ] {
        world
            .host_object_mut(caster)
            .unwrap()
            .set_special_power_ready_seconds(&power, 0.0);
    }
    let mut map = MapData::new();
    map.width = 32;
    map.height = 32;
    map.border_size = 1;
    map.heightmap = vec![0; 34 * 34];
    map.boundaries = vec![ICoord2D::new(extent, extent)];
    if water {
        let mut lake = PolygonTrigger::new(3, AsciiString::from("HqLocationOwnerLake"), Vec::new());
        lake.set_water_area(true);
        for (x, y) in [(0, 0), (200, 0), (200, 200), (0, 200)] {
            lake.add_point(ICoord3D::new(x, y, 12));
        }
        map.polygon_triggers.push(lake);
    }
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_geometry(map);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(512.0, 512.0);
    (world, caster, target)
}

fn with_foreign_held(run: impl FnOnce()) {
    let foreign = gamelogic::system::engine_stores::new_for_world();
    let terrain = foreign.services().terrain().write().unwrap();
    let shroud = foreign.shroud().lock().unwrap();
    let ai = foreign.ai().write().unwrap();
    gamelogic::system::engine_stores::with_active_stores(&foreign, run);
    drop((terrain, shroud, ai));
}

fn click(
    world: &mut GameLogic,
    caster: ObjectId,
    target: ObjectId,
    power: SpecialPowerType,
) -> CommandResult {
    CommandExecutor::new(world, 0)
        .execute_special_power(
            &[caster],
            &power,
            &PowerTarget::Object(target),
        )
        .expect("special power")
}

#[test]
fn admitted_command_center_power_does_not_create_an_ai_machine() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    crate::fixture_content::admit_fixture_content_with_actions(
        &mut world,
        &[crate::fixture_content::FixtureObject::new("AmericaCommandCenter")
            .kind_of("STRUCTURE SELECTABLE")
            .health(2000.0)
            .module("Behavior = OCLSpecialPower ModuleTag_Spy\n SpecialPowerTemplate = SpecialPowerSpySatellite\nEnd")],
        &[],
        "SpecialPower SpecialPowerSpySatellite\n Enum = SPECIAL_SPY_SATELLITE\n ReloadTime = 60000\nEnd\n",
    );
    let mut map = MapData::new();
    map.width = 32;
    map.height = 32;
    map.border_size = 1;
    map.heightmap = vec![0; 34 * 34];
    map.boundaries = vec![ICoord2D::new(320, 320)];
    world.world_services.terrain().write().unwrap().load_map_geometry(map);
    world.world_services.shroud().lock().unwrap().init_shroud_grid(512.0, 512.0);
    let center = world.create_object_for_player("AmericaCommandCenter", 0, Vec3::ZERO).unwrap();
    let object = world.host_object_mut(center).unwrap();
    assert!(!object.has_ai_update_interface());
    object.set_special_power_ready_seconds(&SpecialPowerType::SpySatellite, 0.0);
    assert_eq!(
        CommandExecutor::new(&mut world, 0).execute_special_power(
            &[center],
            &SpecialPowerType::SpySatellite,
            &PowerTarget::Location(Vec3::new(80.0, 0.0, 80.0)),
        ).unwrap(),
        CommandResult::Success
    );
    assert!(!world.is_special_power_ready_for(center, &SpecialPowerType::SpySatellite));
    let object = world.host_object(center).unwrap();
    assert_eq!(object.ai_state(), crate::game_logic::AIState::Idle);
    crate::save_load::snapshot::base_ai_machine_persist::BaseAiObjectSnapshot::capture(object)
        .expect("source-less building power must remain saveable");
}

#[test]
fn location_command_distinguishes_owner_hidden_fogged_and_visible() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "location_command_distinguishes_owner_hidden_fogged_and_visible",
        || {
            let (mut hidden, caster, target) = location_world(false, 32);
            let (mut fogged, other_caster, other_target) = location_world(false, 32);
            let (mut visible, visible_caster, visible_target) = location_world(false, 32);
            assert_eq!((caster, target), (other_caster, other_target));
            assert_eq!((caster, target), (visible_caster, visible_target));
            fogged
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .reveal_map_for_player(0)
                .unwrap();
            visible
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .do_shroud_reveal(&gamelogic::common::Coord3D::new(80.0, 40.0, 0.0), 100.0, 1);
            let position = gamelogic::common::Coord3D::new(80.0, 40.0, 0.0);
            assert_eq!(
                hidden
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_shroud_state(0, &position),
                ShroudState::Hidden
            );
            assert_eq!(
                fogged
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_shroud_state(0, &position),
                ShroudState::Explored
            );
            assert_eq!(
                visible
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_shroud_state(0, &position),
                ShroudState::Visible
            );
            with_foreign_held(|| {
                assert!(
                    click(&mut hidden, caster, target, SpecialPowerType::Airstrike) == CommandResult::InvalidLocation);
                assert!(hidden.is_special_power_ready_for(caster, &SpecialPowerType::Airstrike));
                assert_eq!(click(
                        &mut fogged,
                        other_caster,
                        other_target,
                        SpecialPowerType::Airstrike
                    ), CommandResult::Success, "CPP allows fogged damaging-power locations");
                assert!(
                    !fogged.is_special_power_ready_for(other_caster, &SpecialPowerType::Airstrike)
                );
                assert!(click(
                        &mut visible,
                        visible_caster,
                        visible_target,
                        SpecialPowerType::Airstrike
                    ) == CommandResult::Success);
                assert_eq!(click(&mut hidden, caster, target, SpecialPowerType::Airstrike), CommandResult::InvalidLocation, "other owners cannot reveal this map");
                assert!(hidden.is_special_power_ready_for(caster, &SpecialPowerType::Airstrike));
            });
        },
    );
}

#[test]
fn location_paradrop_reads_driving_water_before_shroud() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "location_paradrop_reads_driving_water_before_shroud",
        || {
            let (mut wet, caster, target) = location_world(true, 32);
            let (mut dry, other_caster, other_target) = location_world(false, 32);
            assert_eq!((caster, target), (other_caster, other_target));
            for world in [&wet, &dry] {
                world
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .reveal_map_for_player(0)
                    .unwrap();
            }
            with_foreign_held(|| {
                assert!(
                    click(&mut wet, caster, target, SpecialPowerType::Paradrop) == CommandResult::InvalidLocation);
                assert!(wet.is_special_power_ready_for(caster, &SpecialPowerType::Paradrop));
                assert!(click(
                        &mut dry,
                        other_caster,
                        other_target,
                        SpecialPowerType::Paradrop
                    ) == CommandResult::Success);
                assert!(!dry.is_special_power_ready_for(other_caster, &SpecialPowerType::Paradrop));
                assert!(click(&mut wet, caster, target, SpecialPowerType::Paradrop) == CommandResult::InvalidLocation);
                assert!(wet.is_special_power_ready_for(caster, &SpecialPowerType::Paradrop));
            });
        },
    );
}

#[test]
fn informational_location_gate_uses_driving_extent_and_inclusive_edges() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "informational_location_gate_uses_driving_extent_and_inclusive_edges",
        || {
            let (mut small, caster, target) = location_world(false, 4);
            let (large, other_caster, other_target) = location_world(false, 32);
            assert_eq!((caster, target), (other_caster, other_target));
            with_foreign_held(|| {
                assert!(
                    click(&mut small, caster, target, SpecialPowerType::SpySatellite) == CommandResult::InvalidLocation);
                assert!(small.is_special_power_ready_for(caster, &SpecialPowerType::SpySatellite));
                for position in [Vec3::ZERO, Vec3::new(40.0, 99.0, 40.0)] {
                    assert!(
                        super::super::special_power::owned_can_do_special_power_at_location(
                            &small,
                            &SpecialPowerType::SpySatellite,
                            position,
                            0
                        )
                    );
                }
                let outside = Vec3::new(80.0, 0.0, 40.0);
                assert!(
                    !super::super::special_power::owned_can_do_special_power_at_location(
                        &small,
                        &SpecialPowerType::SpySatellite,
                        outside,
                        0
                    )
                );
                assert!(
                    super::super::special_power::owned_can_do_special_power_at_location(
                        &large,
                        &SpecialPowerType::SpySatellite,
                        outside,
                        0
                    )
                );
            });
        },
    );
}
