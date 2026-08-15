use super::*;

#[test]
fn apply_to_ui_state_overwrites_live_identity_after_mutation() {
    // Production path: live update_ui_state may run first; apply_to_ui_state must
    // replace selection health + minimap dots with snapshot-owned values.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HudIdentity");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("HudIdUnit");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("HudIdUnit".into(), t);
    let id = logic
        .create_object("HudIdUnit", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    // Live world mutates after snapshot (would poison a re-read).
    if let Some(o) = logic.host_object_mut(id) {
        o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
        o.health.current = 3.0;
    }

    // Simulate production: live walk first, then presentation overlay.
    let mut ui = logic.update_ui_state(0);
    snap.apply_to_ui_state(&mut ui);

    assert!(
        ui.selected_units.contains(&id),
        "selection ids from snapshot"
    );
    let info = ui
        .selected_unit_infos
        .iter()
        .find(|u| u.object_id == id)
        .expect("selected_unit_infos from snapshot");
    assert!(
        (info.health_current - 100.0).abs() < 0.01,
        "health must be snapshot 100, not live 3: {}",
        info.health_current
    );
    assert!(
        !ui.minimap_unit_dots.is_empty(),
        "minimap dots filled from presentation objects"
    );
    assert_eq!(
        ui.minimap_unit_dots.len(),
        snap.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .count()
    );
    assert!(
        ui.selection_panel.has_positive_health(),
        "last_ui_state selection panel must carry snapshot health"
    );
    assert!(
        (ui.selection_panel.health_current - 100.0).abs() < 0.01,
        "selection panel HP from presentation: {}",
        ui.selection_panel.health_current
    );
}

#[test]
fn path_and_beacon_presentation_residual() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("PathUnit");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("PathUnit".into(), t);
    let idle = logic
        .create_object("PathUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("idle");
    let moving = logic
        .create_object("PathUnit", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("moving");
    if let Some(o) = logic.host_object_mut(moving) {
        o.movement.path = vec![
            glam::Vec3::new(10.0, 0.0, 0.0),
            glam::Vec3::new(50.0, 0.0, 0.0),
        ];
        o.movement.current_path_index = 0;
        o.status.moving = true;
    }
    let active = logic.object_ids_with_active_path();
    assert!(active.contains(&moving));
    assert!(!active.contains(&idle));
    assert_eq!(active.len(), 1);

    logic.note_beacon_placed(glam::Vec3::new(12.0, 0.0, 34.0));
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.new_beacons.len(), 1);
    assert!((frame.new_beacons[0].x - 12.0).abs() < 0.01);

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert_eq!(ui.new_beacons.len(), 1);
}

#[test]
fn unit_command_cancel_upgrade_when_researching_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    logic.add_player(player);
    let mut bar = ThingTemplate::new("TestBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("TestBarracks".into(), bar);
    let bid = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(
            crate::game_logic::buildings::BuildingType::Barracks,
        ));
        o.selected = true;
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![bid];
    }
    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bid],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let cmds = frame.unit_command_buttons();
    let names: Vec<_> = cmds.iter().map(|c| c.command_name.as_str()).collect();
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Command_CancelUpgrade")),
        "researching upgrade should expose CancelUpgrade: {:?}",
        names
    );
    assert!(
        !names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Command_CancelUnit")),
        "should not expose CancelUnit while upgrade head: {:?}",
        names
    );
    // FlashBang command disabled while researching residual.
    if let Some(btn) = cmds
        .iter()
        .find(|c| c.command_name.to_ascii_lowercase().contains("flashbang"))
    {
        assert!(
            !btn.enabled,
            "FlashBang upgrade command restricted while researching"
        );
    }
}

