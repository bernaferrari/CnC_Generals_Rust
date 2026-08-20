#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseCondStateType {
    Normal,
    Default,
    Transition,
    Alias,
}

const INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT: u32 = 1 << 0;
const INI_READ_FLAG_GOT_NONIDLE_ANIMS: u32 = 1 << 1;
const INI_READ_FLAG_GOT_IDLE_ANIMS: u32 = 1 << 2;
const NAMEKEY_INVALID: NameKeyType = 0;
const AC_BITS_NAMES: &[&str] = &[
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];
const ACBIT_RANDOMSTART: u32 = 0;
const ACBIT_START_FRAME_FIRST: u32 = 1;
const ACBIT_START_FRAME_LAST: u32 = 2;
const ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT: u32 = 3;
const ACBIT_PRISTINE_BONE_POS_IN_FINAL_FRAME: u32 = 4;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES: u32 = 5;
const ACBIT_RESTART_ANIM_WHEN_COMPLETE: u32 = 6;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES2: u32 = 7;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES3: u32 = 8;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES4: u32 = 9;
const ALL_MAINTAIN_FRAME_FLAGS: u32 = (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES2)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES3)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES4);
const NO_NEXT_DURATION: u32 = u32::MAX;
const DEFAULT_ANIMATION_FRAMES: i32 = 30;
const MSEC_PER_LOGICFRAME_REAL: Real = 1000.0 / LOGICFRAMES_PER_SECOND as Real;
const DRAWABLE_STATUS_NO_STATE_PARTICLES: u32 = 0x00000008;

fn test_flag_bit(flags: u32, bit: u32) -> bool {
    (flags & (1u32 << bit)) != 0
}

fn is_any_maintain_frame_flag_set(flags: u32) -> bool {
    (flags & ALL_MAINTAIN_FRAME_FLAGS) != 0
}

fn is_common_maintain_frame_flag_set(a: u32, b: u32) -> bool {
    (a & ALL_MAINTAIN_FRAME_FLAGS & b & ALL_MAINTAIN_FRAME_FLAGS) != 0
}

fn anim_mode_to_i32(mode: AnimMode) -> i32 {
    match mode {
        AnimMode::Manual => 0,
        AnimMode::Loop => 1,
        AnimMode::Once => 2,
        AnimMode::LoopPingPong => 3,
        AnimMode::LoopBackwards => 4,
        AnimMode::OnceBackwards => 5,
    }
}

fn model_condition_valid_stuff(state: &ModelConditionInfo) -> u8 {
    let mut valid = state.valid_stuff;
    if !state.pristine_bones.is_empty() {
        valid |= MODEL_CONDITION_PRISTINE_BONES_VALID;
    }
    if !state.turrets.is_empty() {
        valid |= MODEL_CONDITION_TURRETS_VALID;
    }
    if state
        .weapon_projectile_launch_bone
        .iter()
        .any(|name| !name.is_empty())
    {
        valid |= MODEL_CONDITION_HAS_PROJECTILE_BONES;
    }
    if state
        .weapon_barrels
        .iter()
        .any(|barrels| !barrels.is_empty())
    {
        valid |= MODEL_CONDITION_BARRELS_VALID;
    }
    if !state.public_bones.is_empty() {
        valid |= MODEL_CONDITION_PUBLIC_BONES_VALID;
    }
    valid
}

// Constants
const WEAPONSLOT_COUNT: usize = 3;
const MAX_TURRETS: usize = 2;
const MODEL_CONDITION_PRISTINE_BONES_VALID: u8 = 0x01;
const MODEL_CONDITION_TURRETS_VALID: u8 = 0x02;
const MODEL_CONDITION_HAS_PROJECTILE_BONES: u8 = 0x04;
const MODEL_CONDITION_BARRELS_VALID: u8 = 0x08;
const MODEL_CONDITION_PUBLIC_BONES_VALID: u8 = 0x10;
