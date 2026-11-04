//! hq-2qedt.30: Patriot assist slot/clip bind from the admitted
//! AssistedTargetingUpdate module (AssistedTargetingUpdate.cpp:74-75,
//! AssistedTargetingUpdate.h:26-27 defaults), not hardcoded clip 4 / slot 0.
use super::*;
use crate::game_logic::host_base_defense::HostAssistedTargetingData;
use crate::game_logic::{Player, Team};
use glam::Vec3;

const BODY: &str = " KindOf = STRUCTURE SELECTABLE FS_BASE_DEFENSE\n Body = ActiveBody BodyTag\n MaxHealth = 600\n InitialHealth = 600\n End\n";

fn world(source: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "assisted-targeting-admission.ini".into(),
                source.to_string(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "PatriotOwner", true));
    world.add_player(Player::new(1, Team::GLA, "Victim", true));
    world
}

fn patriot(module: &str) -> String {
    format!("Object AmericaPatriotBattery\n{BODY}{module}End\nObject AuthoredVictim\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n")
}

#[test]
fn authored_assist_module_binds_clip_and_slot() {
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 2\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert_eq!(
        w.host_object(id).unwrap().assisted_targeting,
        Some(HostAssistedTargetingData { clip_size: 2, weapon_slot: 1 })
    );
}

#[test]
fn authored_assist_module_defaults_match_cpp() {
    let mut w = world(&patriot("Behavior = AssistedTargetingUpdate AssistTag\nEnd\n"));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert_eq!(
        w.host_object(id).unwrap().assisted_targeting,
        Some(HostAssistedTargetingData { clip_size: 1, weapon_slot: 0 })
    );
}

#[test]
fn patriot_name_without_assist_module_stays_unbound() {
    let mut w = world(&patriot(""));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert!(w.host_object(id).unwrap().assisted_targeting.is_none());
}

#[test]
fn pending_assist_clip_comes_from_bound_module_not_retail_four() {
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 2\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let assist = w.host_object(id).unwrap().assisted_targeting.expect("bound");
    let pending = crate::game_logic::host_base_defense::PendingPatriotAssist::new(
        id, id, id, 1, "AmericaPatriotBattery", assist,
    );
    assert_eq!(pending.shots_remaining, 2);
    assert_eq!(pending.weapon_slot, 1);
}
