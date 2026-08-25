use super::*;

#[test]
fn runtime_host_presentation_query_helpers() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tu = ThingTemplate::new("Ranger");
    tu.set_health(100.0);
    tu.add_kind_of(KindOf::Infantry);
    tu.add_kind_of(KindOf::Selectable);
    tu.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ranger".into(), tu);
    let mut tb = ThingTemplate::new("WarFactory");
    tb.set_health(1000.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("WarFactory".into(), tb);
    let mut te = ThingTemplate::new("RedGuard");
    te.set_health(100.0);
    te.add_kind_of(KindOf::Infantry);
    te.add_kind_of(KindOf::Attackable);
    te.add_kind_of(KindOf::Selectable);
    logic.templates.insert("RedGuard".into(), te);
    let u = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let p = logic
        .create_object("WarFactory", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let e = logic
        .create_object("RedGuard", Team::China, glam::Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(p) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.first_mobile_friendly_id(Team::USA), Some(u));
    assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(p));
    assert_eq!(frame.first_enemy_attackable_id(Team::USA), Some(e));
    assert_eq!(frame.count_mobile_friendlies(Team::USA), 1);
}

#[test]
fn money_crate_identity_freezes_for_click_routing() {
    // C++ CommandXlat.cpp:116-149 / 1921-1937 — crate click needs salvage vs
    // ordinary crate identity on the live presentation path.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut salvage = ThingTemplate::new("SalvageCrate");
    salvage
        .add_kind_of(KindOf::Crate)
        .add_kind_of(KindOf::Selectable)
        .set_health(1.0);
    logic.templates.insert("SalvageCrate".into(), salvage);
    let mut heal = ThingTemplate::new("HealCrate");
    heal.add_kind_of(KindOf::Crate)
        .add_kind_of(KindOf::Selectable)
        .set_health(1.0);
    logic.templates.insert("HealCrate".into(), heal);

    let salvage_id = logic
        .create_object(
            "SalvageCrate",
            Team::Neutral,
            glam::Vec3::new(5.0, 0.0, 0.0),
        )
        .expect("salvage");
    let heal_id = logic
        .create_object("HealCrate", Team::Neutral, glam::Vec3::new(15.0, 0.0, 0.0))
        .expect("heal");
    logic
        .host_money_crates
        .register_salvage_crate(salvage_id, 40);
    logic.host_money_crates.register_heal_crate(heal_id);

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let salvage_obj = frame
        .objects
        .iter()
        .find(|o| o.id == salvage_id)
        .expect("salvage freeze");
    let heal_obj = frame
        .objects
        .iter()
        .find(|o| o.id == heal_id)
        .expect("heal freeze");
    assert!(salvage_obj.is_crate && salvage_obj.is_salvage_crate);
    assert!(heal_obj.is_crate && !heal_obj.is_salvage_crate);
}

#[test]
fn player_roster_frozen_from_host() {
    let mut logic = GameLogic::new();
    let cfg = crate::skirmish_config::golden_skirmish_config("PlayerRosterFreeze");
    crate::skirmish_config::apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    let host = logic.get_player(pid).expect("p");
    let frame = PresentationFrame::build_from_logic(&logic, pid);
    assert!(
        !frame.players.is_empty(),
        "roster must include skirmish players"
    );
    let info = frame.player_info(pid).expect("roster entry");
    assert_eq!(info.name, host.name);
    assert_eq!(info.team, host.team);
    assert_eq!(frame.player_name(pid), Some(host.name.as_str()));
    assert_eq!(frame.player_team(pid), Some(host.team));
    assert!(frame.player_info(99999).is_none());
}

#[test]
fn local_team_frozen_from_host_player() {
    let mut logic = GameLogic::new();
    let cfg = crate::skirmish_config::golden_skirmish_config("LocalTeamFreeze");
    crate::skirmish_config::apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    let host_team = logic.get_player(pid).expect("p").team;
    let frame = PresentationFrame::build_from_logic(&logic, pid);
    assert_eq!(frame.local_player_id, pid);
    assert_eq!(frame.local_team, host_team);
    assert_eq!(frame.local_team(), host_team);
}

#[test]
fn centroid_of_ids_from_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), t);
    let a = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 6.0))
        .unwrap();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let c = frame.centroid_of_ids(&[a, b]).expect("c");
    assert!((c.x - 5.0).abs() < 0.01);
    assert!((c.z - 3.0).abs() < 0.01);
    assert!(frame.centroid_of_ids(&[]).is_none());
    assert!(frame.centroid_of_ids(&[ObjectId(99999)]).is_none());
}

#[test]
fn first_alive_position_for_template_from_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("HeroJet");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("HeroJet".into(), t);
    let id = logic
        .create_object("HeroJet", Team::USA, glam::Vec3::new(42.0, 5.0, -7.0))
        .unwrap();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let pos = frame
        .first_alive_position_for_template("herojet")
        .expect("pos");
    assert!((pos.x - 42.0).abs() < 0.01);
    assert!((pos.z + 7.0).abs() < 0.01);
    // Move live after snapshot — presentation still returns frozen pose.
    if let Some(o) = logic.host_object_mut(id) {
        o.set_position(glam::Vec3::new(900.0, 0.0, 900.0));
    }
    let pos2 = frame.first_alive_position_for_template("HeroJet").unwrap();
    assert!((pos2.x - 42.0).abs() < 0.01);
    assert!(frame.first_alive_position_for_template("Missing").is_none());
}

#[test]
fn hotkey_selection_helpers_from_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), t);
    let a = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("Ranger", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    // Destroy b on host after snapshot? Filter uses snapshot destroyed flag.
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let all = frame.alive_selectable_friendly_ids(Team::USA);
    assert_eq!(all, {
        let mut v = vec![a, b];
        v.sort_by_key(|id| id.0);
        v
    });
    let filtered = frame.filter_alive_selectable_ids(&[a, b, enemy, ObjectId(99999)], Team::USA);
    assert!(filtered.contains(&a) && filtered.contains(&b));
    assert!(!filtered.contains(&enemy));
    // Mark destroyed in a rebuilt frame.
    if let Some(o) = logic.host_object_mut(b) {
        o.status.destroyed = true;
    }
    let frame2 = PresentationFrame::build_from_logic(&logic, 0);
    let filtered2 = frame2.filter_alive_selectable_ids(&[a, b], Team::USA);
    assert_eq!(filtered2, vec![a]);
}

