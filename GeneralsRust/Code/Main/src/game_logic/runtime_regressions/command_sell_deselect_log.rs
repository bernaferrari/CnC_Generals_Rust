//! Executable command and lifecycle regressions.

fn command_sell_deselect_log() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Sell212");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("SellBldg212") {
        let mut t = ThingTemplate::new("SellBldg212");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellBldg212".into(), t);
    }
    let Some(bid) = logic.create_object("SellBldg212", Team::USA, Vec3::new(15.0, 0.0, 15.0))
    else {
        return false;
    };
    // Select then sell.
    if let Some(obj) = logic.host_object_mut(bid) {
        obj.select();
    }
    if !logic.start_sell_object(bid) {
        return false;
    }
    logic
        .host_object(bid)
        .is_some_and(|object| object.status.sold && !object.selected)
}

#[test]
fn command_sell_deselect_log_runs() {
    assert!(command_sell_deselect_log());
}
