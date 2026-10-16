//! W3DView.cpp setupWaypointPath/moveAlongWaypointPath with receiving borrows.
//! New typed continuation state; never decode this as the historical path DTO.
//! Serde field order is explicit here; usize is encoded as u64 by bincode1.
//! Acceptance arithmetic pins unfused operations (-ffp-contract=off C++).
//! Coordinates retain original C++ XYZ. The receiving adapter maps XYZ to XZY.
//! No singleton, TLS, publication, synchronization, or duplicate yaw animation.

const MAX_WAYPOINTS: usize = 25;
const SLOTS: usize = MAX_WAYPOINTS + 2;

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Point {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
}

impl Point {
    pub(crate) fn from_world(position: glam::Vec3) -> Self {
        Self {
            x: position.x,
            y: position.z,
            z: position.y,
        }
    }
    pub(crate) fn world(self) -> glam::Vec3 {
        glam::Vec3::new(self.x, self.z, self.y)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Ease {
    input: f32,
    output: f32,
}
impl Ease {
    fn new(input: f32, output: f32) -> Self {
        let output = (1.0 - output).clamp(0.0, 1.0);
        Self {
            input: input.clamp(0.0, 1.0).min(output),
            output,
        }
    }
    fn eval(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let v0 = 1.0 + self.output - self.input;
        if t < self.input {
            t * t / (v0 * self.input)
        } else if t <= self.output {
            // Original 2.0 literal promotes this branch to double.
            ((f64::from(self.input) + 2.0 * f64::from(t - self.input)) / f64::from(v0)) as f32
        } else {
            ((f64::from(self.input)
                + 2.0 * f64::from(self.output - self.input)
                + (2.0 * f64::from(t - self.output) + f64::from(self.output * self.output)
                    - f64::from(t * t))
                    / f64::from(1.0 - self.output))
                / f64::from(v0)) as f32
        }
    }
}

/// These are the receiving View fields, not a new runtime owner or copied pose.
/// Integration should use disjoint &mut borrows of SessionView's actual fields.
pub(crate) struct View<'a> {
    pub(crate) position_x: &'a mut f32,
    pub(crate) position_y: &'a mut f32,
    pub(crate) position_z: &'a mut f32,
    pub(crate) angle: &'a mut f32,
    pub(crate) ground: &'a mut f32,
    pub(crate) camera_offset: &'a mut glam::Vec3,
    pub(crate) basis_pitch_degrees: f32,
    pub(crate) basis_yaw_degrees: f32,
    pub(crate) time_multiplier: &'a mut f32,
    pub(crate) frozen: &'a mut bool,
    pub(crate) arrived: &'a mut bool,
    pub(crate) constraint: &'a mut Option<(f32, f32, f32, f32)>, // lowX, highX, lowY, highY
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Inactive,
    Disabled,
    DisabledFinished,
    Finished,
    SegmentTerminal, // total_time_ms becomes zero, final pose comes next offer
    Shutter,
    Pose,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct WaypointMotion {
    pub(crate) num_waypoints: usize,
    pub(crate) points: [Point; SLOTS],
    pub(crate) lengths: [f32; SLOTS],
    pub(crate) angles: [f32; SLOTS],
    pub(crate) time_multipliers: [i32; SLOTS],
    // C++ declared one slot too few for setup's numWaypoints+1 at full capacity.
    // Safe capacity repairs that out-of-bounds write without dropping a point.
    pub(crate) grounds: [f32; SLOTS],
    pub(crate) total_time_ms: i32,
    pub(crate) elapsed_ms: i32,
    pub(crate) total_distance: f32,
    pub(crate) segment_distance: f32,
    pub(crate) shutter: i32,
    pub(crate) segment: usize,
    pub(crate) current_shutter: i32,
    pub(crate) rolling_average_frames: i32,
    pub(crate) ease: Ease,
    pub(crate) active: bool,
}

impl WaypointMotion {
    /// Caller resolves the ordered link-0 chain and supplies the actual ground
    /// under its final admitted point. No world lookup occurs here. MIN_DELTA
    /// is the original MAP_XY_FACTOR supplied by the content/coordinate adapter.
    pub(crate) fn along_chain(
        chain: &[Point],
        start: Point,
        start_angle: f32,
        start_ground: f32,
        final_ground: f32,
        time_multiplier: i32,
        milliseconds: i32,
        shutter_ms: i32,
        frame_ms: i32,
        orient: bool,
        ease_in_ms: f32,
        ease_out_ms: f32,
        min_delta: f32,
    ) -> Self {
        assert!(frame_ms > 0);
        let mut motion = Self::empty(
            start,
            start_angle,
            milliseconds,
            (shutter_ms / frame_ms).max(1),
            ease_in_ms,
            ease_out_ms,
        );
        for (index, point) in chain.iter().enumerate() {
            if motion.num_waypoints == MAX_WAYPOINTS {
                break;
            }
            motion.num_waypoints += 1;
            let n = motion.num_waypoints;
            motion.points[n] = *point;
            let previous = motion.points[n - 1];
            if length(point.x - previous.x, point.y - previous.y) < min_delta {
                if index + 1 == chain.len() {
                    motion.points[n - 1] = *point;
                }
                motion.num_waypoints -= 1;
            }
        }
        motion.setup(
            start_angle,
            start_ground,
            final_ground,
            time_multiplier,
            orient,
        );
        motion
    }

    /// C++ moveCameraTo ignores its shutter argument and uses a one-frame shutter.
    /// Call instant_admission immediately after construction, before modifiers.
    pub(crate) fn move_to(
        target: Point,
        start: Point,
        angle: f32,
        ground: f32,
        final_ground: f32,
        time_multiplier: i32,
        milliseconds: i32,
        orient: bool,
        ease_in_ms: f32,
        ease_out_ms: f32,
    ) -> Self {
        let mut motion = Self::empty(start, angle, milliseconds, 1, ease_in_ms, ease_out_ms);
        motion.num_waypoints = 2;
        motion.points[2] = target;
        motion.setup(angle, ground, final_ground, time_multiplier, orient);
        motion
    }

    fn empty(
        start: Point,
        angle: f32,
        milliseconds: i32,
        shutter: i32,
        ease_in_ms: f32,
        ease_out_ms: f32,
    ) -> Self {
        let total_time_ms = milliseconds.max(1);
        let mut points = [Point::default(); SLOTS];
        points[0] = start;
        points[1] = start;
        let mut angles = [0.0; SLOTS];
        angles[0] = angle;
        Self {
            num_waypoints: 1,
            points,
            lengths: [0.0; SLOTS],
            angles,
            time_multipliers: [0; SLOTS],
            grounds: [0.0; SLOTS],
            total_time_ms,
            elapsed_ms: 0,
            total_distance: 0.0,
            segment_distance: 0.0,
            shutter,
            segment: 1,
            current_shutter: shutter,
            rolling_average_frames: 1,
            ease: Ease::new(
                ease_in_ms / total_time_ms as f32,
                ease_out_ms / total_time_ms as f32,
            ),
            active: false,
        }
    }

    fn setup(
        &mut self,
        angle: f32,
        ground: f32,
        final_ground: f32,
        time_multiplier: i32,
        orient: bool,
    ) {
        let n = self.num_waypoints;
        let mut next_angle = angle;
        for i in 1..n {
            let dx = self.points[i + 1].x - self.points[i].x;
            let dy = self.points[i + 1].y - self.points[i].y;
            self.lengths[i] = length(dx, dy);
            self.total_distance += self.lengths[i];
            if orient {
                next_angle = f64::from(dx / self.lengths[i]).acos() as f32;
                if dy < 0.0 {
                    next_angle = -next_angle;
                }
                next_angle -= std::f32::consts::PI / 2.0;
                norm(&mut next_angle);
            }
            self.angles[i] = next_angle;
        }
        self.angles[1] = angle;
        self.angles[n] = self.angles[n - 1];
        for i in (2..n).rev() {
            self.angles[i] = (self.angles[i] + self.angles[i - 1]) / 2.0;
        }
        self.lengths[n + 1] = self.lengths[n];
        if self.total_distance < 1.0 {
            self.lengths[n - 1] += 1.0 - self.total_distance;
            self.total_distance = 1.0;
        }
        let mut distance = 0.0;
        for i in 0..=n + 1 {
            let second = distance / self.total_distance;
            self.time_multipliers[i] = time_multiplier;
            self.grounds[i] = ground * (1.0 - second) + final_ground * second;
            distance += self.lengths[i];
        }
        self.points[n + 1] = self.points[n];
        self.points[n + 1].x += self.points[n].x - self.points[n - 1].x;
        self.points[n + 1].y += self.points[n].y - self.points[n - 1].y;
        self.angles[n + 1] = self.angles[n];
        self.grounds[n + 1] = final_ground;
        self.points[0].x -= self.points[2].x - self.points[1].x;
        self.points[0].y -= self.points[2].y - self.points[1].y;
        self.active = n > 1;
    }

    /// Mirrors moveCameraTo's special synchronous1ms call, including restoring
    /// the active flag and clearing arrival AFTER the call. Never force a pose.
    pub(crate) fn instant_admission(
        &mut self,
        view: &mut View<'_>,
        disabled: bool,
    ) -> Option<Step> {
        if self.total_time_ms != 1 {
            return None;
        }
        let result = self.step(1, view, disabled);
        self.active = true;
        *view.arrived = false;
        Some(result)
    }

    pub(crate) fn freeze_angle(&mut self) {
        if self.active {
            for i in 1..=self.num_waypoints {
                self.angles[i] = self.angles[0];
            }
        }
    }
    pub(crate) fn look_toward(&mut self, target: Point, view: &mut View<'_>, disabled: bool) {
        if !self.active {
            return;
        }
        for i in 2..=self.num_waypoints {
            let start = midpoint(self.points[i - 1], self.points[i]);
            let mid = self.points[i];
            let end = midpoint(self.points[i], self.points[i + 1]);
            let mut result = start;
            result.x += 0.5 * (end.x - start.x);
            result.y += 0.5 * (end.y - start.y);
            result.x += (1.0 - 0.5) * 0.5 * (mid.x - end.x + mid.x - start.x);
            result.y += (1.0 - 0.5) * 0.5 * (mid.y - end.y + mid.y - start.y);
            let dx = target.x - result.x;
            let dy = target.y - result.y;
            let len = length(dx, dy);
            if len < 0.1 {
                continue;
            }
            let mut angle = f64::from(dx / len).acos() as f32;
            if dy < 0.0 {
                angle = -angle;
            }
            angle -= std::f32::consts::PI / 2.0;
            norm(&mut angle);
            self.angles[i] = angle;
        }
        self.instant_admission(view, disabled);
    }

    /// Original retarget shifts points only; cached lengths/angle tables stay.
    pub(crate) fn final_move_to(&mut self, target: Point) {
        if !self.active {
            return;
        }
        let old = self.points[self.num_waypoints];
        let dx = target.x - old.x;
        let dy = target.y - old.y;
        for i in 2..=self.num_waypoints {
            self.points[i].x += dx;
            self.points[i].y += dy;
        }
    }

    pub(crate) fn final_look_toward(&mut self, target: Point) {
        if !self.active {
            return;
        }
        for i in self.num_waypoints.saturating_sub(1).max(2)..=self.num_waypoints {
            let start = midpoint(self.points[i - 1], self.points[i]);
            let mid = self.points[i];
            let end = midpoint(self.points[i], self.points[i + 1]);
            let mut result = start;
            result.x += 0.5 * (end.x - start.x);
            result.y += 0.5 * (end.y - start.y);
            result.x += (1.0 - 0.5) * 0.5 * (mid.x - end.x + mid.x - start.x);
            result.y += (1.0 - 0.5) * 0.5 * (mid.y - end.y + mid.y - start.y);
            let dx = target.x - result.x;
            let dy = target.y - result.y;
            let len = length(dx, dy);
            if len < 0.1 {
                continue;
            }
            let mut angle = f64::from(dx / len).acos() as f32;
            if dy < 0.0 {
                angle = -angle;
            }
            angle -= std::f32::consts::PI / 2.0;
            norm(&mut angle);
            if i != self.num_waypoints {
                let mut delta = angle - self.angles[i];
                norm(&mut delta);
                angle = self.angles[i] + delta / 2.0;
                norm(&mut angle);
            }
            self.angles[i] = angle;
        }
    }
    pub(crate) fn final_time_multiplier(&mut self, value: i32, idle_multiplier: &mut f32) {
        if !self.active {
            *idle_multiplier = value as f32;
            return;
        }
        let mut distance = 0.0;
        for i in 0..self.num_waypoints {
            distance += self.lengths[i];
            let second = distance / self.total_distance;
            let first = 1.0 - second;
            self.time_multipliers[i + 1] = (0.5
                + f64::from(self.time_multipliers[i + 1] as f32 * first)
                + f64::from(value as f32 * second))
            .floor() as i32;
        }
    }
    pub(crate) fn rolling_average(&mut self, frames: i32) {
        self.rolling_average_frames = frames.max(1);
    }

    pub(crate) fn step(&mut self, milliseconds: i32, view: &mut View<'_>, disabled: bool) -> Step {
        if !self.active {
            return Step::Inactive;
        }
        // Valid source frame inputs fit i32; explicitly retain integer wrap.
        self.elapsed_ms = self.elapsed_ms.wrapping_add(milliseconds);
        if disabled {
            if self.elapsed_ms > self.total_time_ms {
                self.active = false;
                *view.frozen = false;
                return Step::DisabledFinished;
            }
            return Step::Disabled;
        }
        let n = self.num_waypoints;
        if self.elapsed_ms > self.total_time_ms {
            self.active = false;
            *view.arrived = false;
            *view.frozen = false;
            *view.angle = self.angles[n];
            *view.ground = self.grounds[n];
            update_basis(view);
            let mut position = self.points[n];
            position.z = 0.0;
            commit_focus(view, position);
            return Step::Finished;
        }
        let total = self.total_time_ms as f32;
        let delta = self.ease.eval(self.elapsed_ms as f32 / total)
            - self
                .ease
                .eval(self.elapsed_ms.wrapping_sub(milliseconds) as f32 / total);
        self.segment_distance += delta * self.total_distance;
        while self.segment_distance >= self.lengths[self.segment] {
            *view.arrived = true;
            self.segment_distance -= self.lengths[self.segment];
            self.segment += 1;
            if self.segment >= n {
                self.total_time_ms = 0;
                return Step::SegmentTerminal;
            }
        }
        let mut average = 1.0 / self.rolling_average_frames as f32;
        self.current_shutter -= 1;
        if self.current_shutter > 0 {
            return Step::Shutter;
        }
        self.current_shutter = self.shutter;
        let mut factor = self.segment_distance / self.lengths[self.segment];
        if self.segment == n - 1 {
            average = (f64::from(average) + (1.0 - f64::from(average)) * f64::from(factor)) as f32;
        }
        let first = 1.0 - factor;
        let second = 1.0 - first;
        let mut angle1 = self.angles[self.segment];
        let angle2 = self.angles[self.segment + 1];
        if angle2 - angle1 > std::f32::consts::PI {
            angle1 += 2.0 * std::f32::consts::PI;
        }
        if angle2 - angle1 < -std::f32::consts::PI {
            angle1 -= 2.0 * std::f32::consts::PI;
        }
        let mut angle = angle1 * first + angle2 * second;
        norm(&mut angle);
        let mut delta_angle = angle - *view.angle;
        norm(&mut delta_angle);
        *view.angle += average * delta_angle;
        norm(view.angle);
        let multiplier = self.time_multipliers[self.segment] as f32 * first
            + self.time_multipliers[self.segment + 1] as f32 * second;
        *view.time_multiplier = ((0.5 + f64::from(multiplier)).floor() as i32) as f32;
        *view.ground = self.grounds[self.segment] * first + self.grounds[self.segment + 1] * second;
        update_basis(view);
        let (start, mid, end) = if factor < 0.5 {
            let triple = (
                midpoint(self.points[self.segment - 1], self.points[self.segment]),
                self.points[self.segment],
                midpoint(self.points[self.segment], self.points[self.segment + 1]),
            );
            factor += 0.5;
            triple
        } else {
            let triple = (
                midpoint(self.points[self.segment], self.points[self.segment + 1]),
                self.points[self.segment + 1],
                midpoint(self.points[self.segment + 1], self.points[self.segment + 2]),
            );
            factor -= 0.5;
            triple
        };
        let mut result = start;
        result.x += factor * (end.x - start.x);
        result.y += factor * (end.y - start.y);
        result.x += (1.0 - factor) * factor * (mid.x - end.x + mid.x - start.x);
        result.y += (1.0 - factor) * factor * (mid.y - end.y + mid.y - start.y);
        result.z = 0.0;
        commit_focus(view, result);
        Step::Pose
    }
}
fn length(x: f32, y: f32) -> f32 {
    f64::from(x * x + y * y).sqrt() as f32
}
fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
        z: a.z,
    }
}
fn norm(angle: &mut f32) {
    let pi = std::f32::consts::PI;
    if *angle < -10.0 * pi {
        *angle = 0.0;
    }
    if *angle > 10.0 * pi {
        *angle = 0.0;
    }
    while *angle < -pi {
        *angle += 2.0 * pi;
    }
    while *angle > pi {
        *angle -= 2.0 * pi;
    }
}
fn commit_focus(view: &mut View<'_>, point: Point) {
    *view.position_x = point.x;
    *view.position_y = point.y;
    *view.position_z = point.z;
    *view.constraint = Some(match *view.constraint {
        Some((low_x, high_x, low_y, high_y)) => (
            low_x.min(point.x),
            high_x.max(point.x),
            low_y.min(point.y),
            high_y.max(point.y),
        ),
        None => (point.x, point.x, point.y, point.y),
    });
}