#[test]
fn control_group_recall_keeps_contained_and_non_local_like_cpp() {
    // C++ Squad::getLiveObjects uses Object::isSelectable (no contained/masked
    // peel). SELECT_TEAM then keeps local owner; ADD_TEAM / lookAt do not.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::unit_control::UnitControlSystem;
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), t);
    let local = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let garrisoned = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    let captured = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(local) {
        o.owner_player_id = Some(0);
    }
    if let Some(o) = logic.host_object_mut(garrisoned) {
        o.set_contained_by(Some(ObjectId(50)));
        o.status.masked = true;
        o.owner_player_id = Some(0);
    }
    if let Some(o) = logic.host_object_mut(captured) {
        o.owner_player_id = Some(7);
        o.team = Team::China;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let stored = [local, garrisoned, captured];
    let click = frame.filter_alive_selectable_ids(&stored, Team::USA);
    assert_eq!(click, vec![local], "click path still peels contained");
    assert!(
        !UnitControlSystem::presentation_is_selectable(
            frame.objects.iter().find(|o| o.id == garrisoned).unwrap()
        ),
        "CanSelectDrawable still rejects contained"
    );
    let select_team = frame.filter_live_squad_ids(&stored, true);
    assert_eq!(
        select_team,
        vec![local, garrisoned],
        "SELECT_TEAM keeps garrisoned local, drops captured"
    );
    let add_team = frame.filter_live_squad_ids(&stored, false);
    assert_eq!(
        add_team,
        vec![local, garrisoned, captured],
        "ADD_TEAM / double-tap keep captured last live member"
    );
    assert_eq!(
        *add_team.last().unwrap(),
        captured,
        "lookAt centers on last getLiveObjects member"
    );
}

#[test]
fn box_select_unit_ids_from_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tu = ThingTemplate::new("Ranger");
    tu.set_health(100.0);
    tu.add_kind_of(KindOf::Infantry);
    tu.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), tu);
    let mut ts = ThingTemplate::new("WarFactory");
    ts.set_health(1000.0);
    ts.add_kind_of(KindOf::Structure);
    ts.add_kind_of(KindOf::Selectable);
    logic.templates.insert("WarFactory".into(), ts);
    let u1 = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let u2 = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .unwrap();
    let s = logic
        .create_object("WarFactory", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .unwrap();
    let enemy = logic
        .create_object("Ranger", Team::China, glam::Vec3::new(1.0, 0.0, 1.0))
        .unwrap();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut ids = frame.box_select_unit_ids(Team::USA, -1.0, 10.0, -1.0, 10.0);
    ids.sort_by_key(|id| id.0);
    let mut expect = vec![u1, u2];
    expect.sort_by_key(|id| id.0);
    assert_eq!(ids, expect);
    assert!(!ids.contains(&s));
    // C++ SelectionXlat.cpp:634 — exactly one locally-owned building in the
    // region is selected. Mass-drag still refuses extra structures.
    let only_s = frame.box_select_unit_ids(Team::USA, 1.5, 2.5, 1.5, 2.5);
    assert_eq!(only_s, vec![s]);
    let only_enemy = frame.box_select_unit_ids(Team::USA, 0.5, 1.5, 0.5, 1.5);
    assert_eq!(only_enemy, vec![enemy]);
}

#[test]
fn screen_box_select_uses_the_camera_pixel_region_not_a_world_xz_aabb() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::{Mat4, Vec2, Vec3};

    let mut logic = crate::game_logic::GameLogic::new();
    let mut unit = ThingTemplate::new("ScreenBoxUnit");
    unit.set_health(100.0);
    unit.add_kind_of(KindOf::Infantry);
    unit.add_kind_of(KindOf::Selectable);
    logic.templates.insert("ScreenBoxUnit".into(), unit);

    let center = logic
        .create_object("ScreenBoxUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("center unit");
    // Under a rotated camera this point falls in the world X/Z AABB between
    // the two drag-ray ground hits, but projects outside the actual pixel
    // rectangle.  The screen-space routine must not select it.
    let outside_screen = logic
        .create_object("ScreenBoxUnit", Team::USA, Vec3::new(36.0, 0.0, 0.0))
        .expect("off-rectangle unit");
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    // Retail W3DView drag selection projects drawable centers; unlike a
    // point-click ray cast, it does not inflate the screen region by geometry
    // or selection radius. Keep this intentionally huge value out of the
    // region to prevent a future convenience radius test from changing the
    // observable C++ marquee behavior.
    frame
        .objects
        .iter_mut()
        .find(|object| object.id == outside_screen)
        .expect("frozen off-rectangle unit")
        .selection_radius = 10_000.0;
    let view = Mat4::look_at_rh(Vec3::new(70.0, 90.0, 110.0), Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 1.0, 2_000.0);

    let selected = frame.box_select_unit_ids_in_screen_rect(
        Team::USA,
        view,
        projection,
        Vec2::new(470.0, 470.0),
        Vec2::new(530.0, 530.0),
        Vec2::splat(1_000.0),
    );
    assert_eq!(selected, vec![center]);
    assert!(!selected.contains(&outside_screen));
}

#[test]
fn unit_render_inputs_keep_distinct_source_draw_modules() {
    use crate::assets::AuthoredDrawModel;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = crate::game_logic::GameLogic::new();
    let mut unit = ThingTemplate::new("MultiDrawPresentationProbe");
    unit.set_health(100.0);
    unit.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("MultiDrawPresentationProbe".into(), unit);
    let id = logic
        .create_object("MultiDrawPresentationProbe", Team::USA, glam::Vec3::ZERO)
        .expect("probe object");

    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    let object = frame
        .objects
        .iter_mut()
        .find(|object| object.id == id)
        .expect("frozen object");
    object.model_key = Some("ProbeBody".to_string());
    object.draw_models = vec![
        AuthoredDrawModel {
            module_index: 0,
            model_key: "ProbeBody".to_string(),
            ..Default::default()
        },
        AuthoredDrawModel {
            module_index: 2,
            model_key: "ProbeDoor".to_string(),
            ..Default::default()
        },
    ];

    let inputs = frame.unit_render_inputs();
    let input = inputs
        .iter()
        .find(|input| input.id == id)
        .expect("unit render input");
    assert_eq!(input.model_key, "ProbeBody");
    assert_eq!(
        input.draw_models,
        vec![
            AuthoredDrawModel {
                module_index: 0,
                model_key: "ProbeBody".to_string(),
                ..Default::default()
            },
            AuthoredDrawModel {
                module_index: 2,
                model_key: "ProbeDoor".to_string(),
                ..Default::default()
            },
        ],
        "snapshot hand-off must preserve module order and independent identity"
    );
}

#[test]
fn alive_selectable_friendly_aircraft_ids_residual() {
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut air = ThingTemplate::new("AmericaJetRaptor");
    air.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("AmericaJetRaptor".into(), air);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let _r = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .expect("ranger");
    let a = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("raptor");
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ids = frame.alive_selectable_friendly_aircraft_ids(Team::USA);
    assert_eq!(ids, vec![a], "only aircraft selectable: {:?}", ids);
}