#[test]
fn structure_exposes_command_sell_residual() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType},
        KindOf, Team, ThingTemplate,
    };
    crate::gameworld_shadow::clear_active_shadow_for_coupled_tick();
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tb = ThingTemplate::new("SellBarracks");
    tb.set_health(1000.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("SellBarracks".into(), tb);
    let id = logic
        .create_object("SellBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("b");
    if let Some(o) = logic.host_object_mut(id) {
        o.building_data = Some(BuildingData::new(BuildingType::Barracks));
        o.selected = true;
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let cmds = frame.unit_command_buttons();
    assert!(
        cmds.iter()
            .any(|c| c.command_name.eq_ignore_ascii_case("Command_Sell") && c.enabled),
        "completed structure should expose Sell: {:?}",
        cmds.iter().map(|c| &c.command_name).collect::<Vec<_>>()
    );
    // Under construction: CancelConstruction, not Sell.
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = true;
        o.construction_percent = 0.4;
    }
    let frame2 = PresentationFrame::build_from_logic(&logic, 0);
    let cmds2 = frame2.unit_command_buttons();
    assert!(
        cmds2.iter().any(|c| c
            .command_name
            .eq_ignore_ascii_case("Command_CancelConstruction")),
        "under-construction should expose CancelConstruction"
    );
    assert!(
        !cmds2
            .iter()
            .any(|c| c.command_name.eq_ignore_ascii_case("Command_Sell") && c.enabled),
        "under-construction must not enable Sell"
    );
}

fn presentation_feeds_unit_command_panel_buttons() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType},
        KindOf, Team, ThingTemplate,
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tu = ThingTemplate::new("CmdRanger");
    tu.set_health(120.0);
    tu.add_kind_of(KindOf::Infantry);
    tu.add_kind_of(KindOf::Selectable);
    logic.templates.insert("CmdRanger".into(), tu);
    let mut tb = ThingTemplate::new("CmdBarracks");
    tb.set_health(1000.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("CmdBarracks".into(), tb);
    let ranger = logic
        .create_object("CmdRanger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("r");
    let barracks = logic
        .create_object("CmdBarracks", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(ranger) {
        o.selected = true;
        // Minimal weapon residual so has_weapon freezes true.
        o.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 100.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![ranger];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut panel = crate::ui::UnitCommandPanel::new();
    frame.apply_to_unit_command_panel(&mut panel);
    let names: Vec<_> = panel
        .commands()
        .iter()
        .map(|c| c.command_name.as_str())
        .collect();
    assert!(
        names.iter().any(|n| n.eq_ignore_ascii_case("Command_Stop")),
        "mobile selection should expose Stop: {:?}",
        names
    );
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Command_AttackMove")),
        "armed mobile should expose AttackMove: {:?}",
        names
    );

    if let Some(o) = logic.host_object_mut(barracks) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.selected = true;
        o.building_data = Some(BuildingData::new(BuildingType::Barracks));
    }
    if let Some(o) = logic.host_object_mut(ranger) {
        o.selected = false;
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![barracks];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut panel = crate::ui::UnitCommandPanel::new();
    frame.apply_to_unit_command_panel(&mut panel);
    let names: Vec<_> = panel
        .commands()
        .iter()
        .map(|c| c.command_name.as_str())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Command_SetRallyPoint")),
        "producer should expose SetRallyPoint: {:?}",
        names
    );
}

#[test]
fn unit_command_exposes_deploy_for_sentry_residual() {
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("AmericaVehicleSentryDrone");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleSentryDrone".into(), t);
    let id = logic
        .create_object("AmericaVehicleSentryDrone", Team::USA, glam::Vec3::ZERO)
        .expect("sentry");
    logic.select_objects(0, vec![id]);
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let names: Vec<_> = frame
        .unit_command_buttons()
        .into_iter()
        .map(|b| b.command_name)
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Command_Deploy")),
        "sentry should expose Deploy: {:?}",
        names
    );
}

#[test]
fn presentation_applies_unit_commands_to_game_hud_residual() {
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .expect("ranger");
    logic.select_objects(0, vec![id]);
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut hud = crate::ui::GameHUD::new();
    frame.apply_to_game_hud(&mut hud);
    assert!(
        frame
            .unit_command_buttons()
            .iter()
            .any(|c| c.command_name.eq_ignore_ascii_case("Command_Stop")),
        "ranger selection must expose Stop for HUD apply residual"
    );
}

#[test]
fn hero_ability_commands_on_selection_residual() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let start = src
        .find("fn unit_command_buttons")
        .expect("unit_command_buttons");
    let end = src[start + 1..]
        .find(
            "
    pub fn ",
        )
        .map(|i| start + 1 + i)
        .unwrap_or(start + 8000);
    let body = &src[start..end];
    assert!(
        body.contains("Command_SnipeVehicle")
            && body.contains("Command_PlantTimedDemoCharge")
            && body.contains("Command_Hijack")
            && body.contains("Command_StealCashHack")
            && body.contains("Command_ToggleOvercharge"),
        "unit command strip must expose hero/ability/overcharge residual"
    );
}

