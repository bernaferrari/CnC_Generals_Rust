//! HelicopterSlowDeathBehavior - Rust conversion of C++ HelicopterSlowDeathBehavior
//!
//! Specialized slow death for helicopters with spiral crash.
//! Author: Colin Day, March 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::audio::AudioEventRts;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, ModuleData, ObjectID, Real, UnsignedInt,
    LOGICFRAMES_PER_SECOND, MODELCONDITION_SPECIAL_DAMAGED,
};
use crate::damage::DamageInfo;
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{TheFXListStore, TheGameLogic, TheObjectCreationListStore, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, SlowDeathBehaviorInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::slow_death_behavior::{
    self, parse_death_types, parse_destruction_altitude, parse_destruction_delay,
    parse_destruction_delay_variance, parse_exempt_status, parse_fling_force,
    parse_fling_force_variance, parse_fling_pitch, parse_fling_pitch_variance, parse_fx, parse_ocl,
    parse_probability_modifier, parse_required_status, parse_sink_delay, parse_sink_delay_variance,
    parse_sink_rate, parse_veterancy_levels, parse_weapon, SlowDeathBehaviorModuleData,
};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct HelicopterSlowDeathBehaviorModuleData {
    pub base: SlowDeathBehaviorModuleData,
    pub spiral_orbit_turn_rate: Real,
    pub spiral_orbit_forward_speed: Real,
    pub spiral_orbit_forward_speed_damping: Real,
    pub min_self_spin: Real,
    pub max_self_spin: Real,
    pub self_spin_update_delay: Real,
    pub self_spin_update_amount: Real,
    pub fall_how_fast: Real,
    pub min_blade_fly_off_delay: Real,
    pub max_blade_fly_off_delay: Real,
    pub attach_particle_system: Option<String>,
    pub attach_particle_bone: String,
    pub attach_particle_loc: Coord3D,
    pub blade_object_name: String,
    pub blade_bone: String,
    pub ocl_eject_pilot: Option<Arc<ObjectCreationList>>,
    pub fx_blade: Option<Arc<FXList>>,
    pub ocl_blade: Option<Arc<ObjectCreationList>>,
    pub fx_hit_ground: Option<Arc<FXList>>,
    pub ocl_hit_ground: Option<Arc<ObjectCreationList>>,
    pub fx_final_blow_up: Option<Arc<FXList>>,
    pub ocl_final_blow_up: Option<Arc<ObjectCreationList>>,
    pub delay_from_ground_to_final_death: Real,
    pub final_rubble_object: String,
    pub max_braking: Real,
    pub death_sound: AudioEventRts,
}

impl Default for HelicopterSlowDeathBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: SlowDeathBehaviorModuleData::new(),
            spiral_orbit_turn_rate: 0.0,
            spiral_orbit_forward_speed: 0.0,
            spiral_orbit_forward_speed_damping: 1.0,
            min_self_spin: 0.0,
            max_self_spin: 0.0,
            self_spin_update_delay: 0.0,
            self_spin_update_amount: 0.0,
            fall_how_fast: 0.0,
            min_blade_fly_off_delay: 0.0,
            max_blade_fly_off_delay: 0.0,
            attach_particle_system: None,
            attach_particle_bone: String::new(),
            attach_particle_loc: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            blade_object_name: String::new(),
            blade_bone: String::new(),
            ocl_eject_pilot: None,
            fx_blade: None,
            ocl_blade: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            fx_final_blow_up: None,
            ocl_final_blow_up: None,
            delay_from_ground_to_final_death: 0.0,
            final_rubble_object: String::new(),
            max_braking: 99999.0,
            death_sound: AudioEventRts::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HelicopterSlowDeathBehaviorModuleData, base);

impl HelicopterSlowDeathBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HELICOPTER_SLOW_DEATH_FIELDS)
    }
}

fn token<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens.first().copied().ok_or(INIError::InvalidData)
}

macro_rules! inherited_slow_death_field {
    ($name:ident, $parser:path) => {
        fn $name(
            ini: &mut INI,
            data: &mut HelicopterSlowDeathBehaviorModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            $parser(ini, &mut data.base, tokens)
        }
    };
}