#[test]
fn similar_unit_ids_from_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    t.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ranger".into(), t);
    let mut tb = ThingTemplate::new("MissileDefender");
    tb.set_health(100.0);
    tb.add_kind_of(KindOf::Infantry);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("MissileDefender".into(), tb);
    let a = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let _c = logic
        .create_object(
            "MissileDefender",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    let d = logic
        .create_object("Ranger", Team::China, glam::Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut ids = frame.similar_unit_ids(a, Team::USA);
    ids.sort_by_key(|id| id.0);
    let mut expect = vec![a, b];
    expect.sort_by_key(|id| id.0);
    assert_eq!(ids, expect);
    assert!(!ids.contains(&d));
    assert!(frame.is_enemy_attackable(d, Team::USA));
    assert!(!frame.is_enemy_attackable(a, Team::USA));
}

#[test]
fn similar_unit_ids_use_equivalent_to_and_skip_contained() {
    // C++ InGameUI.cpp:163-172 — leftover ThingTemplate::isEquivalentTo + !isContained.
    // Prefix-stem (AirF_) is not enough without ObjectReskin / BuildVariations.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut stock = ThingTemplate::new("AmericaInfantryRanger");
    stock.set_health(100.0);
    stock.add_kind_of(KindOf::Infantry);
    stock.add_kind_of(KindOf::Selectable);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), stock);
    let mut airf = ThingTemplate::new("AirF_HostTypeSelectRanger");
    airf.set_health(100.0);
    airf.add_kind_of(KindOf::Infantry);
    airf.add_kind_of(KindOf::Selectable);
    logic
        .templates
        .insert("AirF_HostTypeSelectRanger".into(), airf);
    let a = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let same = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    let stem_only = logic
        .create_object(
            "AirF_HostTypeSelectRanger",
            Team::USA,
            glam::Vec3::new(15.0, 0.0, 0.0),
        )
        .unwrap();
    let contained = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    if let Some(obj) = logic.host_object_mut(contained) {
        obj.set_contained_by(Some(a));
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut ids = frame.similar_unit_ids(a, Team::USA);
    ids.sort_by_key(|id| id.0);
    let mut expect = vec![a, same];
    expect.sort_by_key(|id| id.0);
    assert_eq!(ids, expect);
    assert!(
        !ids.contains(&stem_only),
        "AirF_ stem without leftover isEquivalentTo must not type-select"
    );
    assert!(!ids.contains(&contained));
    assert!(frame.similar_unit_ids(contained, Team::USA).is_empty());
}

#[test]
fn type_select_uses_leftover_is_equivalent_to_not_stem() {
    let src = include_str!("../queries.rs");
    assert!(
        src.contains("splash_templates_equivalent(left, right)")
            && src.contains("fn templates_equivalent_for_type_select"),
        "type-select must use leftover ThingTemplate::isEquivalentTo"
    );
    assert!(
        !src.contains("type_select_template_stem") && !src.contains("TYPE_SELECT_GENERAL_PREFIXES"),
        "type-select must not use a general-prefix stem compare"
    );
}

#[test]
fn similar_unit_ids_skip_off_map() {
    // C++ InGameUI.cpp:170-175 — !object->isOffMap() (playable extent).
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    t.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ranger".into(), t);
    let on_map = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let (wmin, wmax) = logic.world_bounds();
    let off_pos = glam::Vec3::new(wmax.x + 80.0, 40.0, wmax.z + 80.0);
    let off_map = logic.create_object("Ranger", Team::USA, off_pos).unwrap();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        crate::game_logic::host_deliver_payload::is_off_map_residual(
            off_pos, wmin.x, wmin.z, wmax.x, wmax.z
        )
    );
    let ids = frame.similar_unit_ids(on_map, Team::USA);
    assert!(ids.contains(&on_map));
    assert!(
        !ids.contains(&off_map),
        "jets/helis/gunships past playable extent must not type-select"
    );
}

#[test]
fn box_select_firebase_propagates_occupant_to_container() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut fb = ThingTemplate::new("AmericaFireBase");
    fb.set_health(1000.0);
    fb.add_kind_of(KindOf::Structure);
    fb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("AmericaFireBase".into(), fb);
    let mut ranger = ThingTemplate::new("Ranger");
    ranger.set_health(100.0);
    ranger.add_kind_of(KindOf::Infantry);
    ranger.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), ranger);
    let container = logic
        .create_object("AmericaFireBase", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let occupant = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(0.5, 0.0, 0.5))
        .unwrap();
    if let Some(obj) = logic.host_object_mut(occupant) {
        obj.set_contained_by(Some(container));
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut ids = frame.box_select_unit_ids(Team::USA, -1.0, 1.0, -1.0, 1.0);
    ids.sort_by_key(|id| id.0);
    assert_eq!(ids, vec![container]);
    assert!(!ids.contains(&occupant));
}

#[test]
fn select_similar_is_structure_aware_and_alt_selects_across_map() {
    // C++ SelectionXlat.cpp:466,475,498-501 + Object.cpp:3024 isMassSelectable.
    // Double-click matches ThingTemplate, refuses structures, and is screen-only
    // unless ALT (selectMatchingAcrossMap).
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::{Mat4, Vec2, Vec3};

    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut ranger = ThingTemplate::new("Ranger");
    ranger.set_health(100.0);
    ranger.add_kind_of(KindOf::Infantry);
    ranger.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Ranger".into(), ranger);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks.set_health(1000.0);
    barracks.add_kind_of(KindOf::Structure);
    barracks.add_kind_of(KindOf::Selectable);
    logic.templates.insert("AmericaBarracks".into(), barracks);

    let on_screen = logic
        .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("on-screen ranger");
    let off_screen = logic
        .create_object("Ranger", Team::USA, Vec3::new(400.0, 0.0, 400.0))
        .expect("off-screen ranger");
    let barracks_a = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::new(2.0, 0.0, 2.0))
        .expect("barracks a");
    let _barracks_b = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .expect("barracks b");

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let view = Mat4::look_at_rh(Vec3::new(70.0, 90.0, 110.0), Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 1.0, 2_000.0);
    let viewport = Vec2::splat(1_000.0);

    let mut screen = frame.similar_unit_ids_for_double_click(
        on_screen,
        Team::USA,
        false,
        view,
        projection,
        viewport,
    );
    screen.sort_by_key(|id| id.0);
    assert_eq!(
        screen,
        vec![on_screen],
        "plain double-click is on-screen only"
    );

    let mut across_map = frame.similar_unit_ids_for_double_click(
        on_screen,
        Team::USA,
        true,
        view,
        projection,
        viewport,
    );
    across_map.sort_by_key(|id| id.0);
    let mut expect_map = vec![on_screen, off_screen];
    expect_map.sort_by_key(|id| id.0);
    assert_eq!(
        across_map, expect_map,
        "ALT double-click selects the same template across the map"
    );

    assert!(
        frame.similar_unit_ids(barracks_a, Team::USA).is_empty(),
        "structures are not mass-selectable (C++ isMassSelectable)"
    );
    assert!(
        frame
            .similar_unit_ids_for_double_click(
                barracks_a,
                Team::USA,
                true,
                view,
                projection,
                viewport,
            )
            .is_empty(),
        "ALT must not type-select buildings either"
    );
}

