pub use game_engine::common::model_condition::{ModelConditionFlag, ModelConditionFlags};

// Individual ObjectStatus constants for easier use (matching C++ enum values)
pub const OBJECT_STATUS_NONE: ObjectStatusTypes = ObjectStatusTypes::None;
pub const OBJECT_STATUS_MASKED: ObjectStatusTypes = ObjectStatusTypes::Masked;
pub const OBJECT_STATUS_CAN_STEALTH: ObjectStatusTypes = ObjectStatusTypes::CanStealth;
pub const OBJECT_STATUS_RECONSTRUCTING: ObjectStatusTypes = ObjectStatusTypes::Reconstructing;
pub const OBJECT_STATUS_UNDER_CONSTRUCTION: ObjectStatusTypes =
    ObjectStatusTypes::UnderConstruction;
pub const OBJECT_STATUS_SOLD: ObjectStatusTypes = ObjectStatusTypes::Sold;

// Individual ModelCondition constants for easier use (matching C++ enum values)
pub const MODELCONDITION_PARACHUTING: ModelConditionFlags = ModelConditionFlags::PARACHUTING;
pub const MODELCONDITION_FREEFALL: ModelConditionFlags = ModelConditionFlags::FREEFALL;
pub const MODELCONDITION_PRONE: ModelConditionFlags = ModelConditionFlags::PRONE;
pub const MODELCONDITION_PANICKING: ModelConditionFlags = ModelConditionFlags::PANICKING;
pub const MODELCONDITION_EXPLODED_FLAILING: ModelConditionFlags =
    ModelConditionFlags::EXPLODED_FLAILING;
pub const MODELCONDITION_EXPLODED_BOUNCING: ModelConditionFlags =
    ModelConditionFlags::EXPLODED_BOUNCING;
pub const MODELCONDITION_SPLATTED: ModelConditionFlags = ModelConditionFlags::SPLATTED;
pub const MODELCONDITION_STUNNED_FLAILING: ModelConditionFlags =
    ModelConditionFlags::STUNNED_FLAILING;
pub const MODELCONDITION_STUNNED: ModelConditionFlags = ModelConditionFlags::STUNNED;
pub const MODELCONDITION_CAPTURED: ModelConditionFlags = ModelConditionFlags::CAPTURED;
pub const MODELCONDITION_RUBBLE: ModelConditionFlags = ModelConditionFlags::RUBBLE;
pub const MODELCONDITION_TOPPLED: ModelConditionFlags = ModelConditionFlags::TOPPLED;
pub const MODELCONDITION_FLOODED: ModelConditionFlags = ModelConditionFlags::FLOODED;
pub const MODELCONDITION_CLIMBING: ModelConditionFlags = ModelConditionFlags::CLIMBING;
pub const MODELCONDITION_RAPPELLING: ModelConditionFlags = ModelConditionFlags::RAPPELLING;
pub const MODELCONDITION_ENEMYNEAR: ModelConditionFlags = ModelConditionFlags::ENEMYNEAR;
pub const MODELCONDITION_POST_COLLAPSE: ModelConditionFlags = ModelConditionFlags::POST_COLLAPSE;
pub const MODELCONDITION_BURNED: ModelConditionFlags = ModelConditionFlags::BURNED;
pub const MODELCONDITION_ACTIVELY_CONSTRUCTING: ModelConditionFlags =
    ModelConditionFlags::ActivelyConstructing;
pub const MODELCONDITION_DOOR_1_OPENING: ModelConditionFlags = ModelConditionFlags::Door1Opening;
pub const MODELCONDITION_DOOR_1_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door1WaitingOpen;
pub const MODELCONDITION_DOOR_1_CLOSING: ModelConditionFlags = ModelConditionFlags::Door1Closing;
pub const MODELCONDITION_DOOR_2_OPENING: ModelConditionFlags = ModelConditionFlags::Door2Opening;
pub const MODELCONDITION_DOOR_2_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door2WaitingOpen;
pub const MODELCONDITION_DOOR_2_CLOSING: ModelConditionFlags = ModelConditionFlags::Door2Closing;
pub const MODELCONDITION_DOOR_3_OPENING: ModelConditionFlags = ModelConditionFlags::Door3Opening;
pub const MODELCONDITION_DOOR_3_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door3WaitingOpen;
pub const MODELCONDITION_DOOR_3_CLOSING: ModelConditionFlags = ModelConditionFlags::Door3Closing;
pub const MODELCONDITION_DOOR_4_OPENING: ModelConditionFlags = ModelConditionFlags::Door4Opening;
pub const MODELCONDITION_DOOR_4_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door4WaitingOpen;
pub const MODELCONDITION_DOOR_4_CLOSING: ModelConditionFlags = ModelConditionFlags::Door4Closing;
pub const MODELCONDITION_DOOR_1_WAITING_TO_CLOSE: ModelConditionFlags =
    ModelConditionFlags::Door1WaitingToClose;
