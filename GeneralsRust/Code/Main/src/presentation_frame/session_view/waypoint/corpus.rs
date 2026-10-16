use super::*;
fn p(x: f32, y: f32, z: f32) -> Point {
    Point { x, y, z }
}
struct Host {
    pos: Point,
    yaw: f32,
    ground: f32,
    offset: glam::Vec3,
    previous: Point,
    debug: bool,
    paused: bool,
    script: bool,
    disabled: bool,
    did: bool,
    recalc: bool,
    mult: f32,
    freeze: bool,
    arrived: bool,
    constraint: Option<(f32, f32, f32, f32)>,
    calls: usize,
    scalars: usize,
}
impl Host {
    fn new() -> Self {
        Self {
            pos: p(0., 0., 0.),
            yaw: 0.,
            ground: 10.,
            offset: glam::Vec3::new(0., -100., 100.),
            previous: p(0., 0., 0.),
            debug: false,
            paused: false,
            script: false,
            disabled: false,
            did: false,
            recalc: false,
            mult: 1.,
            freeze: false,
            arrived: false,
            constraint: None,
            calls: 0,
            scalars: 0,
        }
    }
    fn view(&mut self) -> View<'_> {
        View {
            position_x: &mut self.pos.x,
            position_y: &mut self.pos.y,
            position_z: &mut self.pos.z,
            angle: &mut self.yaw,
            ground: &mut self.ground,
            camera_offset: &mut self.offset,
            basis_pitch_degrees: 37.5,
            basis_yaw_degrees: 0.,
            time_multiplier: &mut self.mult,
            frozen: &mut self.freeze,
            arrived: &mut self.arrived,
            constraint: &mut self.constraint,
        }
    }
    fn along(&mut self, chain: &[Point], ms: i32, shutter: i32) -> WaypointMotion {
        self.calls += 1;
        let end = chain.last().unwrap();
        WaypointMotion::along_chain(
            chain,
            self.pos,
            self.yaw,
            self.ground,
            40. + end.x * 0.1 + end.y * 0.2,
            self.mult as i32,
            ms,
            shutter,
            33,
            true,
            0.,
            0.,
            10.,
        )
    }
    fn offer(&mut self, m: &mut WaypointMotion) {
        self.did = false;
        self.recalc = false;
        if !self.debug && !self.paused {
            if m.active {
                self.previous = self.pos;
                let disabled = self.disabled;
                m.step(33, &mut self.view(), disabled);
                self.did = true;
                self.recalc = true;
            }
        } else if m.active {
            self.did = true;
        }
    }
    fn along_ease(&mut self, chain: &[Point], ms: i32, shutter: i32, ease: f32) -> WaypointMotion {
        self.calls += 1;
        // Reconstruct with authored ease; terrain was sampled exactly once.
        let end = chain.last().unwrap();
        WaypointMotion::along_chain(
            chain,
            self.pos,
            self.yaw,
            self.ground,
            40. + end.x * 0.1 + end.y * 0.2,
            self.mult as i32,
            ms,
            shutter,
            33,
            true,
            ease,
            ease,
            10.,
        )
    }
    fn setup(&mut self) -> WaypointMotion {
        self.calls += 1;
        let mut m = WaypointMotion::move_to(
            p(100., 100., 0.),
            self.pos,
            self.yaw,
            self.ground,
            70.,
            self.mult as i32,
            0,
            true,
            0.,
            0.,
        );
        m.instant_admission(&mut self.view(), false);
        m.look_toward(p(100., 200., 0.), &mut self.view(), false);
        self.scalars += 2;
        m
    }
}
fn base(name: &str, h: &Host, m: &WaypointMotion) -> String {
    let points: Vec<_> = m.points[..=m.num_waypoints + 1]
        .iter()
        .map(|p| format!("[{},{},{}]", p.x, p.y, p.z))
        .collect();
    format!(
        "{{\"case\":\"{}\",\"position\":[{},{},{}],\"angle\":{},\"start_angle\":{},\"nodes\":{},\"total_ms\":{},\"elapsed_ms\":{},\"shutter\":{},\"current_shutter\":{},\"segment\":{},\"moving\":{},\"freeze\":{},\"arrived\":{},\"ground_calls\":{},\"points\":[{}],\"angles\":{:?},\"ground\":{:?},\"scalar_dispatches\":{}}}",
        name,
        h.pos.x,
        h.pos.y,
        h.pos.z,
        h.yaw,
        m.angles[0],
        m.num_waypoints,
        m.total_time_ms,
        m.elapsed_ms,
        m.shutter,
        m.current_shutter,
        m.segment,
        m.active,
        h.freeze,
        h.arrived,
        h.calls,
        points.join(","),
        &m.angles[..=m.num_waypoints + 1],
        &m.grounds[..=m.num_waypoints + 1],
        h.scalars
    )
}
fn original_cases(out: &mut Vec<serde_json::Value>) {
    {
        let mut h = Host::new();
        h.pos = p(10., 20., 0.);
        h.yaw = 0.375;
        h.ground = 13.;
        let mut m = h.along(&[p(100., 20., 7.), p(100., 200., 9.)], 3000, 0);
        emit(out, "admit_owner_angle_ground_first_link", &h, &m);
        m.freeze_angle();
        emit(out, "freeze_saved_start_angle", &h, &m);
    }
    {
        let mut h = Host::new();
        let m = h.along(&[p(100., 0., 0.)], 1000, 65);
        emit(out, "shutter_65ms_floor", &h, &m);
        let mut m = h.along(&[p(100., 0., 0.)], 1000, 66);
        m.step(33, &mut h.view(), false);
        emit(out, "shutter_66ms_first_offer", &h, &m);
        m.step(33, &mut h.view(), false);
        emit(out, "shutter_66ms_second_offer", &h, &m);
    }
    {
        let mut h = Host::new();
        let m = h.along(&[p(3., 0., 0.), p(100., 0., 0.), p(103., 0., 0.)], 1000, 0);
        emit(out, "drop_short_middle_push_short_terminal", &h, &m);
    }
    {
        let mut h = Host::new();
        let m = h.along(&[p(100., 0., 0.)], 1000, 0);
        emit(out, "missing_start_preserves_path", &h, &m);
        emit(out, "missing_setup_target_preserves_path", &h, &m);
    }
    {
        let mut h = Host::new();
        h.yaw = 0.7;
        let m = h.setup();
        emit(out, "instant_setup_crossing_defers_pose_and_yaw", &h, &m);
        let mut m = h.along(&[p(200., 100., 0.)], 2000, 0);
        emit(
            out,
            "same_action_walk_new_path_observes_unadvanced_yaw",
            &h,
            &m,
        );
        m.freeze_angle();
        emit(out, "same_action_walk_freeze_preserves_start", &h, &m);
    }
    {
        let mut h = Host::new();
        h.yaw = 0.7;
        let mut m = h.setup();
        m.step(33, &mut h.view(), false);
        emit(out, "following_view_offer_completes_instant_setup", &h, &m);
        let m = h.along(&[p(200., 100., 0.)], 2000, 0);
        emit(out, "later_path_observes_completed_yaw", &h, &m);
    }
    {
        let mut h = Host::new();
        let mut m = h.along(&[p(100., 0., 0.)], 100, 0);
        h.freeze = true;
        m.step(100, &mut h.view(), false);
        emit(out, "exact_deadline_crossing_defers_completion", &h, &m);
        m.step(1, &mut h.view(), false);
        emit(out, "following_offer_completes_and_unfreezes", &h, &m);
    }
    {
        let mut h = Host::new();
        let mut m = h.along(&[p(100., 0., 0.)], 100, 0);
        h.freeze = true;
        m.step(101, &mut h.view(), true);
        emit(
            out,
            "disabled_terminal_unfreezes_without_pose_update",
            &h,
            &m,
        );
    }
}

