//! Executable command and lifecycle regressions.

fn command_formation_log() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::{Vec2, Vec3};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Form204");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("FormUnit204") {
        let mut t = ThingTemplate::new("FormUnit204");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("FormUnit204".into(), t);
    }
    let Some(a) = logic.create_object("FormUnit204", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("FormUnit204", Team::USA, Vec3::new(10.0, 0.0, 0.0)) else {
        return false;
    };

    if let Some(unit) = logic.get_object_mut(a) {
        unit.set_formation(7, Vec2::new(1.0, -2.0));
        if unit.formation_id != 7 {
            return false;
        }
    } else {
        return false;
    }
    if let Some(unit) = logic.get_object_mut(b) {
        unit.set_formation(7, Vec2::new(-1.0, 2.0));
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    [(a, Vec2::new(1.0, -2.0)), (b, Vec2::new(-1.0, 2.0))]
        .into_iter()
        .all(|(id, offset)| {
            let Some(owner) = logic.get_object(id) else {
                return false;
            };
            let Some(entity_id) = shadow.entity_for_host(id) else {
                return false;
            };
            let Some(observed) = shadow.world().entity(entity_id) else {
                return false;
            };
            owner.formation_id == 7
                && owner.formation_offset == offset
                && observed.formation_id == 7
                && observed.formation_offset == [offset.x, offset.y]
        })
}

#[test]
fn command_formation_log_runs() {
    assert!(command_formation_log());
}
