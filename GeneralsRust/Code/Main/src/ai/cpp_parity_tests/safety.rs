use super::*;

#[test]
fn is_location_safe_rejects_enemies_not_harvesters_or_undetected_stealth() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut pad = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    pad.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    pad.geometry_info.authored = true;
    pad.geometry_info.major_radius = 10.0;
    logic.templates.insert("AmericaSupplyCenter".into(), pad);

    let mut ranger = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".into(), ranger);

    let mut harvester = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
    harvester
        .add_kind_of(crate::game_logic::KindOf::Harvester)
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleChinook".into(), harvester);

    let mut dozer = crate::game_logic::ThingTemplate::new("ChinaVehicleDozer");
    dozer
        .add_kind_of(crate::game_logic::KindOf::Dozer)
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(200.0);
    logic.templates.insert("ChinaVehicleDozer".into(), dozer);

    let template = logic.templates.get("AmericaSupplyCenter").cloned();
    let pos = Vec3::ZERO;
    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(!ai.is_location_safe(&logic, pos, None));
    assert!(ai.is_location_safe(&logic, pos, template.as_ref()));

    let enemy = logic
        .create_object("ChinaInfantryRedguard", Team::China, pos)
        .expect("enemy");
    assert!(!ai.is_location_safe(&logic, pos, template.as_ref()));

    if let Some(obj) = logic.host_object_mut(enemy) {
        obj.status.stealthed = true;
        obj.status.detected = false;
        obj.status.disguised = false;
    }
    assert!(
        ai.is_location_safe(&logic, pos, template.as_ref()),
        "stealthed-unless-detected must not fail safety"
    );
    if let Some(obj) = logic.host_object_mut(enemy) {
        obj.status.detected = true;
    }
    assert!(!ai.is_location_safe(&logic, pos, template.as_ref()));

    logic.destroy_object(enemy);
    let _ = logic
        .create_object("AmericaVehicleChinook", Team::China, pos)
        .expect("harvester");
    let _ = logic
        .create_object("ChinaVehicleDozer", Team::China, pos)
        .expect("dozer");
    assert!(
        ai.is_location_safe(&logic, pos, template.as_ref()),
        "C++ rejects HARVESTER and DOZER from the safety scan"
    );
}