fn emit(out: &mut Vec<serde_json::Value>, name: &str, h: &Host, m: &WaypointMotion) {
    out.push(serde_json::from_str(&base(name, h, m)).unwrap());
}
fn emit2(out: &mut Vec<serde_json::Value>, name: &str, h: &Host, m: &WaypointMotion) {
    let mut base_row = base(name, h, m);
    base_row.pop();
    let row = format!(
        "{},\"segment_length\":{:?},\"time_multipliers\":{:?},\"ground_level\":{},\"camera_offset\":[{},{},{}],\"time_multiplier\":{},\"segment_distance\":{},\"total_distance\":{},\"rolling_average\":{},\"previous_position\":[{},{},{}],\"gate_debug_frozen\":{},\"gate_game_paused\":{},\"gate_script_frozen\":{},\"gate_did_scripted\":{},\"gate_recalc\":{}}}",
        base_row,
        &m.lengths[..=m.num_waypoints + 1],
        &m.time_multipliers[..=m.num_waypoints + 1],
        h.ground,
        h.offset.x,
        h.offset.y,
        h.offset.z,
        h.mult,
        m.segment_distance,
        m.total_distance,
        m.rolling_average_frames,
        h.previous.x,
        h.previous.y,
        h.previous.z,
        h.debug,
        h.paused,
        h.script,
        h.did,
        h.recalc
    );
    out.push(serde_json::from_str(&row).unwrap());
}
pub(super) fn all_cases(out: &mut Vec<serde_json::Value>) {
    original_cases(out);
    {
        let mut h = Host::new();
        h.yaw = 0.3;
        h.ground = 12.;
        let mut m = h.along_ease(
            &[p(100., 0., 0.), p(100., 100., 0.), p(200., 200., 0.)],
            330,
            66,
            66.,
        );
        m.rolling_average(4);
        emit2(out, "rolling4_shutter2_admit", &h, &m);
        for i in 1..=12 {
            h.offer(&mut m);
            emit2(out, &format!("rolling4_shutter2_offer_{i}"), &h, &m);
        }
    }
    {
        let mut h = Host::new();
        h.yaw = -std::f32::consts::PI + 0.03;
        let mut m = h.along(&[p(100., 0., 0.)], 330, 0);
        m.look_toward(p(95., -100., 0.), &mut h.view(), false);
        m.rolling_average(3);
        emit2(out, "wrap_negative_to_positive_admit", &h, &m);
        for i in 1..=6 {
            h.offer(&mut m);
            emit2(out, &format!("wrap_negative_to_positive_{i}"), &h, &m);
        }
    }
    {
        let mut h = Host::new();
        h.yaw = std::f32::consts::PI - 0.03;
        let mut m = h.along(&[p(100., 0., 0.)], 330, 0);
        m.look_toward(p(105., -100., 0.), &mut h.view(), false);
        m.rolling_average(3);
        emit2(out, "wrap_positive_to_negative_admit", &h, &m);
        for i in 1..=6 {
            h.offer(&mut m);
            emit2(out, &format!("wrap_positive_to_negative_{i}"), &h, &m);
        }
    }
    {
        let mut h = Host::new();
        h.yaw = 0.2;
        let mut m = h.along(
            &[p(100., 0., 0.), p(100., 100., 0.), p(200., 100., 0.)],
            330,
            0,
        );
        m.look_toward(p(0., 200., 0.), &mut h.view(), false);
        emit2(out, "ordinary_look_changes_all_later_nodes", &h, &m);
        m.final_look_toward(p(300., -200., 0.));
        emit2(out, "final_look_preserves_earlier_node", &h, &m);
        m.final_move_to(p(300., 150., 7.));
        emit2(
            out,
            "final_move_shifts_nodes_without_rebuilding_lengths",
            &h,
            &m,
        );
        for i in 1..=4 {
            h.offer(&mut m);
            emit2(out, &format!("combined_final_modifiers_{i}"), &h, &m);
        }
    }
    {
        let mut h = Host::new();
        h.mult = 2.;
        let mut m = h.along(&[p(100., 0., 0.), p(300., 0., 0.), p(600., 0., 0.)], 330, 0);
        m.final_time_multiplier(7, &mut h.mult);
        emit2(out, "time_multiplier_distance_ramp_admit", &h, &m);
        for i in 1..=12 {
            h.offer(&mut m);
            if i == 3 {
                m.final_time_multiplier(4, &mut h.mult);
                emit2(out, "time_multiplier_second_modifier", &h, &m);
            }
            emit2(out, &format!("time_multiplier_ramp_offer_{i}"), &h, &m);
        }
        m.final_time_multiplier(5, &mut h.mult);
        emit2(out, "time_multiplier_idle_immediate", &h, &m);
    }
    {
        let mut h = Host::new();
        let mut m = h.along(&[p(100., 0., 0.)], 100, 66);
        h.freeze = true;
        emit2(out, "pause_debug_disabled_admit", &h, &m);
        h.paused = true;
        h.offer(&mut m);
        emit2(out, "game_pause_preserves_shutter_elapsed", &h, &m);
        h.paused = false;
        h.debug = true;
        h.offer(&mut m);
        emit2(out, "debug_freeze_preserves_shutter_elapsed", &h, &m);
        h.debug = false;
        h.script = true;
        h.offer(&mut m);
        emit2(out, "script_freeze_still_advances_path", &h, &m);
        h.script = false;
        h.disabled = true;
        h.offer(&mut m);
        emit2(out, "disabled_advances_elapsed_not_shutter", &h, &m);
        h.paused = true;
        h.offer(&mut m);
        emit2(out, "paused_disabled_does_not_admit", &h, &m);
        h.paused = false;
        h.offer(&mut m);
        emit2(out, "disabled_before_deadline", &h, &m);
        h.offer(&mut m);
        emit2(out, "disabled_terminal_unfreezes_no_pose", &h, &m);
    }
    {
        let mut h = Host::new();
        let mut m = h.along(&[p(100., 0., 0.)], 99, 66);
        h.freeze = true;
        h.offer(&mut m);
        h.offer(&mut m);
        emit2(out, "terminal_pause_before_crossing", &h, &m);
        h.paused = true;
        h.offer(&mut m);
        emit2(out, "terminal_pause_holds_pose_freeze", &h, &m);
        h.paused = false;
        h.offer(&mut m);
        emit2(out, "terminal_crossing_keeps_moving_and_freeze", &h, &m);
        h.paused = true;
        h.offer(&mut m);
        emit2(
            out,
            "terminal_pause_after_crossing_keeps_pending_completion",
            &h,
            &m,
        );
        h.paused = false;
        h.offer(&mut m);
        emit2(out, "terminal_resume_completes_before_shutter", &h, &m);
    }
}