#[test]
fn kind_of_freeze_from_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tw = ThingTemplate::new("Dozer");
    tw.set_health(200.0);
    tw.add_kind_of(KindOf::Vehicle);
    tw.add_kind_of(KindOf::Worker);
    tw.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Dozer".into(), tw);
    let mut tr = ThingTemplate::new("SupplyDock");
    tr.set_health(1.0);
    tr.add_kind_of(KindOf::Harvestable);
    tr.add_kind_of(KindOf::Resource);
    logic.templates.insert("SupplyDock".into(), tr);
    let did = logic
        .create_object("Dozer", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("d");
    let rid = logic
        .create_object("SupplyDock", Team::Neutral, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("r");
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let d = frame.objects.iter().find(|o| o.id == did).expect("dozer");
    assert!(PresentationFrame::object_has_kind(d, KindOf::Worker));
    assert!(PresentationFrame::object_has_kind(d, KindOf::Vehicle));
    assert!(PresentationFrame::object_has_kind(d, KindOf::Selectable));
    // declaration-order residual: Vehicle before Worker before Selectable
    assert!(d.kind_of.windows(2).all(|w| {
        use crate::game_logic::KindOf::*;
        let rank = |k: KindOf| match k {
            Structure => 0,
            Infantry => 1,
            Vehicle => 2,
            Aircraft => 3,
            Projectile => 4,
            Resource => 5,
            Selectable => 6,
            Attackable => 7,
            CommandCenter => 8,
            Worker => 9,
            _ => 99,
        };
        rank(w[0]) <= rank(w[1])
    }));
    let r = frame.objects.iter().find(|o| o.id == rid).expect("res");
    assert!(PresentationFrame::object_has_kind(r, KindOf::Harvestable));
    assert_eq!(frame.worker_objects().len(), 1);
    assert_eq!(frame.harvestable_objects().len(), 1);
}

#[test]
fn upgrades_object_type_freeze_from_host() {
    use crate::game_logic::{
        KindOf, Team, ThingTemplate, Weapon,
        host_mines::{HostMineData, HostMineKind},
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("Overlord");
    t.set_health(1200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Overlord".into(), t);
    let id = logic
        .create_object("Overlord", Team::China, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.applied_upgrades.insert("Upgrade_ChinaChainGuns".into());
        obj.applied_upgrades.insert("Upgrade_Nationalism".into());
        obj.secondary_weapon = Some(Weapon {
            damage: 8.0,
            range: 150.0,
            min_range: 0.0,
            reload_time: 0.5,
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            ..Default::default()
        });
        obj.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let o = frame.objects.iter().find(|r| r.id == id).expect("o");
    assert_eq!(o.object_type, PresentationObjectType::Vehicle);
    assert!(PresentationFrame::object_has_upgrade(
        o,
        "Upgrade_ChinaChainGuns"
    ));
    assert!(o.applied_upgrades.contains(&"Upgrade_Nationalism".into()));
    assert!(o.applied_upgrades.windows(2).all(|w| w[0] <= w[1]));
    assert!(o.has_secondary_weapon);
    assert!((o.secondary_weapon_range - 150.0).abs() < 0.01);
    assert!((o.secondary_weapon_damage - 8.0).abs() < 0.01);
    assert!(o.has_mine);
    assert_eq!(frame.upgraded_objects().len(), 1);
    assert_eq!(frame.mine_objects().len(), 1);
}

#[test]
fn special_power_freeze_from_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("ParticleUplink");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("ParticleUplink".into(), t);
    let id = logic
        .create_object("ParticleUplink", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.special_power_ready = false;
        obj.special_power_cooldown = 180.0;
        obj.special_power_cooldown_remaining = 45.0;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let o = frame.objects.iter().find(|r| r.id == id).expect("o");
    assert!(!o.special_power_ready);
    assert!((o.special_power_cooldown - 180.0).abs() < 0.01);
    assert!((o.special_power_cooldown_remaining - 45.0).abs() < 0.01);
    let frac = PresentationFrame::special_power_cooldown_fraction(o);
    assert!((frac - 0.25).abs() < 0.01);
    assert!(frame.special_power_ready_objects().is_empty());
    if let Some(obj) = logic.host_object_mut(id) {
        obj.special_power_ready = true;
        obj.special_power_cooldown_remaining = 0.0;
    }
    let frame2 = PresentationFrame::build_from_logic(&logic, 1);
    let o2 = frame2.objects.iter().find(|r| r.id == id).expect("o2");
    assert!(o2.special_power_ready);
    assert_eq!(frame2.special_power_ready_objects().len(), 1);
    assert_eq!(PresentationFrame::special_power_cooldown_fraction(o2), 0.0);
}

#[test]
fn local_player_freeze_from_host() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    let mut t = ThingTemplate::new("LocalUnit");
    t.set_health(50.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("LocalUnit".into(), t);
    let _uid = logic
        .create_object("LocalUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let pid = 0u32;
    if let Some(p) = logic.get_player_mut(pid) {
        p.is_local = true;
        p.is_alive = true;
        p.resources.supplies = 12345;
        p.power_available = 40;
        p.power_produced = 100;
        p.power_consumed = 55;
        p.radar_count = 2;
        p.radar_disabled = false;
        p.cash_bounty_percent = 0.1;
        p.unlocked_sciences.insert("SCIENCE_RedGuards".into());
        p.unlocked_sciences.insert("SCIENCE_CashBounty1".into());
        p.queued_upgrades
            .insert("Upgrade_AmericaAdvancedTraining".into());
        p.color_rgb = (10, 20, 30);
    }
    let frame = PresentationFrame::build_from_logic(&logic, pid);
    assert_eq!(frame.local_player_id, pid);
    assert_eq!(frame.local_supplies, 12345);
    assert_eq!(frame.local_power, 40);
    assert_eq!(frame.local_power_produced, 100);
    assert_eq!(frame.local_power_consumed, 55);
    assert!(frame.local_is_alive);
    assert_eq!(frame.local_radar_count, 2);
    assert!(!frame.local_radar_disabled);
    assert!(frame.local_radar_active());
    assert!((frame.local_cash_bounty_percent - 0.1).abs() < 0.001);
    assert!(frame.local_has_science("SCIENCE_CashBounty1"));
    assert!(
        frame
            .local_unlocked_sciences
            .contains(&"SCIENCE_RedGuards".into())
    );
    assert!(
        frame
            .local_queued_upgrades
            .contains(&"Upgrade_AmericaAdvancedTraining".into())
    );
    assert_eq!(frame.local_color_rgb, (10, 20, 30));
    let ratio = frame.local_energy_ratio();
    assert!((ratio - (100.0 / 55.0)).abs() < 0.01);
}

#[test]
fn weapon_and_stealth_freeze_from_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("StealthScout");
    t.set_health(60.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("StealthScout".into(), t);
    let mut tb = ThingTemplate::new("Bunker");
    tb.set_health(300.0);
    tb.add_kind_of(KindOf::Structure);
    logic.templates.insert("Bunker".into(), tb);
    let uid = logic
        .create_object("StealthScout", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("u");
    let bid = logic
        .create_object("Bunker", Team::USA, glam::Vec3::new(8.0, 0.0, 0.0))
        .expect("b");
    if let Some(obj) = logic.host_object_mut(uid) {
        obj.weapon = Some(Weapon {
            damage: 12.0,
            range: 150.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: false,
            can_target_ground: true,
            ..Default::default()
        });
        obj.status.stealthed = true;
        obj.status.detected = false;
        obj.status.attacking = true;
        obj.status.moving = false;
        obj.force_attack = true;
        obj.contained_by = Some(bid);
        obj.camo_stealth_look = 5;
        obj.detection_range = 300.0;
        obj.disguise_as_template = Some("ChinaTroopCrawler".into());
        obj.disguise_as_team = Some(Team::China);
        // Disguised clears effectively_stealthed
        obj.status.disguised = true;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let u = frame.objects.iter().find(|o| o.id == uid).expect("u");
    assert!(u.has_weapon);
    assert!((u.weapon_range - 150.0).abs() < 0.01);
    assert!((u.weapon_damage - 12.0).abs() < 0.01);
    assert!(u.stealthed);
    assert!(!u.detected);
    // disguised => not effectively stealthed
    assert!(!u.effectively_stealthed);
    assert!(u.attacking);
    assert!(u.force_attack);
    assert_eq!(u.contained_by, Some(bid));
    assert_eq!(u.camo_stealth_look, 5);
    assert!((u.detection_range - 300.0).abs() < 0.01);
    assert_eq!(u.disguise_as_template.as_deref(), Some("ChinaTroopCrawler"));
    assert_eq!(u.disguise_as_team, Some(Team::China));
    assert!(u.disguised);
    assert!((u.disguise_transition_opacity - 1.0).abs() < 0.01);
    // DISGUISED model condition residual bit 116.
    use crate::game_logic::host_enum_table_residual::MC_BIT_DISGUISED;
    assert_ne!(u.model_condition_bits & (1u128 << MC_BIT_DISGUISED), 0);
    // Disguise team color residual (China) replaces true USA tint.
    assert_eq!(u.team_color, Team::China.get_color());
    assert_eq!(frame.attacking_units().len(), 1);
    assert_eq!(frame.contained_units().len(), 1);
    // pure stealth unit without disguise
    if let Some(obj) = logic.host_object_mut(uid) {
        obj.status.disguised = false;
        obj.disguise_as_template = None;
        obj.disguise_as_team = None;
    }
    let frame2 = PresentationFrame::build_from_logic(&logic, 1);
    let u2 = frame2.objects.iter().find(|o| o.id == uid).expect("u2");
    assert!(u2.effectively_stealthed);
    assert_eq!(frame2.effectively_stealthed_units().len(), 1);
}

#[test]
fn construction_and_veterancy_freeze_from_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, VeterancyLevel};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("VetUnit");
    t.set_health(80.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("VetUnit".into(), t);
    let mut tb = ThingTemplate::new("BuildMe");
    tb.set_health(200.0);
    tb.add_kind_of(KindOf::Structure);
    logic.templates.insert("BuildMe".into(), tb);
    let uid = logic
        .create_object("VetUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("u");
    let bid = logic
        .create_object("BuildMe", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("b");
    if let Some(obj) = logic.host_object_mut(uid) {
        obj.experience.level = VeterancyLevel::Elite;
        obj.experience.current = 420.0;
    }
    if let Some(obj) = logic.host_object_mut(bid) {
        obj.status.under_construction = true;
        obj.construction_percent = 0.55;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let u = frame.objects.iter().find(|o| o.id == uid).expect("u");
    assert_eq!(u.veterancy, PresentationVeterancy::Elite);
    assert!((u.experience_points - 420.0).abs() < 0.01);
    let b = frame.objects.iter().find(|o| o.id == bid).expect("b");
    assert!(b.under_construction);
    assert!((b.construction_percent - 0.55).abs() < 0.01);
    assert_eq!(frame.under_construction_objects().len(), 1);
    assert_eq!(frame.veteran_or_higher_units().len(), 1);
}

#[test]
fn garrison_and_power_freeze_from_host() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("GarrBldg");
    t.set_health(300.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("GarrBldg".into(), t);
    let id = logic
        .create_object("GarrBldg", Team::USA, glam::Vec3::ZERO)
        .expect("b");
    if let Some(obj) = logic.host_object_mut(id) {
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.garrisoned_units = vec![ObjectId(10), ObjectId(11)];
        bd.max_garrison = 5;
        obj.building_data = Some(bd);
        obj.power_provided = 10;
        obj.power_consumed = 3;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.garrisoned_units, vec![ObjectId(10), ObjectId(11)]);
    assert_eq!(ro.max_garrison, 5);
    assert_eq!(ro.power_provided, 10);
    assert_eq!(ro.power_consumed, 3);
    assert_eq!(frame.garrisoned_structures().len(), 1);
    assert_eq!(frame.net_power_from_objects(), 7);
}

#[test]
fn production_upgrade_queue_freezes_is_upgrade_and_ratio_residual() {
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
    // Partial research progress residual (half of residual 1-frame ≈ tiny).
    if let Some(o) = logic.host_object_mut(bid) {
        if let Some(bd) = o.building_data.as_mut() {
            if let Some(item) = bd.production_queue.first_mut() {
                item.progress = item.total_time * 0.5;
            }
        }
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame
        .objects
        .iter()
        .find(|o| o.id == bid)
        .expect("barracks ro");
    assert_eq!(ro.production_queue.len(), 1);
    assert!(ro.production_queue[0].is_upgrade, "upgrade residual");
    assert!(
        ro.production_queue[0]
            .template_name
            .eq_ignore_ascii_case(UPGRADE_AMERICA_FLASHBANG)
    );
    assert!((ro.production_queue[0].progress_ratio - 0.5).abs() < 0.01);
    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(
        ui.build_queue.iter().any(|e| {
            e.template_name
                .eq_ignore_ascii_case(UPGRADE_AMERICA_FLASHBANG)
                && (e.percent_complete - 0.5).abs() < 0.01
        }),
        "build queue strip freezes upgrade ratio"
    );
}

#[test]
fn production_queue_freezes_from_building_data() {
    use crate::game_logic::buildings::{BuildingData, BuildingType, ProductionItem};
    use crate::game_logic::{KindOf, Resources, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = ThingTemplate::new("ProdBldg");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("ProdBldg".into(), t);
    let id = logic
        .create_object("ProdBldg", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(obj) = logic.host_object_mut(id) {
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "Ranger".into(),
            progress: 0.4,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 150,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: crate::game_logic::buildings::ProductionKind::Unit,
        });
        bd.rally_point = Some(glam::Vec3::new(12.0, 0.0, 3.0));
        obj.building_data = Some(bd);
        obj.guard_position = Some(glam::Vec3::new(1.0, 0.0, 1.0));
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.production_queue.len(), 1);
    assert_eq!(ro.production_queue[0].template_name, "Ranger");
    assert!((ro.production_queue[0].progress - 0.4).abs() < 0.01);
    assert_eq!(ro.production_queue[0].cost_supplies, 150);
    assert!(!ro.production_queue[0].is_upgrade);
    assert!((ro.production_queue[0].progress_ratio - 0.04).abs() < 0.001);
    assert_eq!(ro.rally_point, Some(glam::Vec3::new(12.0, 0.0, 3.0)));
    assert_eq!(ro.guard_position, Some(glam::Vec3::new(1.0, 0.0, 1.0)));
    assert_eq!(frame.structures_with_production().len(), 1);
}

#[test]
fn production_queue_syncs_to_game_hud_residual() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("sync_production_queue_from_presentation")
            && src.contains("apply_to_game_hud"),
        "apply_to_game_hud must sync producer queue onto GameHUD"
    );
    let hud = include_str!("../../ui/hud.rs");
    assert!(
        hud.contains("fn sync_production_queue_from_presentation"),
        "GameHUD must own production queue sync residual"
    );
}

#[test]
fn is_deployed_freezes_from_object_status_residual() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("pub is_deployed: bool") && src.contains("is_deployed: obj.status.deployed"),
        "PresentationFrame must freeze OBJECT_STATUS_DEPLOYED residual"
    );
}

#[test]
fn move_destination_freezes_from_host_movement() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = crate::game_logic::ThingTemplate::new("MoveDestU");
    t.set_health(40.0);
    t.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic.templates.insert("MoveDestU".into(), t);
    let id = logic
        .create_object(
            "MoveDestU",
            crate::game_logic::Team::USA,
            glam::Vec3::new(1.0, 0.0, 1.0),
        )
        .expect("u");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.movement.target_position = Some(glam::Vec3::new(9.0, 0.0, 4.0));
        obj.target = Some(crate::game_logic::ObjectId(99));
        obj.movement.path = vec![
            glam::Vec3::new(1.0, 0.0, 1.0),
            glam::Vec3::new(9.0, 0.0, 4.0),
        ];
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.move_destination, Some(glam::Vec3::new(9.0, 0.0, 4.0)));
    assert_eq!(ro.attack_target, Some(crate::game_logic::ObjectId(99)));
    assert_eq!(ro.path_waypoints.len(), 2);
}

#[test]
fn projectiles_freeze_from_combat_system() {
    let mut logic = crate::game_logic::GameLogic::new();
    let weapon = crate::game_logic::Weapon::default();
    let pid = logic.combat_system_mut().fire_projectile(
        glam::Vec3::new(0.0, 0.0, 0.0),
        glam::Vec3::new(100.0, 0.0, 0.0),
        &weapon,
        crate::game_logic::ObjectId(1),
        Some(crate::game_logic::ObjectId(2)),
        200.0,
    );
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.projectiles.iter().any(|p| p.id == pid),
        "expected projectile {pid:?} in {:?}",
        frame.projectiles.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert!(
        frame
            .projectiles
            .iter()
            .any(|p| (p.target_position.x - 100.0).abs() < 0.1),
        "target pos frozen"
    );
}

#[test]
fn combat_damage_does_not_spawn_floating_text() {
    // C++ addFloatingText is cash-only. DamageApplied stays an event, not a floater.
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_damage_log::record(
        crate::game_logic::ObjectId(11),
        25.0,
        Some(crate::game_logic::ObjectId(1)),
        false,
    );
    let _ = crate::game_logic::host_damage_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame
            .floating_texts
            .iter()
            .all(|t| !matches!(t.kind, PresentationFloatingTextKind::CombatDamage)),
        "retail addFloatingText is cash-only, no CombatDamage -N: {:?}",
        frame
            .floating_texts
            .iter()
            .map(|t| (&t.kind, &t.text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn damage_applied_freezes_from_last_drain() {
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_damage_log::record(
        crate::game_logic::ObjectId(8),
        12.5,
        Some(crate::game_logic::ObjectId(1)),
        false,
    );
    let _ = crate::game_logic::host_damage_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed: false,
                    ..
                } if target.0 == 8 && (*amount - 12.5).abs() < 0.01
            )
        }),
        "expected DamageApplied: {:?}",
        frame.events
    );
}

#[test]
fn move_ordered_freezes_from_last_drain() {
    crate::game_logic::host_move_log::clear();
    crate::game_logic::host_move_log::record(
        crate::game_logic::ObjectId(4),
        Some([10.0, 0.0, 20.0]),
    );
    let _ = crate::game_logic::host_move_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::MoveOrdered {
                    unit,
                    destination
                } if unit.0 == 4 && *destination == [10.0, 0.0, 20.0]
            )
        }),
        "expected MoveOrdered: {:?}",
        frame.events
    );
}

