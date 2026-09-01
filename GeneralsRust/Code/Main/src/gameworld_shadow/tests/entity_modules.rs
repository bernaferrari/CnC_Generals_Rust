//! Phase 7: GENERALS_GAMEWORLD_ENTITY_MODULES spawn/destroy scaffolding.

use super::*;
use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
use gamelogic::world::{
    EntityModuleInstallSpec, HELPER_TAG_DEFECTION, HELPER_TAG_FIRING_TRACKER, HELPER_TAG_SMC,
    HELPER_TAG_STATUS, HELPER_TAG_SUBDUAL, HELPER_TAG_TEMP_WEAPON_BONUS, HELPER_TAG_WEAPON_STATUS,
    ctor_helper_tags,
};

#[test]
fn entity_modules_armed_installs_live_instances() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .set("GENERALS_GAMEWORLD_ENTITY_MODULES", "1");
    // Wave 153: "preview ENTITY_MODULES is default off"
    // (host_gameworld_authority_residual_wave153.rs:48); the attach is
    // opt-in via GENERALS_GAMEWORLD_ENTITY_MODULES, like every other
    // GameWorld authority family.
    assert!(crate::gameworld_shadow::gameworld_entity_modules_enabled());

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ModDefault");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "ModDefaultUnit", 50.0);
    let id = logic
        .create_object("ModDefaultUnit", Team::USA, Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&id.0).expect("map");
    let tags = shadow.world().entity_module_tags(eid).to_vec();
    assert!(!tags.is_empty());
    assert_eq!(shadow.world().entity_live_module_count(eid), tags.len());
    let installed = shadow
        .world()
        .entity(eid)
        .and_then(|e| e.entity_modules.clone())
        .expect("installed");
    assert!(installed.on_created);
    assert_eq!(installed.live_instances, tags.len());
}

#[test]
fn entity_modules_flag_off_does_not_install() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_ENTITY_MODULES", "0")
        .set("GENERALS_GAMEWORLD_SHADOW", "1");

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ModOff");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "ModOffUnit", 50.0);
    let id = logic
        .create_object("ModOffUnit", Team::USA, Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&id.0).expect("map");
    let entity = shadow.world().entity(eid).expect("entity");
    assert!(entity.entity_modules.is_none());
    assert!(shadow.world().entity_module_tags(eid).is_empty());
}

#[test]
fn entity_modules_flag_on_installs_template_plus_helpers() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_ENTITY_MODULES", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1");

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ModOn");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ModOnTank") {
        let mut t = ThingTemplate::new("ModOnTank");
        t.set_health(120.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ModOnTank".into(), t);
    }
    let id = logic
        .create_object("ModOnTank", Team::USA, Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    {
        let o = logic.host_object_mut(id).expect("o");
        o.weapon = Some(Weapon::default());
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&id.0).expect("map");
    let tags = shadow.world().entity_module_tags(eid).to_vec();
    let expected = ctor_helper_tags(&EntityModuleInstallSpec {
        has_weapons: true,
        ..EntityModuleInstallSpec::default()
    });
    assert_eq!(tags, expected);
    assert_eq!(
        tags,
        vec![
            HELPER_TAG_SMC,
            HELPER_TAG_STATUS,
            HELPER_TAG_SUBDUAL,
            HELPER_TAG_DEFECTION,
            HELPER_TAG_WEAPON_STATUS,
            HELPER_TAG_FIRING_TRACKER,
            HELPER_TAG_TEMP_WEAPON_BONUS,
        ]
    );
    let installed = shadow
        .world()
        .entity(eid)
        .and_then(|e| e.entity_modules.clone())
        .expect("installed");
    assert!(installed.on_created);
    assert_eq!(installed.records.len(), tags.len());
}

#[test]
fn entity_modules_on_delete_walks_list_order_before_remove() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_ENTITY_MODULES", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .set("GENERALS_GAMEWORLD_DEFERRED_DESTROY", "1");

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ModDel");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ModDelFact") {
        let mut t = ThingTemplate::new("ModDelFact");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("ModDelFact".into(), t);
    }
    let id = logic
        .create_object("ModDelFact", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&id.0).expect("map");
    let tags = shadow.world().entity_module_tags(eid).to_vec();
    assert!(tags.ends_with(&["ProductionUpdate".to_string()]));
    shadow.world_mut().mark_entity_destroyed(eid);
    assert_eq!(shadow.world_mut().process_destroy_list(), 1);
    assert!(shadow.world().entity(eid).is_none());
    assert_eq!(
        shadow.world().last_entity_on_delete_order(eid),
        tags.as_slice()
    );
}