inherited_slow_death_field!(parse_base_sink_rate, parse_sink_rate);
inherited_slow_death_field!(parse_base_probability_modifier, parse_probability_modifier);
inherited_slow_death_field!(
    parse_base_modifier_bonus_per_overkill_percent,
    slow_death_behavior::parse_modifier_bonus_per_overkill_percent
);
inherited_slow_death_field!(parse_base_sink_delay, parse_sink_delay);
inherited_slow_death_field!(parse_base_sink_delay_variance, parse_sink_delay_variance);
inherited_slow_death_field!(parse_base_destruction_delay, parse_destruction_delay);
inherited_slow_death_field!(
    parse_base_destruction_delay_variance,
    parse_destruction_delay_variance
);
inherited_slow_death_field!(parse_base_destruction_altitude, parse_destruction_altitude);
inherited_slow_death_field!(parse_base_fx, parse_fx);
inherited_slow_death_field!(parse_base_ocl, parse_ocl);
inherited_slow_death_field!(parse_base_weapon, parse_weapon);
inherited_slow_death_field!(parse_base_fling_force, parse_fling_force);
inherited_slow_death_field!(parse_base_fling_force_variance, parse_fling_force_variance);
inherited_slow_death_field!(parse_base_fling_pitch, parse_fling_pitch);
inherited_slow_death_field!(parse_base_fling_pitch_variance, parse_fling_pitch_variance);
inherited_slow_death_field!(parse_base_death_types, parse_death_types);
inherited_slow_death_field!(parse_base_veterancy_levels, parse_veterancy_levels);
inherited_slow_death_field!(parse_base_exempt_status, parse_exempt_status);
inherited_slow_death_field!(parse_base_required_status, parse_required_status);

fn parse_spiral_orbit_turn_rate(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spiral_orbit_turn_rate = INI::parse_angular_velocity_real(token(tokens)?)?;
    Ok(())
}

fn parse_spiral_orbit_forward_speed(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spiral_orbit_forward_speed = INI::parse_velocity_real(token(tokens)?)?;
    Ok(())
}

fn parse_spiral_orbit_forward_speed_damping(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spiral_orbit_forward_speed_damping = INI::parse_real(token(tokens)?)?;
    Ok(())
}

fn parse_min_self_spin(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_self_spin = INI::parse_angular_velocity_real(token(tokens)?)?;
    Ok(())
}

fn parse_max_self_spin(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_self_spin = INI::parse_angular_velocity_real(token(tokens)?)?;
    Ok(())
}

fn parse_self_spin_update_delay(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.self_spin_update_delay = INI::parse_duration_real(token(tokens)?)?;
    Ok(())
}

fn parse_self_spin_update_amount(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.self_spin_update_amount = INI::parse_angle_real(token(tokens)?)?;
    Ok(())
}

fn parse_fall_how_fast(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.fall_how_fast = INI::parse_percent_to_real(token(tokens)?)?;
    Ok(())
}

fn parse_min_blade_fly_off_delay(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_blade_fly_off_delay = INI::parse_duration_real(token(tokens)?)?;
    Ok(())
}

fn parse_max_blade_fly_off_delay(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_blade_fly_off_delay = INI::parse_duration_real(token(tokens)?)?;
    Ok(())
}

fn parse_attach_particle(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attach_particle_system = Some(token(tokens)?.to_string());
    Ok(())
}

fn parse_attach_particle_bone(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attach_particle_bone = token(tokens)?.to_string();
    Ok(())
}

fn parse_attach_particle_loc(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let (x, y, z) = INI::parse_coord_3d(tokens)?;
    data.attach_particle_loc = Coord3D { x, y, z };
    Ok(())
}

fn parse_blade_object_name(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.blade_object_name = token(tokens)?.to_string();
    Ok(())
}

fn parse_blade_bone_name(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.blade_bone = token(tokens)?.to_string();
    Ok(())
}

fn parse_fx_slot(target: &mut Option<Arc<FXList>>, tokens: &[&str]) -> Result<(), INIError> {
    let name = token(tokens)?;
    *target = TheFXListStore::find_fx_list(name).or_else(|| {
        if name.eq_ignore_ascii_case("None") {
            None
        } else {
            Some(TheFXListStore::ensure_fx_list(name))
        }
    });
    Ok(())
}