#[test]
fn attack_targeted_freezes_from_last_drain() {
    crate::game_logic::host_attack_log::clear();
    crate::game_logic::host_attack_log::record(
        crate::game_logic::ObjectId(2),
        Some(crate::game_logic::ObjectId(5)),
    );
    let _ = crate::game_logic::host_attack_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::AttackTargeted {
                    attacker,
                    target: Some(t)
                } if attacker.0 == 2 && t.0 == 5
            )
        }),
        "expected AttackTargeted: {:?}",
        frame.events
    );
}

#[test]
fn owner_changed_freezes_from_last_drain() {
    crate::game_logic::host_owner_log::clear();
    crate::game_logic::host_owner_log::record(
        crate::game_logic::ObjectId(7),
        crate::game_logic::Team::China,
    );
    let _ = crate::game_logic::host_owner_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::OwnerChanged {
                    id,
                    team: crate::game_logic::Team::China
                } if id.0 == 7
            )
        }),
        "expected OwnerChanged: {:?}",
        frame.events
    );
}

#[test]
fn production_complete_freezes_from_last_drain() {
    crate::game_logic::host_production_log::clear();
    crate::game_logic::host_production_log::record_complete(
        crate::game_logic::ObjectId(1),
        "TestRanger",
        crate::game_logic::ObjectId(9),
    );
    let _ = crate::game_logic::host_production_log::drain(); // simulate shadow session
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::ProductionComplete {
                    producer,
                    template,
                    spawned
                } if producer.0 == 1 && spawned.0 == 9 && template == "TestRanger"
            )
        }),
        "expected ProductionComplete: {:?}",
        frame.events
    );
}

