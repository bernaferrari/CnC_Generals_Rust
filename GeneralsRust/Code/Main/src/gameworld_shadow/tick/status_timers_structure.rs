//! Status timers: structure collapse / topple / fire-when-damaged / base regen.

use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 775–780: structure collapse/topple, FWWD continuous, base regen.
    pub(super) fn tick_status_structure(&mut self, eid: EntityId, frame: u32) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
        // Wave 775: StructureCollapseUpdate residual (building sink collapse).
        // state: 0 Standing, 1 WaitingForStart, 2 Collapsing, 3 Done
        if e.structure_collapse_state != 0 && e.structure_collapse_state != 3 {
            use crate::game_logic::host_structure_collapse::STRUCTURE_COLLAPSE_GRAVITY;
            let mut done = false;
            match e.structure_collapse_state {
                1 => {
                    // WaitingForStart — shudder residual
                    if e.structure_collapse_max_shudder > 0.0 {
                        let t = frame as f32 * 0.37;
                        e.structure_collapse_shudder_x = t.sin() * e.structure_collapse_max_shudder;
                        e.structure_collapse_shudder_z =
                            (t * 1.3).cos() * e.structure_collapse_max_shudder;
                    } else {
                        e.structure_collapse_shudder_x = 0.0;
                        e.structure_collapse_shudder_z = 0.0;
                    }
                    if frame >= e.structure_collapse_start_frame {
                        e.structure_collapse_state = 2;
                        e.structure_collapse_velocity = 0.0;
                    }
                }
                2 => {
                    // Collapsing
                    e.structure_collapse_current_height -= e.structure_collapse_velocity;
                    e.structure_collapse_velocity -=
                        STRUCTURE_COLLAPSE_GRAVITY * (1.0 - e.structure_collapse_damping);
                    if e.structure_collapse_max_shudder > 0.0 {
                        let t = frame as f32 * 0.37;
                        e.structure_collapse_shudder_x = t.sin() * e.structure_collapse_max_shudder;
                        e.structure_collapse_shudder_z =
                            (t * 1.3).cos() * e.structure_collapse_max_shudder;
                    } else {
                        e.structure_collapse_shudder_x = 0.0;
                        e.structure_collapse_shudder_z = 0.0;
                    }
                    if e.structure_collapse_current_height + e.structure_collapse_building_height
                        <= 0.0
                    {
                        e.structure_collapse_current_height = -e.structure_collapse_building_height;
                        e.structure_collapse_shudder_x = 0.0;
                        e.structure_collapse_shudder_z = 0.0;
                        e.structure_collapse_state = 3;
                        done = true;
                    }
                }
                _ => {}
            }
            if done {
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_structure_collapse_kill_log::record(
                        crate::game_logic::ObjectId(hid),
                    );
                }
            }
            changed = true;
        }
        // Wave 776: StructureToppleUpdate residual (building fall death).
        // state: 0 Standing, 1 WaitingForStart, 2 Toppling, 3 WaitingForDone, 4 Done
        if e.structure_topple_state != 0 && e.structure_topple_state != 4 {
            use crate::game_logic::host_structure_topple::{
                STRUCTURE_TOPPLE_ACCEL_FACTOR, STRUCTURE_TOPPLE_DONE_DELAY_FRAMES,
                STRUCTURE_TOPPLE_INTEGRITY_DEFAULT,
            };
            let mut done = false;
            match e.structure_topple_state {
                1 => {
                    if frame >= e.structure_topple_start_frame {
                        e.structure_topple_state = 2;
                        e.structure_topple_structural_integrity =
                            STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
                    }
                }
                2 => {
                    let integrity_term = (1.0 - e.structure_topple_structural_integrity).max(0.0);
                    let topple_acceleration = STRUCTURE_TOPPLE_ACCEL_FACTOR
                        * e.structure_topple_accumulated_angle.sin()
                        * integrity_term;
                    let accel = if e.structure_topple_velocity <= 1e-6
                        && e.structure_topple_accumulated_angle <= 1e-6
                    {
                        STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.05
                    } else {
                        topple_acceleration.max(STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.01)
                    };
                    e.structure_topple_velocity += accel;
                    if e.structure_topple_structural_integrity > 0.0 {
                        e.structure_topple_structural_integrity *=
                            e.structure_topple_structural_decay;
                        if e.structure_topple_structural_integrity < 0.0 {
                            e.structure_topple_structural_integrity = 0.0;
                        }
                    }
                    e.structure_topple_accumulated_angle += e.structure_topple_velocity;
                    e.structure_topple_lean_radians = e.structure_topple_accumulated_angle;
                    if e.structure_topple_accumulated_angle >= std::f32::consts::FRAC_PI_2 {
                        e.structure_topple_accumulated_angle = std::f32::consts::FRAC_PI_2;
                        e.structure_topple_lean_radians = e.structure_topple_accumulated_angle;
                        e.structure_topple_state = 3;
                        e.structure_topple_done_frame =
                            frame.saturating_add(STRUCTURE_TOPPLE_DONE_DELAY_FRAMES);
                    }
                }
                3 => {
                    if frame >= e.structure_topple_done_frame {
                        e.structure_topple_state = 4;
                        done = true;
                    }
                }
                _ => {}
            }
            // Wave 777: StructureTopple applyCrushingDamage residual samples.
            if matches!(e.structure_topple_state, 2 | 3 | 4) {
                use crate::game_logic::host_structure_topple::{
                    HostStructureToppleData, HostStructureToppleState,
                };
                let mut st = HostStructureToppleData {
                    state: match e.structure_topple_state {
                        1 => HostStructureToppleState::WaitingForStart,
                        2 => HostStructureToppleState::Toppling,
                        3 => HostStructureToppleState::WaitingForDone,
                        4 => HostStructureToppleState::Done,
                        _ => HostStructureToppleState::Standing,
                    },
                    topple_start_frame: e.structure_topple_start_frame,
                    dir_x: e.structure_topple_dir_x,
                    dir_y: e.structure_topple_dir_y,
                    topple_velocity: e.structure_topple_velocity,
                    accumulated_angle: e.structure_topple_accumulated_angle,
                    structural_integrity: e.structure_topple_structural_integrity,
                    structural_decay: e.structure_topple_structural_decay,
                    done_frame: e.structure_topple_done_frame,
                    lean_radians: e.structure_topple_lean_radians,
                    last_crushed_location: e.structure_topple_last_crushed_location,
                    building_height: e.structure_topple_building_height,
                    facing_width: e.structure_topple_facing_width,
                    ..Default::default()
                };
                let samples =
                    st.take_crush_sweep_samples(e.transform.position.x, e.transform.position.z);
                e.structure_topple_last_crushed_location = st.last_crushed_location;
                if !samples.is_empty() {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_structure_topple_crush_log::record(
                            crate::game_logic::ObjectId(hid),
                            samples,
                        );
                    }
                }
            }
            if done {
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_structure_topple_kill_log::record(
                        crate::game_logic::ObjectId(hid),
                    );
                }
            }
            changed = true;
        }
        // Wave 778: FireWeaponWhenDamagedBehavior continuous residual.
        if e.fwwd_active {
            use crate::game_logic::host_enum_table_residual::{
                HostBodyDamageType, host_calc_body_damage_state,
            };
            let reload = e.fwwd_continuous_reload_frames.max(1);
            // C++ ctor reloadAmmo: first observation starts the clip-reload clock.
            if e.fwwd_last_continuous_frame == 0 {
                e.fwwd_last_continuous_frame = frame;
                changed = true;
            } else if frame.saturating_sub(e.fwwd_last_continuous_frame) >= reload {
                let max_h = e.max_health.max(e.health).max(1.0);
                let state = host_calc_body_damage_state(e.health, max_h);
                let name = match state {
                    HostBodyDamageType::Pristine => e.fwwd_continuous_pristine.as_str(),
                    HostBodyDamageType::Damaged => e.fwwd_continuous_damaged.as_str(),
                    HostBodyDamageType::ReallyDamaged => e.fwwd_continuous_really_damaged.as_str(),
                    HostBodyDamageType::Rubble => e.fwwd_continuous_rubble.as_str(),
                };
                // Continuous weapons typically only for damaged+ unless pristine set.
                let skip_pristine = matches!(state, HostBodyDamageType::Pristine)
                    && e.fwwd_continuous_pristine.is_empty();
                if !skip_pristine && !name.is_empty() {
                    e.fwwd_last_continuous_frame = frame;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_fwwd_continuous_log::record(
                            crate::game_logic::ObjectId(hid),
                            name.to_string(),
                        );
                    }
                    changed = true;
                }
            }
        }
        // Wave 780: BaseRegenerateUpdate residual (structure auto-heal).
        if e.base_regen_active && !e.base_regen_done_sold {
            use crate::game_logic::host_base_regenerate::{
                BASE_REGEN_HEAL_RATE_FRAMES, base_regen_heal_amount,
            };
            if e.base_regen_pending_damage {
                // C++ onDamage non-healing: delay wake.
                use crate::game_logic::host_base_regenerate::BASE_REGEN_DELAY_FRAMES;
                e.base_regen_wake_frame = frame.saturating_add(BASE_REGEN_DELAY_FRAMES);
                e.base_regen_pending_damage = false;
                changed = true;
            }
            if e.sold {
                e.base_regen_done_sold = true;
                changed = true;
            } else if !e.under_construction {
                let max_h = e.max_health.max(e.health).max(1.0);
                if e.health + f32::EPSILON < max_h && frame >= e.base_regen_wake_frame {
                    let elapsed = frame.saturating_sub(e.base_regen_wake_frame);
                    if elapsed % BASE_REGEN_HEAL_RATE_FRAMES == 0 {
                        let amount = base_regen_heal_amount(max_h);
                        if amount > 0.0 {
                            e.health = (e.health + amount).min(max_h);
                            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                                crate::game_logic::host_heal_log::record(
                                    crate::game_logic::ObjectId(hid),
                                    e.health,
                                );
                            }
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
    }
}