pub const MODELCONDITION_JETAFTERBURNER: ModelConditionFlags = ModelConditionFlags::JETAFTERBURNER;
pub const MODELCONDITION_JETEXHAUST: ModelConditionFlags = ModelConditionFlags::JETEXHAUST;
pub const MODELCONDITION_SPECIAL_CHEERING: ModelConditionFlags =
    ModelConditionFlags::SPECIAL_CHEERING;
pub const MODELCONDITION_SPECIAL_DAMAGED: ModelConditionFlags =
    ModelConditionFlags::SPECIAL_DAMAGED;
pub const MODELCONDITION_ATTACKING: ModelConditionFlags = ModelConditionFlags::ATTACKING;
pub const MODELCONDITION_DYING: ModelConditionFlags = ModelConditionFlags::DYING;
pub const MODELCONDITION_CARRYING: ModelConditionFlags = ModelConditionFlags::CARRYING;
pub const MODELCONDITION_DEPLOYED: ModelConditionFlags = ModelConditionFlags::DEPLOYED;
pub const MODELCONDITION_MOVING: ModelConditionFlags = ModelConditionFlags::MOVING;
pub const MODELCONDITION_PACKING: ModelConditionFlags = ModelConditionFlags::Packing;
pub const MODELCONDITION_UNPACKING: ModelConditionFlags = ModelConditionFlags::Unpacking;
pub const MODELCONDITION_OVER_WATER: ModelConditionFlags = ModelConditionFlags::OVER_WATER;
pub const MODELCONDITION_SOLD: ModelConditionFlags = ModelConditionFlags::SOLD;
pub const MODELCONDITION_ARMED: ModelConditionFlags = ModelConditionFlags::ARMED;
pub const MODELCONDITION_SECOND_LIFE: ModelConditionFlags = ModelConditionFlags::SECOND_LIFE;
pub const MODELCONDITION_JAMMED: ModelConditionFlags = ModelConditionFlags::JAMMED;
pub const MODELCONDITION_WEAPONSET_VETERAN: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_VETERAN;
pub const MODELCONDITION_WEAPONSET_ELITE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_ELITE;
pub const MODELCONDITION_WEAPONSET_HERO: ModelConditionFlags = ModelConditionFlags::WEAPONSET_HERO;
pub const MODELCONDITION_WEAPONSET_CRATEUPGRADE_ONE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_CRATEUPGRADE_ONE;
pub const MODELCONDITION_WEAPONSET_CRATEUPGRADE_TWO: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_CRATEUPGRADE_TWO;
pub const MODELCONDITION_WEAPONSET_PLAYER_UPGRADE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE;
pub const MODELCONDITION_RIDER1: ModelConditionFlags = ModelConditionFlags::RIDER1;
pub const MODELCONDITION_RIDER2: ModelConditionFlags = ModelConditionFlags::RIDER2;
pub const MODELCONDITION_RIDER3: ModelConditionFlags = ModelConditionFlags::RIDER3;
pub const MODELCONDITION_RIDER4: ModelConditionFlags = ModelConditionFlags::RIDER4;
pub const MODELCONDITION_RIDER5: ModelConditionFlags = ModelConditionFlags::RIDER5;
pub const MODELCONDITION_RIDER6: ModelConditionFlags = ModelConditionFlags::RIDER6;
pub const MODELCONDITION_RIDER7: ModelConditionFlags = ModelConditionFlags::RIDER7;
pub const MODELCONDITION_RIDER8: ModelConditionFlags = ModelConditionFlags::RIDER8;
pub const MODELCONDITION_DOCKING: ModelConditionFlags = ModelConditionFlags::DOCKING;
pub const MODELCONDITION_DOCKING_BEGINNING: ModelConditionFlags =
    ModelConditionFlags::DOCKING_BEGINNING;
pub const MODELCONDITION_DOCKING_ACTIVE: ModelConditionFlags = ModelConditionFlags::DOCKING_ACTIVE;
pub const MODELCONDITION_DOCKING_ENDING: ModelConditionFlags = ModelConditionFlags::DOCKING_ENDING;
pub const MODELCONDITION_PREORDER: ModelConditionFlags = ModelConditionFlags::PREORDER;