fn parse_ocl_slot(
    target: &mut Option<Arc<ObjectCreationList>>,
    tokens: &[&str],
) -> Result<(), INIError> {
    *target = TheObjectCreationListStore::find_object_creation_list(token(tokens)?);
    Ok(())
}

fn parse_ocl_eject_pilot(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_eject_pilot, tokens)
}

fn parse_fx_blade(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_blade, tokens)
}

fn parse_ocl_blade(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_blade, tokens)
}

fn parse_fx_hit_ground(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_hit_ground, tokens)
}

fn parse_ocl_hit_ground(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_hit_ground, tokens)
}

fn parse_fx_final_blow_up(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_final_blow_up, tokens)
}

fn parse_ocl_final_blow_up(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_final_blow_up, tokens)
}

fn parse_delay_from_ground_to_final_death(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.delay_from_ground_to_final_death = INI::parse_duration_real(token(tokens)?)?;
    Ok(())
}

fn parse_final_rubble_object(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.final_rubble_object = token(tokens)?.to_string();
    Ok(())
}

fn parse_sound_death_loop(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.death_sound = AudioEventRts::new(token(tokens)?);
    Ok(())
}

fn parse_max_braking(
    _ini: &mut INI,
    data: &mut HelicopterSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_braking = INI::parse_real(token(tokens)?)?;
    Ok(())
}

const HELICOPTER_SLOW_DEATH_FIELDS: &[FieldParse<HelicopterSlowDeathBehaviorModuleData>] = &[
    FieldParse {
        token: "SinkRate",
        parse: parse_base_sink_rate,
    },
    FieldParse {
        token: "ProbabilityModifier",
        parse: parse_base_probability_modifier,
    },
    FieldParse {
        token: "ModifierBonusPerOverkillPercent",
        parse: parse_base_modifier_bonus_per_overkill_percent,
    },
    FieldParse {
        token: "SinkDelay",
        parse: parse_base_sink_delay,
    },
    FieldParse {
        token: "SinkDelayVariance",
        parse: parse_base_sink_delay_variance,
    },
    FieldParse {
        token: "DestructionDelay",
        parse: parse_base_destruction_delay,
    },
    FieldParse {
        token: "DestructionDelayVariance",
        parse: parse_base_destruction_delay_variance,
    },
    FieldParse {
        token: "DestructionAltitude",
        parse: parse_base_destruction_altitude,
    },
    FieldParse {
        token: "FX",
        parse: parse_base_fx,
    },
    FieldParse {
        token: "OCL",
        parse: parse_base_ocl,
    },
    FieldParse {
        token: "Weapon",
        parse: parse_base_weapon,
    },
    FieldParse {
        token: "FlingForce",
        parse: parse_base_fling_force,
    },
    FieldParse {
        token: "FlingForceVariance",
        parse: parse_base_fling_force_variance,
    },
    FieldParse {
        token: "FlingPitch",
        parse: parse_base_fling_pitch,
    },
    FieldParse {
        token: "FlingPitchVariance",
        parse: parse_base_fling_pitch_variance,
    },
    FieldParse {
        token: "DeathTypes",
        parse: parse_base_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_base_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_base_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_base_required_status,
    },
    FieldParse {
        token: "SpiralOrbitTurnRate",
        parse: parse_spiral_orbit_turn_rate,
    },
    FieldParse {
        token: "SpiralOrbitForwardSpeed",
        parse: parse_spiral_orbit_forward_speed,
    },
    FieldParse {
        token: "SpiralOrbitForwardSpeedDamping",
        parse: parse_spiral_orbit_forward_speed_damping,
    },
    FieldParse {
        token: "MinSelfSpin",
        parse: parse_min_self_spin,
    },
    FieldParse {
        token: "MaxSelfSpin",
        parse: parse_max_self_spin,
    },
    FieldParse {
        token: "SelfSpinUpdateDelay",
        parse: parse_self_spin_update_delay,
    },
    FieldParse {
        token: "SelfSpinUpdateAmount",
        parse: parse_self_spin_update_amount,
    },
    FieldParse {
        token: "FallHowFast",
        parse: parse_fall_how_fast,
    },
    FieldParse {
        token: "MinBladeFlyOffDelay",
        parse: parse_min_blade_fly_off_delay,
    },
    FieldParse {
        token: "MaxBladeFlyOffDelay",
        parse: parse_max_blade_fly_off_delay,
    },
    FieldParse {
        token: "AttachParticle",
        parse: parse_attach_particle,
    },
    FieldParse {
        token: "AttachParticleBone",
        parse: parse_attach_particle_bone,
    },
    FieldParse {
        token: "AttachParticleLoc",
        parse: parse_attach_particle_loc,
    },
    FieldParse {
        token: "BladeObjectName",
        parse: parse_blade_object_name,
    },
    FieldParse {
        token: "BladeBoneName",
        parse: parse_blade_bone_name,
    },
    FieldParse {
        token: "OCLEjectPilot",
        parse: parse_ocl_eject_pilot,
    },
    FieldParse {
        token: "FXBlade",
        parse: parse_fx_blade,
    },
    FieldParse {
        token: "OCLBlade",
        parse: parse_ocl_blade,
    },
    FieldParse {
        token: "FXHitGround",
        parse: parse_fx_hit_ground,
    },
    FieldParse {
        token: "OCLHitGround",
        parse: parse_ocl_hit_ground,
    },
    FieldParse {
        token: "FXFinalBlowUp",
        parse: parse_fx_final_blow_up,
    },
    FieldParse {
        token: "OCLFinalBlowUp",
        parse: parse_ocl_final_blow_up,
    },
    FieldParse {
        token: "DelayFromGroundToFinalDeath",
        parse: parse_delay_from_ground_to_final_death,
    },
    FieldParse {
        token: "FinalRubbleObject",
        parse: parse_final_rubble_object,
    },
    FieldParse {
        token: "SoundDeathLoop",
        parse: parse_sound_death_loop,
    },
    FieldParse {
        token: "MaxBraking",
        parse: parse_max_braking,
    },
];

