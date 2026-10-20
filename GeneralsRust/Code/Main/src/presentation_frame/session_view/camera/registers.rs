//! Persistent source rcInfo union payload, owned once by SessionCamera.
//! Optional words preserve unavailable initialization safely. Source-space XYZ
//! storage is converted exactly once at the receiving Rust Y-up boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum RotationKind {
    Scalar,
    Tracking,
}

/// One representation of the C++ overlapping payload, never a scalar mirror
/// plus a cached Vec3. Optional words retain initialization provenance safely.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RotationRegisters {
    first: Option<u32>,          // ObjectID OR startAngle.to_bits()
    second: Option<u32>,         // endAngle.to_bits() OR target position x.to_bits()
    target_yz: Option<[u32; 2]>, // source-space ground y, height z
    kind: RotationKind,
    active: bool,
    hold_frames: u32,
}

/// New complete envelope payload. This is NOT any frozen native/logic/session
/// DTO and must only be serialized inside a newly versioned owner envelope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RotationRegistersState {
    first: Option<u32>,
    second: Option<u32>,
    target_yz: Option<[u32; 2]>,
    kind: RotationKind,
    active: bool,
    hold_frames: u32,
}

impl Default for RotationRegisters {
    fn default() -> Self {
        Self {
            first: None,
            second: None,
            target_yz: None,
            kind: RotationKind::Scalar,
            active: false,
            hold_frames: 0,
        }
    }
}

impl RotationRegisters {
    pub(crate) fn kind(&self) -> RotationKind {
        self.kind
    }
    pub(crate) fn active(&self) -> bool {
        self.active
    }
    pub(crate) fn hold_frames(&self) -> u32 {
        self.hold_frames
    }

    /// Source rotateCamera/valid rotateCameraTowardPosition scalar stores.
    pub(crate) fn begin_scalar(&mut self, start: f32, end: f32) {
        self.scalar_prefix();
        self.first = Some(start.to_bits());
        self.second = Some(end.to_bits());
        self.active = true;
    }

    /// The look-toward prefix precedes its coincident-direction early return.
    /// A rejected direction changes kind/hold but does not write angle words.
    pub(crate) fn scalar_prefix(&mut self) {
        self.kind = RotationKind::Scalar;
        self.hold_frames = 0;
    }

    /// Source rotateCameraTowardObject writes ID, not target position.
    pub(crate) fn begin_tracking(&mut self, object_id: u32, hold_frames: u32) {
        self.kind = RotationKind::Tracking;
        self.first = Some(object_id);
        self.hold_frames = hold_frames;
        self.active = true;
    }

    pub(crate) fn tracking_id(&self) -> Option<u32> {
        (self.kind == RotationKind::Tracking)
            .then_some(self.first)
            .flatten()
    }

    /// Call only at the original enabled, admitted tracking observation phase.
    /// None means lookup failed: retaining the words is the source operation.
    /// Coordinates here are original (x, ground-y, height-z), not Rust Y-up.
    pub(crate) fn observe_tracking(&mut self, current: Option<glam::Vec3>) {
        if let Some(position) = current {
            let (x, y, z) = (position.x, position.z, position.y);
            self.second = Some(x.to_bits());
            self.target_yz = Some([y.to_bits(), z.to_bits()]);
        }
    }

    pub(crate) fn cached_target(&self) -> Option<glam::Vec3> {
        let x = f32::from_bits(self.second?);
        let [y, z] = self.target_yz?;
        Some(glam::Vec3::new(x, f32::from_bits(z), f32::from_bits(y)))
    }

    pub(crate) fn scalar_angles(&self) -> Option<[f32; 2]> {
        if self.kind != RotationKind::Scalar {
            return None;
        }
        let values = [f32::from_bits(self.first?), f32::from_bits(self.second?)];
        values.into_iter().all(f32::is_finite).then_some(values)
    }

    /// Source cameraModFreezeAngle only writes while a rotation is active.
    pub(crate) fn freeze_angle(&mut self, current_angle: f32) {
        if !self.active {
            return;
        }
        match self.kind {
            RotationKind::Tracking => self.first = Some(0), // INVALID_ID
            RotationKind::Scalar => {
                self.first = Some(current_angle.to_bits());
                self.second = Some(current_angle.to_bits());
            }
        }
    }

    /// Completion/cancellation clears activity, not the persistent registers.
    pub(crate) fn finish(&mut self) {
        self.active = false;
    }