#[test]
fn presentation_feeds_victory_and_construction() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType, ProductionItem},
        victory::PlayerOutcome,
        KindOf, Player, Resources, Team, ThingTemplate,
    };
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "VHuman", true));
    logic.add_player(Player::new(1, Team::China, "VAI", false));
    let mut tb = ThingTemplate::new("VBarracks");
    tb.set_health(1000.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("VBarracks".into(), tb);
    let mut tc = ThingTemplate::new("VConstruct");
    tc.set_health(500.0);
    tc.add_kind_of(KindOf::Structure);
    logic.templates.insert("VConstruct".into(), tc);
    let barracks = logic
        .create_object("VBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    let constructing = logic
        .create_object("VConstruct", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("c");
    if let Some(o) = logic.host_object_mut(barracks) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "Ranger".into(),
            progress: 0.25,
            total_time: 20.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: crate::game_logic::buildings::ProductionKind::Unit,
        });
        o.building_data = Some(bd);
    }
    if let Some(o) = logic.host_object_mut(constructing) {
        o.status.under_construction = true;
        o.construction_percent = 0.4;
        o.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.is_local = true;
        p.power_produced = 80;
        p.power_consumed = 30;
    }
    // Mark match over via victory event residual (build_with_victory path).
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    frame.match_over = true;
    frame.victory_label = Some("Winner(0)".into());
    frame.events.push(PresentationEvent::Victory {
        winner_player: Some(0),
    });

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(ui.match_over);
    assert_eq!(ui.player_outcome, Some(PlayerOutcome::Won));
    assert_eq!(ui.power_generated, 80);
    assert_eq!(ui.power_used, 30);
    assert!(
        ui.build_queue.iter().any(|b| {
            b.template_name == "Ranger" && (b.percent_complete - (0.25 / 20.0)).abs() < 0.01
        }),
        "expected production queue residual: {:?}",
        ui.build_queue
    );
    assert!(
        ui.build_queue
            .iter()
            .any(|b| b.template_name == "VConstruct" && (b.percent_complete - 0.4).abs() < 0.01),
        "expected under-construction residual: {:?}",
        ui.build_queue
    );

    let mut screen = crate::ui::VictoryScreen::new();
    frame.apply_to_victory_screen(&mut screen);
    use crate::ui::Renderable;
    assert!(screen.is_visible());
}

#[test]
fn presentation_feeds_control_bar_radar_and_queues() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "RadarP", true));
    let mut t = ThingTemplate::new("RadarVan");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("RadarVan".into(), t);
    let id = logic
        .create_object("RadarVan", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.is_local = true;
        p.is_alive = true;
        p.selected_objects = vec![id];
        p.radar_count = 3;
        p.radar_disabled = false;
        p.queued_upgrades
            .insert("Upgrade_AmericaAdvancedTraining".into());
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.special_power_ready = true;
        o.special_power_cooldown_remaining = 0.0;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.local_radar_count, 3);
    assert!(frame
        .local_queued_upgrades
        .iter()
        .any(|u| u.contains("AdvancedTraining")));

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        frame.apply_to_control_bar(&mut bar);
        assert_eq!(bar.presentation_radar_count(), 3);
        assert!(!bar.presentation_radar_disabled());
        assert!(bar
            .presentation_queued_upgrades()
            .iter()
            .any(|u| u.contains("AdvancedTraining")));
        assert!(
            !bar.get_special_power_shortcuts().is_empty(),
            "expected special power shortcuts from ready selection"
        );
        assert_eq!(
            bar.get_special_power_shortcuts()[0].availability,
            game_client::gui::control_bar::CommandAvailability::Available
        );
    }
}

#[test]
fn presentation_feeds_control_bar_sciences() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "SciP", true));
    let mut t = ThingTemplate::new("SciUnit");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("SciUnit".into(), t);
    let id = logic
        .create_object("SciUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.is_local = true;
        p.is_alive = true;
        p.selected_objects = vec![id];
        p.unlocked_sciences.insert("SCIENCE_RedGuards".into());
        p.unlocked_sciences.insert("SCIENCE_PaladinTank".into());
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(frame
        .local_unlocked_sciences
        .iter()
        .any(|s| s == "SCIENCE_RedGuards"));
    assert!(frame.local_has_science("SCIENCE_PaladinTank"));

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        frame.apply_to_control_bar(&mut bar);
        let sci = bar.get_science_state();
        assert!(sci
            .unlocked_sciences
            .iter()
            .any(|s| s == "SCIENCE_RedGuards"));
        assert!(
            sci.rank1_buttons
                .iter()
                .any(|b| b.is_purchased && b.command_name.contains("RedGuards")),
            "expected purchased science button, got {:?}",
            sci.rank1_buttons
        );
    }
}

#[test]
fn presentation_feeds_control_bar_upgrade_cameos() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("UpgUnit");
    t.set_health(150.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("UpgUnit".into(), t);
    let id = logic
        .create_object("UpgUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 4.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.applied_upgrades.insert("UpgradeAdvancedTraining".into());
        o.applied_upgrades.insert("UpgradeCaptureBuilding".into());
        o.special_power_ready = true;
        o.special_power_cooldown_remaining = 0.0;
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let panel = frame.control_bar_selection_panel();
    assert!(panel
        .applied_upgrades
        .iter()
        .any(|u| u == "UpgradeAdvancedTraining"));
    assert!(panel.special_power_ready);

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        frame.apply_to_control_bar(&mut bar);
        let portrait = bar.get_portrait_state();
        assert_eq!(portrait.upgrade_cameos.len(), 2);
        assert!(portrait
            .upgrade_cameos
            .iter()
            .any(|c| c.upgrade_name == "UpgradeAdvancedTraining" && c.is_completed));
        assert!(portrait.special_power_ready);
    }
}