pub struct HelicopterSlowDeathBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<HelicopterSlowDeathBehaviorModuleData>,
    orbit_direction: Int,
    forward_angle: Real,
    forward_speed: Real,
    self_spin: Real,
    self_spin_towards_max: Bool,
    last_self_spin_update_frame: UnsignedInt,
    blade_fly_off_frame: UnsignedInt,
    hit_ground_frame: UnsignedInt,
    active: Bool,
}

impl HelicopterSlowDeathBehavior {
    /// Create new helicopter slow death behavior
    /// Matches C++ HelicopterSlowDeathBehavior constructor at line 136
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<HelicopterSlowDeathBehaviorModuleData>,
    ) -> Self {
        // Get current frame from game logic (matches C++ line 145-146)
        let current_frame = TheGameLogic::get_frame();

        // Calculate random blade fly off delay (matches C++ lines 185-186)
        let blade_delay = crate::helpers::get_game_logic_random_value_real(
            module_data.min_blade_fly_off_delay,
            module_data.max_blade_fly_off_delay,
        );

        Self {
            object: Arc::downgrade(&object),
            module_data,
            orbit_direction: 1,             // C++ line 140: ORBIT_DIRECTION_LEFT
            forward_angle: 0.0,             // C++ line 141
            forward_speed: 0.0,             // C++ line 142, set in beginSlowDeath
            self_spin: 0.0,                 // C++ line 143, set in beginSlowDeath
            self_spin_towards_max: false,   // C++ line 144
            last_self_spin_update_frame: 0, // C++ line 145
            blade_fly_off_frame: current_frame + blade_delay as UnsignedInt, // C++ line 146
            hit_ground_frame: 0,            // C++ line 147
            active: false,
        }
    }

    /// Begin the slow death sequence
    /// Matches C++ HelicopterSlowDeathBehavior::beginSlowDeath at line 160
    pub fn begin_slow_death(&mut self) {
        self.active = true;

        // In C++ lines 163-164: SlowDeathBehavior::beginSlowDeath(damageInfo);
        // Call base class functionality (not shown here)

        // In C++ lines 167-170: Stop ambient sound
        // if (getObject()->getDrawable()) {
        //     getObject()->getDrawable()->stopAmbientSound();
        // }
        log::debug!("HelicopterSlowDeathBehavior: Would stop ambient sound");

        // In C++ lines 175-181: Set death sound
        // m_deathSound = modData->m_deathSound;
        // if (!m_deathSound.getEventName().isEmpty()) {
        //     m_deathSound.setObjectID(getObject()->getID());
        //     m_deathSound.setPlayingHandle(TheAudio->addAudioEvent(&m_deathSound));
        // }
        log::debug!("HelicopterSlowDeathBehavior: Would start death sound");

        // Pick orbit direction (C++ line 189: always left for now)
        self.orbit_direction = 1; // ORBIT_DIRECTION_LEFT

        // Record current forward angle (C++ line 195)
        // m_forwardAngle = getObject()->getOrientation();
        // Note: This would need object access
        log::trace!("HelicopterSlowDeathBehavior: Would set forward angle from object orientation");

        // Set forward speed (C++ line 198)
        self.forward_speed = self.module_data.spiral_orbit_forward_speed;

        // Start self spin at minimum (C++ line 201)
        self.self_spin = self.module_data.min_self_spin;

        // We will start changing spin towards max (C++ line 204)
        self.self_spin_towards_max = true;

        // In C++ lines 207-213: Set locomotor lift and braking
        // Locomotor *locomotor = getObject()->getAIUpdateInterface()->getCurLocomotor();
        // locomotor->setMaxLift(-TheGlobalData->m_gravity * (1.0f - modData->m_fallHowFast));
        // locomotor->setMaxBraking(modData->m_maxBraking);
        log::debug!("HelicopterSlowDeathBehavior: Would configure locomotor for fall");

        // In C++ lines 216-250: Attach particle system to bone if present
        // if (modData->m_attachParticleSystem) {
        //     ParticleSystem *pSys = TheParticleSystemManager->createParticleSystem(...);
        //     pSys->attachToObject(getObject());
        // }
        log::debug!("HelicopterSlowDeathBehavior: Would attach particle effects");
    }
}

