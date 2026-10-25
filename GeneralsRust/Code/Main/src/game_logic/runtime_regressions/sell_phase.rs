//! Executable command and lifecycle regressions.

use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn owned_sell_phase_finishes_without_ready_writeback() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "OwnedSeller", true));
    ensure_template(&mut logic, "SfSellPad", 1000);
    let id = logic
        .create_object_for_player("SfSellPad", 0, Vec3::ZERO)
        .unwrap();
    assert!(logic.start_sell_object(id));
    // The actual sell list retains its admission frame. Advance through
    // the CPP scaffold delay and decrement phase, never a shadow-ready log.
    // CPP BuildAssistant: 45-frame scaffold delay followed by 1/90
    // per-frame decrement until -0.5, including the final float boundary.
    for frame in 0..=179 {
        logic.set_current_frame(frame);
        logic.update_sell_list();
        if frame == 44 {
            // CPP startSell sets 99.9%, retained through frame44.
            assert_eq!(logic.host_object(id).unwrap().construction_percent, 0.999);
        }
        if frame == 178 {
            assert!(
                logic.host_object(id).is_some(),
                "sell must retain the final scaffold frame"
            );
        }
        logic.process_destroy_list();
        if logic.host_object(id).is_none() {
            break;
        }
    }
    assert!(logic.host_object(id).is_none());
    assert!(logic.sell_list_for_snapshot().is_empty());
}

fn ensure_template(logic: &mut GameLogic, name: &str, cost: u32) {
    let mut t = ThingTemplate::new(name);
    t.set_health(200.0);
    t.set_cost(cost, 0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert(name.into(), t);
}