#[test]
fn presentation_feeds_shake_skybox_superweapon() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.queue_pending_screen_shake(2);
    logic.queue_pending_screen_shake(5);
    logic.set_script_skybox_enabled_for_test(true);
    logic.set_script_superweapon_display_enabled_for_test(false);
    logic.set_script_named_timer_display_shown_for_test(true);
    logic.hide_script_superweapon_object_for_test(crate::game_logic::ObjectId(42));

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(frame.screen_shakes.contains(&2));
    assert!(frame.screen_shakes.contains(&5));
    assert!(frame.script_skybox_enabled);
    assert!(!frame.superweapon_display_enabled);
    assert!(frame.named_timer_display_shown);
    assert!(frame.superweapon_hidden_objects.contains(&42));

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(ui.screen_shakes.contains(&5));
    assert!(ui.script_skybox_enabled);
    assert!(!ui.superweapon_display_enabled);
    assert!(ui.named_timer_display_shown);
    assert!(ui.superweapon_hidden_objects.contains(&42));
}

#[test]
fn presentation_feeds_camera_controls() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.queue_pending_camera_zoom(0.55, 1.5);
    logic.queue_pending_camera_zoom_reset();
    logic.queue_pending_camera_pitch(-0.2, 0.8);
    logic.queue_pending_camera_rotate(1.0, 2.0);
    logic.queue_pending_camera_look_toward(glam::Vec3::new(10.0, 0.0, 20.0), 1.0);
    logic.set_pending_camera_look_toward_reverse_rotation(true);
    logic.queue_pending_camera_slave_enable("AmericaSpyDrone", "Bone01");
    logic.queue_pending_camera_slave_disable();
    logic.upsert_script_named_timer("TimerA", "00:30", true);
    logic.set_script_cameo_flash("Command_AmericaRanger", 3);

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.camera_zoom, Some((0.55, 1.5)));
    assert!(frame.camera_zoom_reset);
    assert_eq!(frame.camera_pitch, Some((-0.2, 0.8)));
    assert_eq!(frame.camera_rotate, Some((1.0, 2.0)));
    assert_eq!(frame.camera_look_toward, Some([10.0, 0.0, 20.0]));
    assert!(frame.camera_look_toward_reverse_rotation);
    assert_eq!(
        frame
            .camera_slave_enable
            .as_ref()
            .map(|(t, b)| (t.as_str(), b.as_str())),
        Some(("AmericaSpyDrone", "Bone01"))
    );
    assert!(frame.camera_slave_disable);
    assert!(
        frame
            .named_timers
            .iter()
            .any(|(n, t, c)| n == "TimerA" && t == "00:30" && *c)
    );
    assert!(
        frame
            .cameo_flash
            .iter()
            .any(|(b, c)| b == "Command_AmericaRanger" && *c == 3)
    );

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert_eq!(ui.camera_zoom, Some((0.55, 1.5)));
    assert!(ui.camera_zoom_reset);
    assert!(ui.named_timers.iter().any(|(n, _, _)| n == "TimerA"));
    assert!(
        ui.cameo_flash
            .iter()
            .any(|(b, c)| b.contains("Ranger") && *c == 3)
    );
}