impl UpdateModuleInterface for HelicopterSlowDeathBehavior {
    /// Update the helicopter slow death behavior each frame
    /// Matches C++ HelicopterSlowDeathBehavior::update at line 261
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Return early if not activated (C++ lines 268-269)
        if !self.active {
            return UpdateSleepTime::Forever;
        }

        // Get current frame (C++ uses TheGameLogic->getFrame())
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Update self spin and orbit if we haven't hit ground yet (C++ lines 278-396)
        if self.hit_ground_frame == 0 {
            // In C++ lines 285-290: Rotate object based on self spin
            // Matrix3D xfrm = *copter->getTransformMatrix();
            // xfrm.In_Place_Pre_Rotate_Z(m_selfSpin * m_orbitDirection);
            // copter->setTransformMatrix(&xfrm);
            log::trace!("Would rotate helicopter by self spin");

            // Update self spin rate over time (C++ lines 296-333)
            if self.module_data.self_spin_update_delay > 0.0
                && current_frame
                    >= self.last_self_spin_update_frame
                        + self.module_data.self_spin_update_delay as UnsignedInt
            {
                if self.self_spin_towards_max {
                    // Going towards max (C++ lines 301-312)
                    self.self_spin +=
                        self.module_data.self_spin_update_amount / LOGICFRAMES_PER_SECOND as Real;
                    if self.self_spin >= self.module_data.max_self_spin {
                        self.self_spin = self.module_data.max_self_spin;
                        self.self_spin_towards_max = false;
                    }
                } else {
                    // Going towards min (C++ lines 315-326)
                    self.self_spin -=
                        self.module_data.self_spin_update_amount / LOGICFRAMES_PER_SECOND as Real;
                    if self.self_spin <= self.module_data.min_self_spin {
                        self.self_spin = self.module_data.min_self_spin;
                        self.self_spin_towards_max = true;
                    }
                }
                self.last_self_spin_update_frame = current_frame;
            }

            // In C++ lines 336-349: Apply physics force for spiral motion
            // PhysicsBehavior *physics = copter->getPhysics();
            // Coord3D force;
            // force.x = Cos(m_forwardAngle) * m_forwardSpeed;
            // force.y = Sin(m_forwardAngle) * m_forwardSpeed;
            // force.z = 0.0f;
            // physics->applyMotiveForce(&force);
            log::trace!("Would apply spiral motion force");

            // Update forward angle (C++ line 352)
            self.forward_angle +=
                self.module_data.spiral_orbit_turn_rate * self.orbit_direction as Real;

            // Apply damping to forward speed (C++ line 355)
            self.forward_speed *= self.module_data.spiral_orbit_forward_speed_damping;

            // Check if it's time for blade to fly off (C++ lines 358-393)
            if self.blade_fly_off_frame > 0 {
                self.blade_fly_off_frame = self.blade_fly_off_frame.saturating_sub(1);
                if self.blade_fly_off_frame == 0 {
                    // In C++ lines 364-382: Get blade position from bone and create blade
                    // Drawable *draw = copter->getDrawable();
                    // draw->getPristineBonePositions(modData->m_bladeBone.str(), ...);
                    // FXList::doFXPos(modData->m_fxBlade, &bladePos);
                    // ObjectCreationList::create(modData->m_oclBlade, copter, &bladePos, ...);
                    log::debug!("Would spawn blade object and play FX");

                    // In C++ lines 389-390: Eject pilot if veteran or better
                    // if (modData->m_oclEjectPilot && copter->getVeterancyLevel() > LEVEL_REGULAR)
                    //     EjectPilotDie::ejectPilot(modData->m_oclEjectPilot, copter, NULL);
                    log::debug!("Would eject pilot if veteran");
                }
            }
        }

