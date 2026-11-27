//! Bridge presentation cues and host coordinate conversion.

use glam::Vec3;

use generals_sim::bridge_behavior::{BRIDGE_DAMAGED_TO_SOUND, BRIDGE_REPAIRED_TO_SOUND};

pub(crate) fn bridge_deck_corners(info: &generals_sim::terrain::BridgeInfo) -> [Vec3; 4] {
    [info.from_left, info.from_right, info.to_left, info.to_right]
        .map(|point| Vec3::new(point.x, point.z, point.y))
}

/// C++ `onBodyDamageStateChange` DamagedTo / RepairedTo cue.
#[derive(Debug, Clone, Default)]
pub struct HostBridgeBodyFxCue {
    pub fx: Vec<String>,
    pub ocl: Vec<String>,
    pub sound: Option<String>,
    pub repaired: bool,
    pub state: u8,
}

fn leftover_is_condition_worse(old_state: u8, new_state: u8) -> bool {
    // C++ IS_CONDITION_WORSE(old, new) := old > new (old is worse than new → repaired).
    old_state > new_state
}

fn leftover_road_for_template(
    name: &str,
) -> Option<game_engine::common::ini::ini_terrain_bridge::TerrainRoadType> {
    let ascii = game_engine::common::ascii_string::AsciiString::from(name);
    game_engine::common::ini::ini_terrain_bridge::IniTerrainBridge::find_terrain_bridge_by_name(
        &ascii,
    )
}