#[test]
fn presentation_feeds_script_camera() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.set_script_time_frozen_for_test(true);
    logic.queue_pending_script_fps_limit(15);
    logic.queue_pending_view_guardband(0.25, -0.10);
    logic.queue_pending_camera_focus(glam::Vec3::new(100.0, 0.0, 200.0));
    logic.queue_pending_camera_bw_mode(true, 30);
    logic.queue_pending_camera_shaker(glam::Vec3::new(40.0, 3.0, -80.0), 2.5, 0.4, 120.0);

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(frame.script_time_frozen);
    assert!(frame.time_frozen_for_simulation);
    assert_eq!(frame.script_fps_limit, Some(15));
    assert_eq!(frame.view_guardband, Some((0.25, -0.10)));
    assert_eq!(frame.camera_focus, Some([100.0, 0.0, 200.0]));
    assert_eq!(frame.camera_bw_mode, Some((true, 30)));
    assert!(
        frame
            .camera_shakers
            .iter()
            .any(|(pos, a, d, r)| (pos[0] - 40.0).abs() < 1e-5
                && (pos[1] - 3.0).abs() < 1e-5
                && (pos[2] + 80.0).abs() < 1e-5
                && (*a - 2.5).abs() < 1e-5
                && (*d - 0.4).abs() < 1e-5
                && (*r - 120.0).abs() < 1e-5)
    );

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(ui.script_time_frozen);
    assert!(ui.time_frozen_for_simulation);
    assert_eq!(ui.script_fps_limit, Some(15));
    assert_eq!(ui.view_guardband, Some((0.25, -0.10)));
    assert_eq!(ui.camera_focus, Some([100.0, 0.0, 200.0]));
    assert_eq!(ui.camera_bw_mode, Some((true, 30)));
    assert!(!ui.camera_shakers.is_empty());
}

#[test]
fn presentation_feeds_media_queue() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.queue_pending_movie("EALogo.bik");
    logic.queue_pending_radar_movie("RadarIntro.bik");
    logic.queue_pending_music_stop();
    logic.queue_pending_popup_message("General, hold the line!");

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.pending_movie.as_deref(), Some("EALogo.bik"));
    assert_eq!(frame.pending_radar_movie.as_deref(), Some("RadarIntro.bik"));
    assert!(frame.pending_music_stop);
    assert!(
        frame
            .pending_popup_messages
            .iter()
            .any(|m| m.message.contains("hold the line"))
    );

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert_eq!(ui.pending_movie.as_deref(), Some("EALogo.bik"));
    assert_eq!(ui.pending_radar_movie.as_deref(), Some("RadarIntro.bik"));
    assert!(ui.pending_music_stop);
    assert!(
        ui.pending_popup_messages
            .iter()
            .any(|m| m.contains("hold the line"))
    );
}

#[test]
fn presentation_feeds_mission_objectives() {
    use crate::game_logic::{Player, Team};
    use crate::ui::objectives::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "ObjP", true));
    logic.upsert_mission_objective(ObjectiveDisplay {
        id: Some("OBJ_HOLD".into()),
        title: "Hold the ridge".into(),
        description: "Defend until reinforcements arrive.".into(),
        status: ObjectiveStatus::Active,
        progress: Some((1, 3)),
        category: ObjectiveCategory::Primary,
    });
    logic.upsert_mission_objective(ObjectiveDisplay {
        id: Some("OBJ_SCOUT".into()),
        title: "Scout the pass".into(),
        description: "Reveal the northern FOW.".into(),
        status: ObjectiveStatus::Completed,
        progress: None,
        category: ObjectiveCategory::Secondary,
    });

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame
            .objectives
            .iter()
            .any(|o| o.title.contains("Hold the ridge") && o.status == ObjectiveStatus::Active),
        "objectives: {:?}",
        frame.objectives
    );
    assert!(
        frame
            .objectives
            .iter()
            .any(|o| o.id.as_deref() == Some("OBJ_SCOUT"))
    );

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert_eq!(ui.objectives.len(), frame.objectives.len());
    assert!(
        ui.objectives
            .iter()
            .any(|o| o.title.contains("Hold the ridge"))
    );
    assert!(ui.objectives.iter().any(|o| {
        o.id.as_deref() == Some("OBJ_SCOUT") && o.status == ObjectiveStatus::Completed
    }));
}