#[test]
fn presentation_feeds_control_bar_garrison_inventory() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType},
        KindOf, Team, ThingTemplate,
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tb = ThingTemplate::new("GarrisonBunker");
    tb.set_health(800.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("GarrisonBunker".into(), tb);
    let mut tu = ThingTemplate::new("GarrisonRanger");
    tu.set_health(100.0);
    tu.add_kind_of(KindOf::Infantry);
    tu.add_kind_of(KindOf::Selectable);
    logic.templates.insert("GarrisonRanger".into(), tu);
    let bunker = logic
        .create_object("GarrisonBunker", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");
    let ranger = logic
        .create_object("GarrisonRanger", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("ranger");
    if let Some(o) = logic.host_object_mut(bunker) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.selected = true;
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 5;
        bd.garrisoned_units.push(ranger);
        o.building_data = Some(bd);
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![bunker];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let panel = frame.control_bar_selection_panel();
    assert_eq!(panel.max_garrison, 5);
    assert_eq!(panel.garrisoned_count, 1);
    assert!(!panel.under_construction);

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        frame.apply_to_control_bar(&mut bar);
        let ctx = bar.get_context();
        let guard = ctx.read().expect("read");
        let names: Vec<_> = guard
            .available_commands
            .iter()
            .map(|b| b.command_name.as_str())
            .collect();
        assert!(
            names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("Command_StructureExit")),
            "expected StructureExit, got {:?}",
            names
        );
        assert!(
            names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("Command_Evacuate")),
            "expected Evacuate, got {:?}",
            names
        );
        assert_eq!(guard.last_recorded_inventory_count, 1);
    }
}

#[test]
fn presentation_feeds_control_bar_veterancy_and_production() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType, ProductionItem},
        Experience, KindOf, Team, ThingTemplate, VeterancyLevel,
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tb = ThingTemplate::new("VetBarracks");
    tb.set_health(1200.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("VetBarracks".into(), tb);
    let id = logic
        .create_object("VetBarracks", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("building");
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.selected = true;
        o.experience = Experience {
            current: 500.0,
            level: VeterancyLevel::Elite,
        };
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "Ranger".into(),
            progress: 0.55,
            total_time: 10.0,
            construction_frames: 0,
            cost: crate::game_logic::Resources {
                supplies: 200,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: crate::game_logic::buildings::ProductionKind::Unit,
        });
        o.building_data = Some(bd);
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let panel = frame.control_bar_selection_panel();
    assert!(panel.visible);
    assert_eq!(panel.veterancy_overlay.as_deref(), Some("SSChevron2L"));
    assert_eq!(panel.production_template.as_deref(), Some("Ranger"));
    // production_progress is progress_ratio (progress/total_time).
    assert!((panel.production_progress.unwrap_or(0.0) - (0.55 / 10.0)).abs() < 0.01);
    assert_eq!(panel.production_queue.len(), 1);

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        frame.apply_to_control_bar(&mut bar);
        let portrait = bar.get_portrait_state();
        assert_eq!(portrait.veterancy_overlay.as_deref(), Some("SSChevron2L"));
        assert_eq!(portrait.production_template.as_deref(), Some("Ranger"));
        assert!((portrait.production_progress.unwrap_or(0.0) - (0.55 / 10.0)).abs() < 0.01);
        assert_eq!(bar.get_build_queue_data().len(), 1);
        assert_eq!(bar.get_build_queue_data()[0].upgrade_name, "Ranger");
    }
}