pub(crate) fn body_transition_cues_for(old_state: u8, new_state: u8) -> HostBridgeBodyFxCue {
    let repaired = leftover_is_condition_worse(old_state, new_state);
    let mut cue = HostBridgeBodyFxCue {
        repaired,
        state: new_state,
        ..HostBridgeBodyFxCue::default()
    };
    if let Some(road) = leftover_road_for_template("Concrete")
        .or_else(|| leftover_road_for_template("GenericBridge"))
    {
        let idx = new_state as usize;
        for i in 0..3 {
            if repaired {
                if let Some(name) = road.get_repaired_to_fx_string(idx, i) {
                    let s = name.as_str().trim();
                    if !s.is_empty() {
                        cue.fx.push(s.to_string());
                    }
                }
                if let Some(name) = road.get_repaired_to_ocl_string(idx, i) {
                    let s = name.as_str().trim();
                    if !s.is_empty() {
                        cue.ocl.push(s.to_string());
                    }
                }
            } else {
                if let Some(name) = road.get_damage_to_fx_string(idx, i) {
                    let s = name.as_str().trim();
                    if !s.is_empty() {
                        cue.fx.push(s.to_string());
                    }
                }
                if let Some(name) = road.get_damage_to_ocl_string(idx, i) {
                    let s = name.as_str().trim();
                    if !s.is_empty() {
                        cue.ocl.push(s.to_string());
                    }
                }
            }
        }
        cue.sound = if repaired {
            road.get_repaired_to_sound_string(idx)
        } else {
            road.get_damage_to_sound_string(idx)
        }
        .and_then(|s| {
            let t = s.as_str().trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });
    }
    if cue.sound.is_none() && (new_state == 1 || new_state == 2 || repaired) {
        cue.sound = Some(if repaired {
            BRIDGE_REPAIRED_TO_SOUND.to_string()
        } else {
            BRIDGE_DAMAGED_TO_SOUND.to_string()
        });
    }
    cue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use generals_sim::bridge_behavior::*;

    #[test]
    fn repair_creates_rising_scaffold_and_blocks_heal() {
        // C++ DozerAIUpdate.cpp:665-688 createBridgeScaffolding + isScaffoldInMotion.
        let mut reg = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(7);
        reg.register_span(
            id,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 40.0),
            Vec3::new(10.0, 0.0, 40.0),
        );
        assert!(reg.create_scaffolding(id));
        assert!(reg.is_scaffold_in_motion(id));
        for _ in 0..BRIDGE_SCAFFOLD_RISE_FRAMES {
            assert!(reg.is_scaffold_in_motion(id));
            reg.tick_scaffolds();
        }
        assert!(!reg.is_scaffold_in_motion(id));
        assert!(reg.is_scaffold_present(id));
        assert!(reg.scaffolds_created > 0);
        let tiles = reg.tiled_scaffold_sites(id);
        assert!(
            tiles.len() >= 2,
            "C++ tiles from both ends, got {}",
            tiles.len()
        );
        assert!(!reg.create_scaffolding(id));
        let gone = reg.remove_scaffolding(id);
        assert!(gone.is_empty());
        assert!(!reg.is_scaffold_present(id));
    }

    #[test]
    fn rubble_restamps_and_splats_deck_units() {
        // C++ TerrainLogic.cpp Bridge::updateDamageState :852-909.
        let mut reg = HostBridgeBehaviorRegistry::new();
        let bridge = ObjectId(1);
        let unit = ObjectId(2);
        reg.register_span(
            bridge,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 20.0),
            Vec3::new(20.0, 0.0, 20.0),
        );
        let on_deck = reg.occupants_on_deck(bridge, &[(unit, Vec3::new(10.0, 0.0, 10.0))]);
        assert_eq!(on_deck, vec![unit]);
        assert!(reg.on_enter_rubble(bridge, &on_deck));
        assert!(!reg.on_enter_rubble(bridge, &on_deck));
        assert!(reg.rubble_restamps > 0 && reg.splat_kills > 0);
    }

    #[test]
    fn tower_damage_mirrors_to_span_and_siblings() {
        let mut reg = HostBridgeBehaviorRegistry::new();
        let span = ObjectId(1);
        let t0 = ObjectId(10);
        let t1 = ObjectId(11);
        reg.register_span(span, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0));
        reg.bind_towers(span, [t0, t1, ObjectId(0), ObjectId(0)]);
        let targets = reg.mirror_targets(t0);
        assert!(targets.contains(&span));
        assert!(targets.contains(&t1));
        assert!(!targets.contains(&t0));
        assert_eq!(reg.span_id_for(t1), Some(span));
    }

    #[test]
    fn scaffold_tiles_from_both_ends_and_rise_then_build() {
        let mut reg = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(7);
        reg.register_span(
            id,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 40.0),
            Vec3::new(10.0, 0.0, 40.0),
        );
        let tiles = reg.tiled_scaffold_sites(id);
        let deck: Vec<_> = tiles.iter().filter(|t| !t.is_support).collect();
        assert!(
            deck.len() >= 2,
            "C++ tiles from both ends, got {}",
            deck.len()
        );
        assert!(
            deck.iter()
                .any(|t| (t.rise_to - deck[0].rise_to).length() > 1.0),
            "left and right rise points must differ"
        );
        for tile in deck {
            assert!(
                tile.create_pos.y < tile.rise_to.y,
                "C++ setScaffoldData starts sunken"
            );
            let mut anim =
                HostScaffoldAnim::from_tile(ObjectId(1), tile, Vec3::new(5.0, 0.0, 20.0));
            assert_eq!(anim.motion, HostScaffoldMotion::Rise);
            let first = anim.step(anim.last_pos);
            assert!(first.y > tile.create_pos.y);
            anim.last_pos = first;
            for _ in 0..200 {
                if anim.motion == HostScaffoldMotion::Still {
                    break;
                }
                let next = anim.step(anim.last_pos);
                anim.last_pos = next;
            }
            assert_eq!(anim.motion, HostScaffoldMotion::Still);
        }
    }

    #[test]
    fn mid_damage_damaged_state_syncs() {
        let mut reg = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(3);
        reg.register_span(id, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0));
        assert!(reg.note_body_state(id, 1));
        assert_eq!(reg.damaged_syncs, 1);
        assert!(reg.note_body_state(id, 2));
        assert_eq!(reg.damaged_syncs, 2);
        assert!(reg.note_body_state(id, 3));
        assert_eq!(reg.damaged_syncs, 2);
        let cue = body_transition_cues_for(0, 1);
        assert!(!cue.repaired);
        assert_eq!(cue.state, 1);
        assert_eq!(cue.sound.as_deref(), Some(BRIDGE_DAMAGED_TO_SOUND));
        let repair = body_transition_cues_for(3, 1);
        assert!(repair.repaired);
        assert_eq!(repair.sound.as_deref(), Some(BRIDGE_REPAIRED_TO_SOUND));
    }

    #[test]
    fn staged_die_fx_and_ocl_fire_at_delays() {
        let mut reg = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(9);
        reg.register_span(id, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0));
        reg.mark_death(id, 10);
        assert_eq!(reg.take_due_die_fx(id, 10), 1);
        assert_eq!(reg.take_due_die_ocl(id, 10), 1);
        assert_eq!(reg.take_due_die_fx(id, 10), 0);
        assert_eq!(reg.take_due_die_ocl(id, 10), 0);
        assert_eq!(reg.take_due_die_fx(id, 18), 1);
        assert_eq!(reg.take_due_die_ocl(id, 18), 1);
        assert!(reg.die_fx >= 2);
        assert!(reg.die_ocl >= 2);
    }
    #[test]
    fn callback_slots_reread_routing_without_reselecting_the_original_span() {
        let mut reg = HostBridgeBehaviorRegistry::new();
        let span = ObjectId(1);
        let other = ObjectId(2);
        let victim = ObjectId(10);
        let old = ObjectId(11);
        let replacement = ObjectId(12);
        for id in [span, other] {
            reg.register_span(id, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::ONE);
        }
        reg.bind_towers(span, [victim, old, ObjectId(0), ObjectId(0)]);
        let captured_span = reg.span_id_for(victim).unwrap();
        assert_eq!(reg.callback_target_at(captured_span, victim, 0), None);
        // A completed recipient may rewrite a later slot before it is visited.
        reg.bind_towers(span, [victim, replacement, ObjectId(0), ObjectId(0)]);
        assert_eq!(
            reg.callback_target_at(captured_span, victim, 1),
            Some(replacement)
        );
        reg.bind_towers(other, [victim, ObjectId(0), ObjectId(0), ObjectId(0)]);
        assert_eq!(reg.span_id_for(victim), Some(other));
        assert_eq!(reg.callback_target_at(captured_span, victim, 4), Some(span));
        assert_eq!(reg.callback_target_at(span, span, 4), None);
        assert_eq!(reg.callback_target_at(span, span, 2), None);
    }
    #[test]
    fn original_bridge_death_zero_sentinel_and_wrapped_clock_survive_encoded_owner() {
        let mut registry = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(9);
        registry.register_span(id, Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::X + Vec3::Z);
        registry.mark_death(id, 0);
        assert_eq!(registry.span(id).unwrap().death_frame, 0);
        assert_eq!(registry.take_due_die_fx(id, 0), 0);
        assert_eq!(registry.take_due_die_ocl(id, 8), 0);
        registry.mark_death(id, u32::MAX - 3);
        let encoded = bincode_legacy::serialize(&registry).unwrap();
        let mut restored: HostBridgeBehaviorRegistry =
            bincode_legacy::deserialize(&encoded).unwrap();
        assert_eq!(restored.take_due_die_fx(id, 4), 1);
        assert_eq!(restored.take_due_die_ocl(id, 4), 1);
        assert_eq!(restored.take_due_die_fx(id, 4), 0);
        assert_eq!(restored.take_due_die_ocl(id, 4), 0);
        // Original onDie assigns the current frame, not only the first ever hit.
        restored.mark_death(id, 13);
        assert_eq!(restored.span(id).unwrap().death_frame, 13);
        assert_eq!(restored.take_due_die_fx(id, 13), 1);
    }

    #[test]
    fn random_surface_position_draws_three_logic_reals_in_cpp_order() {
        use game_engine::common::random_value::{RandomState, SessionRandom};
        let mut registry = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(10);
        let (fl, fr, tl, tr) = (
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(40.0, 5.0, 0.0),
            Vec3::new(0.0, 5.0, 100.0),
            Vec3::new(40.0, 5.0, 100.0),
        );
        registry.register_span(id, fl, fr, tl, tr);
        let mut seeded = RandomState::default();
        seeded.seed_random(31);
        let span = registry.span(id).unwrap();
        let a = SessionRandom::from_state(seeded.clone());
        let got = span.random_surface_position(|lo, hi| a.next_real(lo, hi), 11.0);
        // BridgeBehavior.cpp:511-535: scale(to-left), scale(from-right), height.
        let mut reference = seeded.clone();
        let s1 = reference.next_real(0.0, 1.0);
        let s2 = reference.next_real(0.0, 1.0);
        let h = reference.next_real(0.0, 11.0);
        assert_eq!(
            got,
            fl + (tl - fl) * s1 + (fr - fl) * s2 + Vec3::new(0.0, h, 0.0)
        );
        assert_eq!(a.snapshot().seed_words(), reference.seed_words());
        // TransitionEffectsHeight 0 consumes no third draw (delta <= 0 returns hi).
        let b = SessionRandom::from_state(seeded.clone());
        let flat = span.random_surface_position(|lo, hi| b.next_real(lo, hi), 0.0);
        assert_eq!(flat.y, 5.0);
        let mut two = seeded;
        two.next_real(0.0, 1.0);
        two.next_real(0.0, 1.0);
        assert_eq!(b.snapshot().seed_words(), two.seed_words());
    }
}
