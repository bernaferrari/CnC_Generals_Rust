#[derive(Debug, Clone, Copy)]
enum RecoilState {
    Idle,
    RecoilStart,
    Recoil,
    Settle,
}

fn recoil_state_to_i32(state: RecoilState) -> i32 {
    match state {
        RecoilState::Idle => 0,
        RecoilState::RecoilStart => 1,
        RecoilState::Recoil => 2,
        RecoilState::Settle => 3,
    }
}

fn recoil_state_from_i32(value: i32) -> RecoilState {
    match value {
        1 => RecoilState::RecoilStart,
        2 => RecoilState::Recoil,
        3 => RecoilState::Settle,
        _ => RecoilState::Idle,
    }
}

fn xfer_matrix3d_values(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) -> Result<(), String> {
    let cols = matrix.to_cols_array();
    let mut row0 = [cols[0], cols[4], cols[8], cols[12]];
    let mut row1 = [cols[1], cols[5], cols[9], cols[13]];
    let mut row2 = [cols[2], cols[6], cols[10], cols[14]];

    for value in row0
        .iter_mut()
        .chain(row1.iter_mut())
        .chain(row2.iter_mut())
    {
        xfer.xfer_real(value).map_err(|e| e.to_string())?;
    }

    let rebuilt_cols = [
        row0[0], row1[0], row2[0], 0.0, row0[1], row1[1], row2[1], 0.0, row0[2], row1[2], row2[2],
        0.0, row0[3], row1[3], row2[3], 1.0,
    ];
    *matrix = Matrix3D::from_cols_array(&rebuilt_cols);
    Ok(())
}

/// Weapon recoil information
#[derive(Debug, Clone)]
struct WeaponRecoilInfo {
    /// Current recoil state
    state: RecoilState,

    /// Current shift amount
    shift: Real,

    /// Recoil rate
    recoil_rate: Real,
}

impl WeaponRecoilInfo {
    fn new() -> Self {
        Self {
            state: RecoilState::Idle,
            shift: 0.0,
            recoil_rate: 0.0,
        }
    }
}

/// Animation override settings
///
/// Used to override animation behavior (duration, frame, etc.)
#[derive(Debug, Clone)]
struct AnimationOverride {
    /// Override for animation loop duration (in frames)
    duration_frames: Option<u32>,

    /// Override for animation completion time (in frames, for ONCE animations)
    completion_frames: Option<u32>,

    /// Manual frame override
    manual_frame: Option<i32>,
}

impl AnimationOverride {
    fn new() -> Self {
        Self {
            duration_frames: None,
            completion_frames: None,
            manual_frame: None,
        }
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.duration_frames = None;
        self.completion_frames = None;
        self.manual_frame = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveModelState {
    Condition(usize),
    Transition(usize),
}