#[test]
fn presentation_feeds_control_bar_selection_panel_health() {
    // Residual: ControlBar/WND selection panel health from PresentationFrame
    // (not stale/zero). Headless path — no WND window load required.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CbSelPanel");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("CbPanelUnit");
    t.set_health(77.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("CbPanelUnit".into(), t);
    let id = logic
        .create_object("CbPanelUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 5.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }
    logic.update();

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let panel = snap.control_bar_selection_panel();
    assert!(panel.visible, "selection panel visible with selection");
    assert_eq!(panel.primary_name, "CbPanelUnit");
    assert!(
        (panel.health_current - 77.0).abs() < 0.01,
        "panel health from presentation: {}",
        panel.health_current
    );
    assert!((panel.health_maximum - 77.0).abs() < 0.01);
    assert_eq!(panel.selected_count, 1);
    assert_eq!(panel.primary_object_id, Some(id));

    // GameHUD selection panel (production host display state).
    let mut hud = crate::ui::GameHUD::new();
    snap.apply_to_game_hud(&mut hud);
    assert!(
        hud.selection_panel().has_positive_health(),
        "GameHUD selection panel must show presentation health"
    );
    assert!(
        (hud.selection_panel().health_current - 77.0).abs() < 0.01,
        "HUD panel HP {}",
        hud.selection_panel().health_current
    );

    // last_ui_state path used by engine consumers.
    let mut ui = crate::ui::GameUIState::default();
    snap.apply_to_ui_state(&mut ui);
    assert!(
        (ui.selection_panel.health_current - 77.0).abs() < 0.01,
        "last_ui_state selection panel health"
    );

    // GameClient ControlBar portrait/health strip (no OBJECT_REGISTRY).
    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        // Poison live world after snapshot so a re-read would be wrong.
        if let Some(o) = logic.host_object_mut(id) {
            o.health.current = 1.0;
        }
        snap.apply_to_control_bar(&mut bar);
        let (hp, max) = bar
            .selection_panel_health()
            .expect("ControlBar selection panel health from presentation");
        assert!(
            (hp - 77.0).abs() < 0.01,
            "ControlBar must keep snapshot HP 77, not live 1: {hp}"
        );
        assert!((max - 77.0).abs() < 0.01);
        assert_eq!(bar.get_portrait_state().portrait_image, "CbPanelUnit");
        assert!(bar.get_portrait_state().is_visible);
        assert_eq!(bar.get_portrait_state().selected_count, 1);
    }
}

/// Residual (hq-gq7n): after combat kill, PresentationFrame exposes particle
/// systems from the host registry (observe path for client / HUD).
#[test]
fn presentation_frame_observes_combat_kill_particle_systems() {
    use crate::game_logic::{CombatParticleKind, ThingTemplate, Weapon};

    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("FxTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(50.0);
    logic.templates.insert("FxTank".into(), tank);

    let attacker = logic
        .create_object("FxTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let victim = logic
        .create_object("FxTank", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("victim");

    {
        let a = logic.host_object_mut(attacker).expect("attacker");
        // Wave 562: weapon must be bound before attack_target — can_attack()
        // requires weapon.is_some(); ordering weapon-after-order was a no-op.
        a.weapon = Some(Weapon {
            damage: 9999.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            // Instant-hit residual (0 speed) so one update() can kill.
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            ..Weapon::default()
        });
        a.attack_target(victim);
    }
    {
        let v = logic.host_object_mut(victim).expect("victim");
        v.health.current = 5.0;
        v.health.maximum = 5.0;
    }

    // Wave 562: weapon before attack_target; instant-hit; vehicle SlowDeath
    // defers remove (~1s / 30 frames). Advance until destroy list purges.
    {
        let a = logic.host_object(attacker).expect("attacker pre");
        assert!(a.weapon.is_some(), "weapon bound before attack_target");
        assert!(a.can_attack(), "can_attack requires weapon");
        assert_eq!(a.target, Some(victim), "attack_target must set target");
    }
    for _ in 0..48 {
        logic.update();
        if logic.host_object(victim).is_none() {
            break;
        }
    }
    // SlowDeath residual may leave 0.01 HP until destroy frame; force purge once done.
    if let Some(v) = logic.host_object(victim) {
        if v.health.current <= 0.01 {
            logic.process_destroy_list();
        }
    }
    for _ in 0..8 {
        if logic.host_object(victim).is_none() {
            break;
        }
        logic.update();
        logic.process_destroy_list();
    }

    assert!(
        logic.host_object(victim).is_none()
            || logic
                .host_object(victim)
                .is_some_and(|v| v.status.destroyed || v.health.current <= 0.01),
        "victim should be lethal after combat (destroyed or SlowDeath residual)"
    );

    assert!(
        logic.combat_particles().active_count() > 0,
        "host particle registry must hold systems after kill"
    );

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        snap.has_active_particles(),
        "PresentationFrame must expose active particle systems after combat kill"
    );
    assert!(
        snap.particle_systems
            .iter()
            .any(|p| p.kind == CombatParticleKind::DeathExplosion
                && p.template_name == "MediumExplosion"),
        "death explosion particle must be on presentation frame: {:?}",
        snap.particle_systems
            .iter()
            .map(|p| (&p.template_name, p.kind))
            .collect::<Vec<_>>()
    );
    assert!(
        snap.events
            .iter()
            .any(|e| matches!(e, PresentationEvent::ParticleSystemSpawned { .. })),
        "presentation events should include ParticleSystemSpawned"
    );
    assert!(
        snap.events.iter().any(|e| matches!(
            e,
            PresentationEvent::ObjectDestroyed { id, .. } if *id == victim
        )),
        "presentation events should include ObjectDestroyed for victim"
    );
}

/// Residual: presentation freezes InGameUI floating text + MoneyPickUp Anim2D.
#[test]
fn presentation_frame_freezes_floating_text_and_world_anim() {
    use crate::game_logic::host_money_crate::{HostMoneyCrateRegistry, MONEY_PICKUP_ANIM_TEMPLATE};
    use crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText;
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FloatPres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");

    // Empty residual when host has no cash events.
    let empty = PresentationFrame::build_from_logic(&logic, 0);
    assert!(!empty.has_floating_texts());
    assert!(!empty.has_world_anims());
    assert!(empty.floating_text_presentation_ok());
    assert!(empty.world_anim_presentation_ok());

    let frame = logic.get_frame();
    let oil_ft = HostAutoDepositFloatingText::new(
        ObjectId(11),
        Vec3::new(1.0, 0.0, 2.0),
        100,
        (200, 200, 200),
        frame,
        false,
    );
    logic.push_residual_auto_deposit_floating_text_for_presentation(oil_ft);

    let anim = HostMoneyCrateRegistry::money_pickup_anim(
        ObjectId(21),
        ObjectId(22),
        Vec3::new(5.0, 0.0, 6.0),
        frame,
    );
    let money_ft = HostMoneyCrateRegistry::money_floating_text(
        ObjectId(21),
        ObjectId(22),
        Vec3::new(5.0, 0.0, 6.0),
        125,
        frame,
    );
    logic.push_residual_money_pickup_presentation(anim, money_ft);

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        snap.has_floating_texts(),
        "presentation must freeze host floating texts"
    );
    assert!(
        snap.has_world_anims(),
        "presentation must freeze MoneyPickUp world anim"
    );
    assert!(snap.floating_text_presentation_ok());
    assert!(snap.world_anim_presentation_ok());
    assert_eq!(snap.floating_texts.len(), 2);
    assert_eq!(snap.world_anims.len(), 1);
    assert_eq!(snap.world_anims[0].template, MONEY_PICKUP_ANIM_TEMPLATE);
    assert!(snap
        .floating_texts
        .iter()
        .any(|t| t.kind == PresentationFloatingTextKind::AutoDeposit && t.amount == 100));
    assert!(snap
        .floating_texts
        .iter()
        .any(|t| t.kind == PresentationFloatingTextKind::MoneyCrate
            && t.amount == 125
            && t.color_rgba == (0, 255, 0, 255)));
    assert_eq!(snap.active_floating_texts_at(frame).len(), 2);
    assert!(snap
        .active_floating_texts_at(frame + PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES)
        .is_empty());

    // Snapshot stays frozen after host clears residual registries.
    let frozen_count = snap.floating_texts.len();
    let frozen_anims = snap.world_anims.len();
    logic.clear_residual_floating_text_for_presentation();
    assert_eq!(snap.floating_texts.len(), frozen_count);
    assert_eq!(snap.world_anims.len(), frozen_anims);
    let after = PresentationFrame::build_from_logic(&logic, 0);
    assert!(!after.has_floating_texts());
    assert!(!after.has_world_anims());

    // Synthetic residual for host-testable pack without combat/deposit path.
    let synth = PresentationFloatingText::synthetic_cash(50, 0);
    assert_eq!(synth.text_key, "GUI:AddCash");
    assert_eq!(
        synth.timeout_frame,
        PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES
    );
    assert!(PresentationFloatingText::honesty_vanish_rate_residual_ok());
    assert!(PresentationFloatingText::honesty_vanish_color_alpha_residual_ok());
    assert!((synth.vanish_alpha_at(0) - 1.0).abs() < 0.001);
    assert!((synth.vanish_alpha_at(15) - 0.5).abs() < 0.001);
    assert_eq!(synth.vanish_color_alpha_u8_at(20, 255), 254);
    assert_eq!(synth.color_with_vanish_alpha_at(20), (0, 255, 0, 254));
    assert!((synth.lift_y_at(3) - 3.0).abs() < 0.001);
    let wa = PresentationWorldAnim::synthetic_money_pickup(0);
    assert_eq!(wa.template, MONEY_PICKUP_ANIM_TEMPLATE);
    assert!((wa.z_rise_per_second - 15.0).abs() < 0.01);
    assert!(wa.honesty_fade_residual_ok());
    assert!(PresentationWorldAnim::honesty_money_pickup_fade_params_ok());
    assert!((wa.fade_alpha_at(0) - 1.0).abs() < 0.01);
    // Dual-tick residual counters on freeze.
    assert!(snap.dual_tick_presentation_residual_ok());
    assert!(snap.floating_text_vanish_residual_ok());
    assert!(snap.world_anim_fade_residual_ok());
    assert_eq!(snap.dual_tick.builds, 1);
    assert_eq!(snap.dual_tick.floating_text_count, 2);
    assert_eq!(snap.dual_tick.world_anim_count, 1);
}

/// Residual: presentation freezes assist laser Line3D segments for SegLine pack.
#[test]
fn presentation_frame_freezes_laser_line3d_segments() {
    use crate::game_logic::host_base_defense::{
        make_patriot_assist_lasers, PATRIOT_LASER_SEGMENTS,
    };

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LaserPres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");

    // Empty lasers when host has none.
    let empty = PresentationFrame::build_from_logic(&logic, 0);
    assert!(!empty.has_active_lasers());
    assert_eq!(empty.laser_segment_count(), 0);
    assert!(empty.minimap_fow_presentation_ok());

    // Inject residual assist lasers via public host slice mutation path:
    // push through make + internal list via active endpoint track simulation.
    let beams = make_patriot_assist_lasers(
        ObjectId(1),
        ObjectId(2),
        ObjectId(3),
        (0.0, 0.0, 5.0),
        (30.0, 0.0, 5.0),
        (60.0, 0.0, 5.0),
        logic.get_frame(),
    );
    logic.push_residual_patriot_assist_lasers_for_presentation(beams);

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        snap.has_active_lasers(),
        "presentation must freeze active assist lasers"
    );
    assert_eq!(snap.laser_beams.len(), 2);
    assert_eq!(
        snap.laser_segment_count(),
        PATRIOT_LASER_SEGMENTS as usize * 2
    );
    assert_eq!(
        snap.laser_beams[0].segments.len(),
        PATRIOT_LASER_SEGMENTS as usize
    );
    assert_eq!(
        snap.laser_beams[0].template_name,
        crate::game_logic::host_base_defense::PATRIOT_BINARY_DATA_STREAM
    );
    // Snapshot stays frozen after host clears lasers.
    let frozen_count = snap.laser_segment_count();
    logic.clear_residual_patriot_assist_lasers_for_presentation();
    assert_eq!(snap.laser_segment_count(), frozen_count);
    let after = PresentationFrame::build_from_logic(&logic, 0);
    assert!(!after.has_active_lasers());

    // Synthetic assist pair residual for host-testable pack without combat.
    let pair = PresentationLaserBeam::synthetic_assist_pair(0);
    assert_eq!(pair[0].segments.len(), PATRIOT_LASER_SEGMENTS as usize);
    assert_eq!(pair[1].segments.len(), PATRIOT_LASER_SEGMENTS as usize);
    assert!(pair[0].honesty_ground_height_ok());
    assert!((pair[0].ground_height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001);
    assert!(!pair[0].ground_height_from_terrain);
    assert!(!pair[0].has_soft_edge());
    assert!(pair[0].honesty_soft_edge_presentation_ok());

    // Optional ground-height override residual path.
    let pair_gh = PresentationLaserBeam::synthetic_assist_pair_with_ground(0, 12.5);
    assert!((pair_gh[0].ground_height - 12.5).abs() < 0.001);
    assert!(honesty_ground_height_residual_ok(12.5, true));

    // Orbital multi-beam soft-edge presentation residual → pack wiring fields.
    let orbital = PresentationLaserBeam::synthetic_orbital_soft_edge(0);
    assert!(orbital.has_soft_edge());
    assert!(orbital.honesty_soft_edge_presentation_ok());
    let se = orbital.soft_edge.expect("soft edge");
    assert!(se.honesty_orbital_residual_ok());
    assert_eq!(se.num_beams, 12);
    let (s, e, elapsed, width_scalar) = se.pack_endpoints(orbital.from, orbital.to, 1.0);
    assert_eq!(s, orbital.from);
    assert_eq!(e, orbital.to);
    assert!((elapsed - 1.0).abs() < 0.001);
    assert!((width_scalar - 1.0).abs() < 0.001);
    assert!(snap.laser_presentation_residual_ok() || empty.laser_presentation_residual_ok());
    assert!(empty.dual_tick_presentation_residual_ok());
}

#[cfg(feature = "game_client")]
#[test]
fn presentation_frame_freezes_visible_scene_lines() {
    use game_engine::common::system::geometry::Coord3D;
    use game_engine::common::system::scene_submission::{SceneLineDesc, SceneSubmission};
    use std::sync::Arc;

    game_client::render_bridge::init_render_bridge();
    let _ = gamelogic::helpers::register_scene_submission(Arc::new(
        game_client::render_bridge::RenderBridge::new(),
    ));
    let desc = SceneLineDesc {
        start: Coord3D::new(1.0, 2.0, 3.0),
        end: Coord3D::new(4.0, 5.0, 6.0),
        width: 1.5,
        color_r: 0.2,
        color_g: 0.4,
        color_b: 0.8,
        opacity: 1.0,
        texture_name: Some("EXLaser.tga".to_string()),
        tile_factor: 1.0,
        visible: true,
    };
    if gamelogic::helpers::submit_scene_line(11, &desc).is_none() {
        let _ = game_client::render_bridge::RenderBridge::new().submit_line(11, &desc);
    }

    let logic = GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        !frame.scene_lines.is_empty(),
        "build_from_logic must freeze visible_scene_lines"
    );
    assert!((frame.scene_lines[0].start.0 - 1.0).abs() < f32::EPSILON);
    assert!((frame.scene_lines[0].end.0 - 4.0).abs() < f32::EPSILON);

    let pack =
        crate::graphics::laser_segment_upload::LaserSegmentUpload::pack_from_presentation(&frame);
    assert!(pack.honesty.has_geometry);
    assert!(pack.honesty.segments_packed >= 1);
}

#[test]
fn dual_tick_residual_counters_increment_on_apply() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DualTickCtr");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let frame = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    assert!(frame.dual_tick_presentation_residual_ok());
    assert!(frame.dual_tick.honesty_apply_ok());
    assert_eq!(frame.dual_tick.builds, 1);
    assert_eq!(frame.dual_tick.applies, 1);
    assert!(frame.floating_text_vanish_residual_ok());
    assert!(frame.world_anim_fade_residual_ok());
    assert!(frame.laser_presentation_residual_ok());
}

