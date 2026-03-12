//! HelicopterSlowDeathBehavior - Rust conversion of C++ HelicopterSlowDeathBehavior
//!
//! Specialized slow death for helicopters with spiral crash.
//! Author: Colin Day, March 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    Bool, Coord3D, Int, ModuleData, Real, UnsignedInt, LOGICFRAMES_PER_SECOND,
    MODELCONDITION_SPECIAL_DAMAGED,
};
use crate::helpers::TheTerrainLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct HelicopterSlowDeathBehaviorModuleData {
    pub base: BehaviorModuleData,
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
    pub attach_particle_bone: String,
    pub attach_particle_loc: Coord3D,
    pub blade_object_name: String,
    pub blade_bone: String,
    pub delay_from_ground_to_final_death: Real,
    pub final_rubble_object: String,
    pub max_braking: Real,
}

impl Default for HelicopterSlowDeathBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            spiral_orbit_turn_rate: 0.1,
            spiral_orbit_forward_speed: 10.0,
            spiral_orbit_forward_speed_damping: 0.95,
            min_self_spin: 0.05,
            max_self_spin: 0.5,
            self_spin_update_delay: 10.0,
            self_spin_update_amount: 0.1,
            fall_how_fast: 0.8,
            min_blade_fly_off_delay: 30.0,
            max_blade_fly_off_delay: 90.0,
            attach_particle_bone: String::new(),
            attach_particle_loc: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            blade_object_name: String::new(),
            blade_bone: String::new(),
            delay_from_ground_to_final_death: 30.0,
            final_rubble_object: String::new(),
            max_braking: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(HelicopterSlowDeathBehaviorModuleData, base);

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
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<HelicopterSlowDeathBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        // Get current frame from game logic (matches C++ line 145-146)
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Calculate random blade fly off delay (matches C++ lines 185-186)
        let blade_delay = crate::helpers::get_game_logic_random_value_real(
            specific_data.min_blade_fly_off_delay,
            specific_data.max_blade_fly_off_delay,
        );

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            orbit_direction: 1,             // C++ line 140: ORBIT_DIRECTION_LEFT
            forward_angle: 0.0,             // C++ line 141
            forward_speed: 0.0,             // C++ line 142, set in beginSlowDeath
            self_spin: 0.0,                 // C++ line 143, set in beginSlowDeath
            self_spin_towards_max: false,   // C++ line 144
            last_self_spin_update_frame: 0, // C++ line 145
            blade_fly_off_frame: current_frame + blade_delay as UnsignedInt, // C++ line 146
            hit_ground_frame: 0,            // C++ line 147
            active: false,
        })
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

pub struct HelicopterSlowDeathBehaviorFactory;
impl HelicopterSlowDeathBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HelicopterSlowDeathBehavior::new(
            thing,
            module_data,
        )?))
    }
}