        // In C++ lines 400-412: Check collision with trees
        // PhysicsBehavior *phys = copter->getPhysics();
        // ObjectID treeID = phys->getLastCollidee();
        // Object *tree = TheGameLogic->findObjectByID(treeID);
        // if (tree && tree->isKindOf(KINDOF_SHRUBBERY))
        //     hitATree = TRUE;
        log::trace!("Would check tree collision");

        // In C++ lines 417-450: Check if hit ground
        if self.hit_ground_frame == 0 {
            if let Some(object) = self.object.upgrade() {
                if let Ok(mut guard) = object.write() {
                    let pos = *guard.get_position();
                    let ground = TheTerrainLogic::get()
                        .map(|terrain| terrain.get_ground_height(pos.x, pos.y, None))
                        .unwrap_or(pos.z);

                    if pos.z <= ground + 1.0 {
                        self.hit_ground_frame = current_frame;
                        let _ = guard.set_disabled_held(true);
                        guard.set_model_condition_state(MODELCONDITION_SPECIAL_DAMAGED);
                    }
                }
            }
        }

        // Check if time for final explosion (C++ lines 453-474)
        if self.hit_ground_frame > 0
            && current_frame - self.hit_ground_frame
                > self.module_data.delay_from_ground_to_final_death as UnsignedInt
        {
            // In C++:
            // FXList::doFXObj(modData->m_fxFinalBlowUp, copter);
            // ObjectCreationList::create(modData->m_oclFinalBlowUp, copter, NULL);
            // const ThingTemplate* ttn = TheThingFactory->findTemplate(modData->m_finalRubbleObject);
            // Object *rubble = TheThingFactory->newObject(ttn, copter->getTeam());
            // rubble->setTransformMatrix(copter->getTransformMatrix());
            // TheGameLogic->destroyObject(copter);
            log::debug!("Would create final explosion, spawn rubble, and destroy helicopter");
        }