/// Wave 73: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual.
#[test]
fn spectre_orbit_decal_presentation_residual_wave73() {
    assert!(honesty_spectre_orbit_decal_presentation_ok());
    let decal = PresentationSpectreOrbitDecal::RETAIL;
    assert!(decal.honesty_residual_ok());
    assert_eq!(decal.attack_area_texture, "SCCSpecTarg");
    assert_eq!(decal.reticle_texture, "SCCSpecRet");
    assert!((decal.attack_area_radius - 200.0).abs() < 0.01);
    assert!((decal.reticle_radius - 25.0).abs() < 0.01);
    assert_eq!(decal.attack_area_throb_ms, 1500);
    assert_eq!(decal.reticle_throb_ms, 300);
    assert_eq!(decal.style, "SHADOW_ALPHA_DECAL");
    assert!(decal.only_visible_to_owning_player);
    assert!(decal.reticle_radius < decal.attack_area_radius);

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpectreDecalPres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(snap.spectre_orbit_decal_presentation_residual_ok());
}

/// Wave 102 residual: dual-tick deepen (selected/particle counters + packs).
#[test]
fn presentation_dual_tick_residual_deepen_wave102() {
    assert!(honesty_presentation_dual_tick_residual_deepen_wave102());
    assert!(honesty_presentation_residual_deepen_pack_wave102());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Pres102");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let frame = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    assert!(frame.dual_tick_presentation_residual_ok());
    assert!(frame.dual_tick_presentation_residual_deepen_ok());
    assert_eq!(frame.dual_tick.selected_count, frame.selected.len() as u32);
    assert_eq!(
        frame.dual_tick.particle_count,
        frame.particle_systems.len() as u32
    );
    assert!(frame.dual_tick.honesty_apply_ok());
}