#[test]
fn presentation_feeds_script_and_cinematic_ui() {
    use crate::game_logic::{Player, Team};
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "ScriptP", true));
    if let Some(p) = logic.get_player_mut(0) {
        p.is_local = true;
        p.radar_count = 1;
        p.radar_disabled = false;
    }
    logic.push_script_ui_message("Objective updated: Hold the ridge");
    logic.set_cinematic_letterbox(true);
    logic.set_cinematic_text(Some("Incoming transmission...".into()));
    logic.set_military_caption(Some("General: Hold the line!".into()));
    logic.set_radar_forced(true);

    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame
            .script_messages
            .iter()
            .any(|m| m.contains("Hold the ridge"))
    );
    assert!(frame.cinematic_letterbox);
    assert_eq!(
        frame.cinematic_text.as_deref(),
        Some("Incoming transmission...")
    );
    assert_eq!(
        frame.military_caption.as_deref(),
        Some("General: Hold the line!")
    );
    assert!(frame.radar_forced);
    assert!(frame.radar_ui_enabled);

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(
        ui.script_messages
            .iter()
            .any(|m| m.contains("Hold the ridge"))
    );
    assert!(ui.cinematic_letterbox);
    assert_eq!(
        ui.cinematic_text.as_deref(),
        Some("Incoming transmission...")
    );
    assert_eq!(
        ui.military_caption.as_deref(),
        Some("General: Hold the line!")
    );
    assert!(ui.radar_forced);
    assert!(ui.radar_enabled);
}

#[test]
fn presentation_feeds_radar_into_ui_state() {
    use glam::Vec3;
    let mut logic = crate::game_logic::GameLogic::new();
    logic.queue_radar_message_at(
        "Enemy spotted north",
        Vec3::new(100.0, 0.0, 200.0),
        crate::game_logic::radar_notifications::RadarKind::Attack,
    );
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(frame.events.iter().any(|e| {
        matches!(
            e,
            PresentationEvent::RadarMessage {
                text,
                kind: 1,
                position,
                ..
            } if text.contains("Enemy") && (position.x - 100.0).abs() < 0.1
        )
    }));

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert!(
        ui.radar_messages.iter().any(|m| m.contains("Enemy")),
        "radar text: {:?}",
        ui.radar_messages
    );
    assert!(
        ui.radar_events
            .iter()
            .any(|e| e.kind == crate::ui::RadarPingKind::Attack),
        "radar events: {:?}",
        ui.radar_events
    );
    assert!(
        ui.radar_pings
            .iter()
            .any(|p| (p.position.x - 100.0).abs() < 0.1),
        "radar pings: {:?}",
        ui.radar_pings
    );
    assert_eq!(ui.last_radar_ping.map(|p| p.x), Some(100.0));
}

#[test]
fn sold_status_freezes_into_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(500.0);
    st.build_cost.supplies = 800;
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let id = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
    }
    assert!(logic.start_sell_object(id));
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame
        .objects
        .iter()
        .find(|o| o.id == id)
        .expect("renderable");
    assert!(ro.sold, "sold residual must freeze");
    assert!(ro.unselectable, "unselectable residual must freeze");
    assert!(
        (ro.model_condition_bits
            & (1u128
                << crate::game_logic::host_enum_table_residual::partially_constructed_model_bit()))
            != 0
    );
}

#[test]
fn reconstructing_freezes_into_presentation() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("GLABarracks".into(), st);
    let id = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = true;
        o.status.reconstructing = true;
        o.is_rebuild_hole = false;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!(ro.reconstructing);
    assert!(ro.under_construction);
}

#[test]
fn production_door_opening_freezes_into_presentation() {
    use crate::game_logic::game_logic::GameLogic;
    use crate::game_logic::host_enum_table_residual::{
        door_1_opening_model_bit, host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut st = ThingTemplate::new("AmericaBarracks");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), st);
    let id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(id) {
        o.start_production_door_cycle(0);
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.production_door_phase, 1);
    assert!(host_model_condition_has(
        ro.model_condition_bits,
        door_1_opening_model_bit()
    ));
    // UnitRenderInput also freezes bits.
    let uri = frame.unit_render_inputs();
    if let Some(u) = uri.iter().find(|u| u.id == id) {
        assert!(host_model_condition_has(
            u.model_condition_bits,
            door_1_opening_model_bit()
        ));
        assert_eq!(u.production_door_phase, 1);
    }
}

#[test]
fn construction_complete_freezes_into_presentation_events() {
    crate::game_logic::host_construction_log::clear();
    crate::game_logic::host_construction_log::record(
        crate::game_logic::ObjectId(42),
        "TestBarracks",
    );
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::ConstructionComplete {
                    id,
                    template
                } if id.0 == 42 && template == "TestBarracks"
            )
        }),
        "expected ConstructionComplete: {:?}",
        frame.events
    );
    // drained
    assert!(crate::game_logic::host_construction_log::drain().is_empty());
}

#[test]
fn radar_messages_freeze_into_presentation_events() {
    use glam::Vec3;
    let mut logic = crate::game_logic::GameLogic::new();
    logic.queue_radar_message_at(
        "Test radar ping",
        Vec3::ZERO,
        crate::game_logic::radar_notifications::RadarKind::Generic,
    );
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::RadarMessage { text, .. } if text.contains("Test radar")
            )
        }),
        "expected RadarMessage in presentation events: {:?}",
        frame.events
    );
}

#[test]
fn select_all_uses_locally_controlled_not_faction_team() {
    // C++ kindOfUnitSelection requires isLocallyControlled. Same-faction
    // 2v2 ally units must not enter SELECT_ALL.
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::USA, "USA-Ally", false));
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    t.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ranger".into(), t);
    let mine = logic
        .create_object_for_player("Ranger", 0, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("mine");
    let ally = logic
        .create_object_for_player("Ranger", 1, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ids = frame.alive_select_all_unit_ids(Team::USA, false);
    assert_eq!(
        ids,
        vec![mine],
        "SELECT_ALL must skip same-faction ally {ally:?}"
    );
    assert!(
        frame
            .objects
            .iter()
            .any(|o| o.id == ally && o.team == Team::USA && o.owner_player_id == Some(1)),
        "ally must be same-faction but not locally controlled"
    );
}