        // Always update every frame (C++ line 476: return UPDATE_SLEEP_NONE)
        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for HelicopterSlowDeathBehavior {
    fn get_module_name(&self) -> &'static str {
        "HelicopterSlowDeathBehavior"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl SlowDeathBehaviorInterface for HelicopterSlowDeathBehavior {
    fn is_slow_death_active(&self) -> bool {
        self.active
    }

    fn begin_slow_death(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.begin_slow_death();
        Ok(())
    }

    fn get_probability_modifier(&self, _damage_info: &DamageInfo) -> Int {
        1
    }

    fn is_die_applicable(&self, _damage_info: &DamageInfo) -> bool {
        true
    }

    fn get_slow_death_phase(&self) -> u32 {
        if self.hit_ground_frame == 0 {
            0
        } else {
            2
        }
    }
}

impl DieModuleInterface for HelicopterSlowDeathBehavior {
    fn on_die(
        &mut self,
        damage: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        SlowDeathBehaviorInterface::begin_slow_death(self, damage)
    }
}

impl Snapshotable for HelicopterSlowDeathBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_int(&mut self.orbit_direction)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer orbit_direction: {:?}", e))?;
        xfer.xfer_real(&mut self.forward_angle)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer forward_angle: {:?}", e))?;
        xfer.xfer_real(&mut self.forward_speed)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer forward_speed: {:?}", e))?;
        xfer.xfer_real(&mut self.self_spin)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer self_spin: {:?}", e))?;
        xfer.xfer_bool(&mut self.self_spin_towards_max)
            .map_err(|e| {
                format!(
                    "HelicopterSlowDeathBehavior xfer self_spin_towards_max: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.last_self_spin_update_frame)
            .map_err(|e| {
                format!(
                    "HelicopterSlowDeathBehavior xfer last_self_spin_update_frame: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.blade_fly_off_frame)
            .map_err(|e| {
                format!(
                    "HelicopterSlowDeathBehavior xfer blade_fly_off_frame: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.hit_ground_frame)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer hit_ground_frame: {:?}", e))?;
        xfer.xfer_bool(&mut self.active)
            .map_err(|e| format!("HelicopterSlowDeathBehavior xfer active: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct HelicopterSlowDeathBehaviorModule {
    behavior: HelicopterSlowDeathBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<HelicopterSlowDeathBehaviorModuleData>,
}

impl HelicopterSlowDeathBehaviorModule {
    pub fn new(
        behavior: HelicopterSlowDeathBehavior,
        module_name: &AsciiString,
        module_data: Arc<HelicopterSlowDeathBehaviorModuleData>,
    ) -> Self {
        Self {
            behavior,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            module_data,
        }
    }
}

impl Snapshotable for HelicopterSlowDeathBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl EngineModule for HelicopterSlowDeathBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

impl BehaviorModuleInterface for HelicopterSlowDeathBehaviorModule {
    fn get_module_name(&self) -> &'static str {
        "HelicopterSlowDeathBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        self.behavior.get_update()
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(&mut self.behavior)
    }

    fn get_slow_death_behavior_interface(&mut self) -> Option<&mut dyn SlowDeathBehaviorInterface> {
        Some(&mut self.behavior)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helicopter_slow_death_defaults_match_cpp_constructor() {
        let data = HelicopterSlowDeathBehaviorModuleData::default();

        assert_eq!(data.spiral_orbit_turn_rate, 0.0);
        assert_eq!(data.spiral_orbit_forward_speed, 0.0);
        assert_eq!(data.spiral_orbit_forward_speed_damping, 1.0);
        assert_eq!(data.min_self_spin, 0.0);
        assert_eq!(data.max_self_spin, 0.0);
        assert_eq!(data.self_spin_update_delay, 0.0);
        assert_eq!(data.self_spin_update_amount, 0.0);
        assert_eq!(data.fall_how_fast, 0.0);
        assert_eq!(data.min_blade_fly_off_delay, 0.0);
        assert_eq!(data.max_blade_fly_off_delay, 0.0);
        assert_eq!(data.attach_particle_system, None);
        assert_eq!(data.attach_particle_bone, "");
        assert_eq!(data.blade_bone, "");
        assert_eq!(data.blade_object_name, "");
        assert_eq!(data.final_rubble_object, "");
        assert_eq!(data.delay_from_ground_to_final_death, 0.0);
        assert_eq!(data.max_braking, 99999.0);
        assert_eq!(data.death_sound.get_event_name(), "");
    }

    #[test]
    fn helicopter_slow_death_parse_from_ini_preserves_cpp_fields() {
        let mut data = HelicopterSlowDeathBehaviorModuleData::default();
        let mut ini = INI::new();

        ini.with_inline_source(
            "SinkRate = 0.33\n\
             ProbabilityModifier = 7\n\
             SpiralOrbitTurnRate = 0.11\n\
             SpiralOrbitForwardSpeed = 9.5\n\
             SpiralOrbitForwardSpeedDamping = 0.8\n\
             MinSelfSpin = 0.02\n\
             MaxSelfSpin = 0.4\n\
             SelfSpinUpdateDelay = 1s\n\
             SelfSpinUpdateAmount = 0.15\n\
             FallHowFast = 80%\n\
             MinBladeFlyOffDelay = 500ms\n\
             MaxBladeFlyOffDelay = 2s\n\
             AttachParticle = SmokeTrail\n\
             AttachParticleBone = Rotor\n\
             AttachParticleLoc = X:1 Y:2 Z:3\n\
             BladeObjectName = TestBlade\n\
             BladeBoneName = MainRotor\n\
             FXBlade = TestBladeFX\n\
             FXHitGround = TestHitFX\n\
             FXFinalBlowUp = TestFinalFX\n\
             DelayFromGroundToFinalDeath = 1500ms\n\
             FinalRubbleObject = TestRubble\n\
             SoundDeathLoop = TestLoopSound\n\
             MaxBraking = 123.5\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("helicopter slow death ini parses");

        assert!((data.base.sink_rate - 0.33).abs() < f32::EPSILON);
        assert_eq!(data.base.probability_modifier, 7);
        assert!(
            (data.spiral_orbit_turn_rate - (0.11_f32.to_radians() / 30.0)).abs() < f32::EPSILON
        );
        assert!((data.spiral_orbit_forward_speed - (9.5 / 30.0)).abs() < f32::EPSILON);
        assert!((data.spiral_orbit_forward_speed_damping - 0.8).abs() < f32::EPSILON);
        assert!((data.min_self_spin - (0.02_f32.to_radians() / 30.0)).abs() < f32::EPSILON);
        assert!((data.max_self_spin - (0.4_f32.to_radians() / 30.0)).abs() < f32::EPSILON);
        assert!((data.self_spin_update_delay - 30.0).abs() < 0.001);
        assert!((data.self_spin_update_amount - 0.15_f32.to_radians()).abs() < f32::EPSILON);
        assert!((data.fall_how_fast - 0.8).abs() < f32::EPSILON);
        assert!((data.min_blade_fly_off_delay - 15.0).abs() < 0.001);
        assert!((data.max_blade_fly_off_delay - 60.0).abs() < 0.001);
        assert_eq!(data.attach_particle_system.as_deref(), Some("SmokeTrail"));
        assert_eq!(data.attach_particle_bone, "Rotor");
        assert_eq!(data.attach_particle_loc, Coord3D::new(1.0, 2.0, 3.0));
        assert_eq!(data.blade_object_name, "TestBlade");
        assert_eq!(data.blade_bone, "MainRotor");
        assert!(data.fx_blade.is_some());
        assert!(data.fx_hit_ground.is_some());
        assert!(data.fx_final_blow_up.is_some());
        assert!((data.delay_from_ground_to_final_death - 45.0).abs() < 0.001);
        assert_eq!(data.final_rubble_object, "TestRubble");
        assert_eq!(data.death_sound.get_event_name(), "TestLoopSound");
        assert!((data.max_braking - 123.5).abs() < f32::EPSILON);
    }

    #[test]
    fn helicopter_slow_death_rejects_missing_cpp_field_value() {
        let mut data = HelicopterSlowDeathBehaviorModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("SpiralOrbitTurnRate =\nEnd\n", |ini| {
                data.parse_from_ini(ini)
            })
            .expect_err("missing value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.spiral_orbit_turn_rate, 0.0);
    }
}

pub struct HelicopterSlowDeathBehaviorFactory;
impl HelicopterSlowDeathBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let typed = module_data
            .as_ref()
            .downcast_ref::<HelicopterSlowDeathBehaviorModuleData>()
            .cloned()
            .unwrap_or_else(|| {
                warn!("HelicopterSlowDeathBehavior legacy factory data expected; using defaults");
                HelicopterSlowDeathBehaviorModuleData::default()
            });
        Ok(Box::new(HelicopterSlowDeathBehavior::new(
            thing,
            Arc::new(typed),
        )))
    }
}