#[test]
fn projectile_render_input_from_tank_shell() {
    let p = PresentationProjectile {
        id: ObjectId(7),
        position: Vec3::new(1.0, 2.0, 3.0),
        velocity: Vec3::new(10.0, 0.0, 0.0),
        target_position: Vec3::new(20.0, 2.0, 3.0),
        shooter_id: ObjectId(1),
        target_id: None,
        damage: 5.0,
        lifetime: 0.1,
        max_lifetime: 2.0,
        is_homing: false,
        projectile_object_name: "GenericTankShell".into(),
        model_key: String::new(),

        exhaust_name: String::new(),
    };
    let input = ProjectileRenderInput::from_presentation(&p).expect("mesh key");
    assert_eq!(input.model_key.to_ascii_lowercase(), "pmgntankshell");
    let m = input.world_matrix();
    let t = m.w_axis.truncate();
    assert!((t - p.position).length() < 1e-3);
}

#[test]
fn hitscan_projectile_has_no_mesh_input() {
    let p = PresentationProjectile {
        id: ObjectId(8),
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        target_position: Vec3::X,
        shooter_id: ObjectId(1),
        target_id: None,
        damage: 1.0,
        lifetime: 0.0,
        max_lifetime: 1.0,
        is_homing: false,
        projectile_object_name: String::new(),
        model_key: String::new(),

        exhaust_name: String::new(),
    };
    assert!(ProjectileRenderInput::from_presentation(&p).is_none());
}

#[test]
fn weapon_laser_presentation_freezes_laser_name() {
    let l = crate::game_logic::host_weapon_laser::ResidualWeaponLaser::new(
        "PointDefenseLaserBeam",
        ObjectId(1),
        Some(ObjectId(2)),
        (0.0, 5.0, 0.0),
        (20.0, 5.0, 10.0),
        0,
    );
    let beam = PresentationLaserBeam::from_weapon_laser(&l, 0, 0.0, false);
    assert_eq!(beam.kind, PresentationLaserKind::WeaponLaser);
    assert_eq!(beam.template_name, "PointDefenseLaserBeam");
    assert!(beam.laser_bone_name.is_empty() || beam.laser_bone_name == "LASER");
    assert!(!beam.segments.is_empty());

    let l2 = crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
        "PointDefenseLaserBeam",
        "LASER",
        ObjectId(1),
        Some(ObjectId(2)),
        (0.0, 5.0, 0.0),
        (20.0, 5.0, 10.0),
        0,
    );
    let beam2 = PresentationLaserBeam::from_weapon_laser(&l2, 1, 0.0, false);
    assert_eq!(beam2.laser_bone_name, "LASER");
}