    pub(crate) fn capture(&self) -> RotationRegistersState {
        RotationRegistersState {
            first: self.first,
            second: self.second,
            target_yz: self.target_yz,
            kind: self.kind,
            active: self.active,
            hold_frames: self.hold_frames,
        }
    }

    pub(crate) fn restore(state: RotationRegistersState) -> Self {
        Self {
            first: state.first,
            second: state.second,
            target_yz: state.target_yz,
            kind: state.kind,
            active: state.active,
            hold_frames: state.hold_frames,
        }
    }

    /// Import the EXISTING retained Rust tracking value from a historical V7
    /// DTO. This preserves that record's promised continuation, not proof that
    /// C++ had offered/observed it before that historical record was written.
    pub(crate) fn import_v7_tracking(
        id: u32,
        target: [f32; 3],
        active: bool,
        hold_frames: u32,
    ) -> Self {
        let mut owner = Self::default();
        owner.begin_tracking(id, hold_frames);
        owner.observe_tracking(Some(glam::Vec3::new(target[0], target[2], target[1])));
        owner.active = active;
        owner
    }

    /// V7 scalar records did not retain inactive end words or trailing cache.
    /// Do not guess those missing source values from current camera position.
    pub(crate) fn import_v7_scalar(start: f32, active_end: Option<f32>) -> Self {
        Self {
            first: Some(start.to_bits()),
            second: active_end.map(f32::to_bits),
            target_yz: None,
            kind: RotationKind::Scalar,
            active: active_end.is_some(),
            hold_frames: 0,
        }
    }
}

impl RotationRegisters {
    pub(crate) fn is_tracking(&self) -> bool {
        self.kind == RotationKind::Tracking
    }
    /// Compatibility observations only; never import these to repair V8 state.
    /// Tracking's old Rust scalar placeholders did not represent its cache.
    pub(crate) fn legacy_scalar(&self, current_yaw: f32) -> (f32, Option<f32>) {
        if self.is_tracking() {
            return (0., self.active.then_some(current_yaw));
        }
        let finite = |word: Option<u32>| {
            word.map(f32::from_bits)
                .filter(|value| value.is_finite())
                .unwrap_or(0.)
        };
        (finite(self.first), self.active.then(|| finite(self.second)))
    }
    pub(crate) fn legacy_id(&self) -> Option<u32> {
        self.tracking_id().filter(|id| *id != 0)
    }
    pub(crate) fn scalar_target(&self) -> Option<f32> {
        if self.kind != RotationKind::Scalar || !self.active {
            return None;
        }
        self.second.map(f32::from_bits)
    }
    pub(crate) fn scalar_start(&self) -> Option<f32> {
        (self.kind == RotationKind::Scalar)
            .then_some(self.first)
            .flatten()
            .map(f32::from_bits)
    }
}
impl RotationRegistersState {
    pub(crate) fn validate_timing(
        &self,
        timing: super::rotation::RotationTiming,
    ) -> Result<(), String> {
        self.validate()?;
        timing.validate()?;
        if self.kind == RotationKind::Tracking {
            let milliseconds = timing.duration_ms();
            if milliseconds < 1
                || timing.turn_frames != (milliseconds / 33).max(1) as u32
                || timing.turn_frames.checked_add(self.hold_frames).is_none()
            {
                return Err("invalid camera tracking rotation timing".into());
            }
        }
        Ok(())
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if (self.target_yz.is_some() && self.second.is_none())
            || (self.second.is_some() && self.first.is_none())
            || (self.active && self.first.is_none())
            || self.hold_frames > i32::MAX as u32 / 33
            || (self.kind == RotationKind::Scalar && self.hold_frames != 0)
            || (self.kind == RotationKind::Tracking && self.first.is_none())
        {
            return Err("invalid camera rotation registers".into());
        }
        if self.target_yz.is_some_and(|values| {
            values
                .into_iter()
                .any(|bits| !f32::from_bits(bits).is_finite())
        }) {
            return Err("nonfinite retained camera target coordinates".into());
        }
        if self.kind == RotationKind::Tracking
            && self.target_yz.is_some()
            && self
                .second
                .is_some_and(|bits| !f32::from_bits(bits).is_finite())
        {
            return Err("nonfinite retained camera target coordinate".into());
        }
        Ok(())
    }
}

#[cfg(test)]
impl RotationRegisters {
    /// Test observation only: raw source-order words, available after a real
    /// tracking offer has initialized all coordinates. No absent word fallback.
    pub(crate) fn initialized_source_words(&self) -> Option<[u32; 4]> {
        let [y, z] = self.target_yz?;
        Some([self.first?, self.second?, y, z])
    }
}