fn update_basis(view: &mut View<'_>) {
    let radians = f64::from(std::f32::consts::PI) / 180.0;
    view.camera_offset.y = -(f64::from(view.camera_offset.z)
        / (f64::from(view.basis_pitch_degrees) * radians).tan()) as f32;
    view.camera_offset.x = -(f64::from(view.camera_offset.y)
        * (f64::from(view.basis_yaw_degrees) * radians).tan()) as f32;
}

impl Default for WaypointMotion {
    fn default() -> Self {
        Self::empty(Point::default(), 0.0, 1, 1, 0.0, 0.0)
    }
}
impl WaypointMotion {
    /// Reject malformed continuation before it can index the fixed arrays.
    /// Cached lengths deliberately need not match retargeted points: C++
    /// cameraModFinalMoveTo modifies points without rebuilding those lengths.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_WAYPOINTS).contains(&self.num_waypoints)
            || !(1..=self.num_waypoints).contains(&self.segment)
            || (self.active && self.num_waypoints < 2)
            || self.total_time_ms < 0
            || self.elapsed_ms < 0
            || self.shutter < 1
            || self.current_shutter < 1
            || self.current_shutter > self.shutter
            || self.rolling_average_frames < 1
            || !self.total_distance.is_finite()
            || self.total_distance < 0.0
            || !self.segment_distance.is_finite()
            || self.segment_distance < 0.0
            || !self.ease.input.is_finite()
            || !self.ease.output.is_finite()
            || !(0.0..=self.ease.output).contains(&self.ease.input)
            || !(0.0..=1.0).contains(&self.ease.output)
        {
            return Err("invalid waypoint continuation dimensions or clocks".into());
        }
        if self.active
            && (self.total_distance < 1.0
                || ((self.total_time_ms == 0) != (self.segment == self.num_waypoints))
                || (self.total_time_ms > 0 && self.elapsed_ms > self.total_time_ms)
                || (self.segment < self.num_waypoints && self.lengths[self.segment] <= 0.0))
        {
            return Err("invalid active waypoint continuation phase".into());
        }
        if self
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
            || self.lengths.iter().any(|n| !n.is_finite() || *n < 0.0)
            || self
                .angles
                .iter()
                .chain(self.grounds.iter())
                .any(|n| !n.is_finite())
        {
            return Err("non-finite waypoint continuation geometry".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
