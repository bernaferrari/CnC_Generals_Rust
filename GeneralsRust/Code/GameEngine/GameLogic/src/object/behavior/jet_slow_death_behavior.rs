//! JetSlowDeathBehavior - Rust conversion of C++ JetSlowDeathBehavior.
//!
//! Handles the multi-stage airborne jet death sequence: initial effects,
//! falling/rolling, ground impact, and final blow-up.

use crate::common::audio::AudioEventRts;
use crate::common::{
    xfer::XferExt, AsciiString, Bool, KindOf, ModuleData, ObjectID, ObjectStatusMaskType,
    ObjectStatusTypes, PathfindLayerEnum, Real, TheGameLogic, UnsignedInt, XferVersion, INVALID_ID,
};
use crate::damage::DamageInfo;
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{TheAudio, TheFXListStore, TheObjectCreationListStore, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, SlowDeathBehaviorInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType,
};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct JetSlowDeathBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub fx_on_ground_death: Option<Arc<FXList>>,
    pub ocl_on_ground_death: Option<Arc<ObjectCreationList>>,
    pub fx_initial_death: Option<Arc<FXList>>,
    pub ocl_initial_death: Option<Arc<ObjectCreationList>>,
    pub delay_secondary_from_initial_death: UnsignedInt,
    pub fx_secondary: Option<Arc<FXList>>,
    pub ocl_secondary: Option<Arc<ObjectCreationList>>,
    pub fx_hit_ground: Option<Arc<FXList>>,
    pub ocl_hit_ground: Option<Arc<ObjectCreationList>>,
    pub delay_final_blow_up_from_hit_ground: UnsignedInt,
    pub fx_final_blow_up: Option<Arc<FXList>>,
    pub ocl_final_blow_up: Option<Arc<ObjectCreationList>>,
    pub death_loop_sound: AudioEventRts,
    pub roll_rate: Real,
    pub roll_rate_delta: Real,
    pub pitch_rate: Real,
    pub fall_how_fast: Real,
}

impl Default for JetSlowDeathBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            fx_on_ground_death: None,
            ocl_on_ground_death: None,
            fx_initial_death: None,
            ocl_initial_death: None,
            delay_secondary_from_initial_death: 0,
            fx_secondary: None,
            ocl_secondary: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            delay_final_blow_up_from_hit_ground: 0,
            fx_final_blow_up: None,
            ocl_final_blow_up: None,
            death_loop_sound: AudioEventRts::new(""),
            roll_rate: 0.0,
            roll_rate_delta: 1.0,
            pitch_rate: 0.0,
            fall_how_fast: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(JetSlowDeathBehaviorModuleData, base);

impl JetSlowDeathBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, JET_SLOW_DEATH_FIELDS)
    }
}

fn token<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens.first().copied().ok_or(INIError::InvalidData)
}

fn parse_fx_slot(data: &mut Option<Arc<FXList>>, tokens: &[&str]) -> Result<(), INIError> {
    let name = token(tokens)?;
    *data = TheFXListStore::find_fx_list(name).or_else(|| {
        if name.eq_ignore_ascii_case("None") {
            None
        } else {
            Some(TheFXListStore::ensure_fx_list(name))
        }
    });
    Ok(())
}

fn parse_ocl_slot(
    data: &mut Option<Arc<ObjectCreationList>>,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = token(tokens)?;
    *data = TheObjectCreationListStore::find_object_creation_list(name);
    Ok(())
}

fn parse_fx_on_ground_death(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_on_ground_death, tokens)
}

fn parse_ocl_on_ground_death(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_on_ground_death, tokens)
}

fn parse_fx_initial_death(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_initial_death, tokens)
}

fn parse_ocl_initial_death(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_initial_death, tokens)
}

fn parse_delay_secondary(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.delay_secondary_from_initial_death = INI::parse_duration_unsigned_int(token(tokens)?)?;
    Ok(())
}

fn parse_fx_secondary(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_secondary, tokens)
}

fn parse_ocl_secondary(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_secondary, tokens)
}

fn parse_fx_hit_ground(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_hit_ground, tokens)
}

fn parse_ocl_hit_ground(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_hit_ground, tokens)
}

fn parse_delay_final(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.delay_final_blow_up_from_hit_ground = INI::parse_duration_unsigned_int(token(tokens)?)?;
    Ok(())
}

fn parse_fx_final_blow_up(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_slot(&mut data.fx_final_blow_up, tokens)
}

fn parse_ocl_final_blow_up(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_slot(&mut data.ocl_final_blow_up, tokens)
}

fn parse_death_loop_sound(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.death_loop_sound = AudioEventRts::new(token(tokens)?);
    Ok(())
}

fn parse_roll_rate(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.roll_rate = INI::parse_real(token(tokens)?)?;
    Ok(())
}

fn parse_roll_rate_delta(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.roll_rate_delta = INI::parse_percent_to_real(token(tokens)?)?;
    Ok(())
}

fn parse_pitch_rate(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.pitch_rate = INI::parse_real(token(tokens)?)?;
    Ok(())
}

fn parse_fall_how_fast(
    _ini: &mut INI,
    data: &mut JetSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.fall_how_fast = INI::parse_percent_to_real(token(tokens)?)?;
    Ok(())
}

const JET_SLOW_DEATH_FIELDS: &[FieldParse<JetSlowDeathBehaviorModuleData>] = &[
    FieldParse {
        token: "FXOnGroundDeath",
        parse: parse_fx_on_ground_death,
    },
    FieldParse {
        token: "OCLOnGroundDeath",
        parse: parse_ocl_on_ground_death,
    },
    FieldParse {
        token: "FXInitialDeath",
        parse: parse_fx_initial_death,
    },
    FieldParse {
        token: "OCLInitialDeath",
        parse: parse_ocl_initial_death,
    },
    FieldParse {
        token: "DelaySecondaryFromInitialDeath",
        parse: parse_delay_secondary,
    },
    FieldParse {
        token: "FXSecondary",
        parse: parse_fx_secondary,
    },
    FieldParse {
        token: "OCLSecondary",
        parse: parse_ocl_secondary,
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
        token: "DelayFinalBlowUpFromHitGround",
        parse: parse_delay_final,
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
        token: "DeathLoopSound",
        parse: parse_death_loop_sound,
    },
    FieldParse {
        token: "RollRate",
        parse: parse_roll_rate,
    },
    FieldParse {
        token: "RollRateDelta",
        parse: parse_roll_rate_delta,
    },
    FieldParse {
        token: "PitchRate",
        parse: parse_pitch_rate,
    },
    FieldParse {
        token: "FallHowFast",
        parse: parse_fall_how_fast,
    },
];

pub struct JetSlowDeathBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<JetSlowDeathBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    timer_death_frame: UnsignedInt,
    timer_on_ground_frame: UnsignedInt,
    roll_rate: Real,
    death_loop_sound: AudioEventRts,
    slow_death_activated: Bool,
}

impl JetSlowDeathBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<JetSlowDeathBehaviorModuleData>,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            timer_death_frame: 0,
            timer_on_ground_frame: 0,
            roll_rate: 0.0,
            death_loop_sound: AudioEventRts::new(""),
            slow_death_activated: false,
        }
    }

    fn owner(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.object.upgrade()
    }

    fn do_fx(&self, fx: &Option<Arc<FXList>>, object: &Arc<RwLock<GameObject>>) {
        if let Some(fx) = fx {
            let _ = fx.do_fx_obj(object, None);
        }
    }

    fn do_ocl(&self, ocl: &Option<Arc<ObjectCreationList>>, object: &Arc<RwLock<GameObject>>) {
        if let Some(ocl) = ocl {
            let _ = ObjectCreationList::create(ocl, object, None);
        }
    }

    fn destroy_owner(&self) {
        if let Some(object) = self.owner() {
            if let Ok(object) = object.read() {
                let _ = TheGameLogic::destroy_object_by_id(object.get_id());
            }
        }
    }

    fn stop_loop_sound(&mut self) {
        let handle = self.death_loop_sound.get_playing_handle();
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(handle);
        }
        self.death_loop_sound.set_playing_handle(0);
    }

    fn hit_tree(&self, object: &GameObject) -> bool {
        let Some(physics) = object.get_physics() else {
            return false;
        };
        let Ok(physics) = physics.lock() else {
            return false;
        };
        let tree_id = physics.get_last_collidee();
        if tree_id == INVALID_ID {
            return false;
        }
        TheGameLogic::find_object_by_id(tree_id)
            .and_then(|tree| {
                tree.read()
                    .ok()
                    .map(|tree| tree.is_kind_of(KindOf::Shrubbery))
            })
            .unwrap_or(false)
    }

    fn begin_slow_death_internal(&mut self, _damage_info: &DamageInfo) {
        self.slow_death_activated = true;
        self.timer_death_frame = TheGameLogic::get_frame();
        self.roll_rate = self.module_data.roll_rate;

        let Some(object) = self.owner() else {
            return;
        };

        self.do_fx(&self.module_data.fx_initial_death, &object);
        self.do_ocl(&self.module_data.ocl_initial_death, &object);

        if !self
            .module_data
            .death_loop_sound
            .get_event_name()
            .is_empty()
        {
            self.death_loop_sound = self.module_data.death_loop_sound.clone();
            if let Ok(object_guard) = object.read() {
                self.death_loop_sound.set_object_id(object_guard.get_id());
            }
            if let Some(audio) = TheAudio::get() {
                let handle = audio.add_audio_event(&self.death_loop_sound);
                self.death_loop_sound.set_playing_handle(handle);
            }
        }

        let ai = object
            .read()
            .ok()
            .and_then(|object| object.get_ai_update_interface());
        let Some(ai) = ai else {
            return;
        };
        let locomotor = ai.lock().ok().and_then(|ai| ai.get_cur_locomotor());
        let Some(locomotor) = locomotor else {
            return;
        };
        if let Ok(mut locomotor) = locomotor.lock() {
            let gravity = -1.0;
            locomotor.set_max_lift(-gravity * (1.0 - self.module_data.fall_how_fast));
            locomotor.set_max_turn_rate(0.0);
        };
    }
}

impl UpdateModuleInterface for JetSlowDeathBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if !self.slow_death_activated {
            return Ok(UpdateSleepTime::None);
        }

        let Some(object) = self.owner() else {
            return Ok(UpdateSleepTime::Forever);
        };

        {
            if let Ok(object_guard) = object.read() {
                if let Some(physics) = object_guard.get_physics() {
                    if let Ok(mut physics) = physics.lock() {
                        physics.set_roll_rate(self.roll_rate);
                    }
                }
            }
        }
        self.roll_rate *= self.module_data.roll_rate_delta;

        if self.timer_on_ground_frame == 0 {
            let (height, hit_tree) = {
                let mut height = 1.0;
                let mut hit_tree = false;
                if let Ok(mut object_guard) = object.write() {
                    let position = *object_guard.get_position();
                    let layer = TheTerrainLogic::get()
                        .map(|terrain| terrain.get_layer_for_destination(&position))
                        .unwrap_or(PathfindLayerEnum::Ground);
                    object_guard.set_layer(layer);
                    height = if layer == PathfindLayerEnum::Ground {
                        object_guard.get_height_above_terrain()
                    } else {
                        let layer_height = TheTerrainLogic::get()
                            .map(|terrain| terrain.get_layer_height(position.x, position.y, layer))
                            .unwrap_or(position.z);
                        let height = position.z - layer_height;
                        if (0.0..=1.0).contains(&height) {
                            0.0
                        } else {
                            height
                        }
                    };
                    hit_tree = self.hit_tree(&object_guard);
                }
                (height, hit_tree)
            };

            if height <= 0.0 || hit_tree {
                self.stop_loop_sound();
                self.do_fx(&self.module_data.fx_hit_ground, &object);
                self.do_ocl(&self.module_data.ocl_hit_ground, &object);
                self.timer_on_ground_frame = TheGameLogic::get_frame();

                if let Ok(object_guard) = object.read() {
                    if let Some(physics) = object_guard.get_physics() {
                        if let Ok(mut physics) = physics.lock() {
                            physics.set_pitch_rate(self.module_data.pitch_rate);
                        }
                    }
                }
            }

            if self.timer_death_frame != 0
                && TheGameLogic::get_frame().saturating_sub(self.timer_death_frame)
                    >= self.module_data.delay_secondary_from_initial_death
            {
                self.do_fx(&self.module_data.fx_secondary, &object);
                self.do_ocl(&self.module_data.ocl_secondary, &object);
                self.timer_death_frame = 0;
            }
        } else if TheGameLogic::get_frame().saturating_sub(self.timer_on_ground_frame)
            >= self.module_data.delay_final_blow_up_from_hit_ground
        {
            self.do_fx(&self.module_data.fx_final_blow_up, &object);
            self.do_ocl(&self.module_data.ocl_final_blow_up, &object);
            self.destroy_owner();
        }

        Ok(UpdateSleepTime::None)
    }
}

impl DieModuleInterface for JetSlowDeathBehavior {
    fn on_die(
        &mut self,
        damage: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(object) = self.owner() else {
            return Ok(());
        };

        let ground_death = object
            .read()
            .ok()
            .map(|object| {
                !object.is_significantly_above_terrain()
                    || object
                        .get_status_bits()
                        .test(ObjectStatusTypes::DeckHeightOffset)
            })
            .unwrap_or(true);

        if ground_death {
            self.do_fx(&self.module_data.fx_on_ground_death, &object);
            self.do_ocl(&self.module_data.ocl_on_ground_death, &object);
            self.destroy_owner();
        } else {
            self.begin_slow_death_internal(damage);
        }

        if let Ok(mut object) = object.write() {
            object.clear_status(ObjectStatusMaskType::from_status(
                ObjectStatusTypes::DeckHeightOffset,
            ));
        }
        Ok(())
    }
}

impl SlowDeathBehaviorInterface for JetSlowDeathBehavior {
    fn is_slow_death_active(&self) -> bool {
        self.slow_death_activated
    }

    fn begin_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.begin_slow_death_internal(damage_info);
        Ok(())
    }

    fn get_probability_modifier(&self, _damage_info: &DamageInfo) -> i32 {
        1
    }

    fn is_die_applicable(&self, _damage_info: &DamageInfo) -> bool {
        true
    }

    fn get_slow_death_phase(&self) -> u32 {
        if self.timer_on_ground_frame == 0 {
            0
        } else {
            2
        }
    }
}

impl BehaviorModuleInterface for JetSlowDeathBehavior {
    fn get_module_name(&self) -> &'static str {
        "JetSlowDeathBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn get_slow_death_behavior_interface(&mut self) -> Option<&mut dyn SlowDeathBehaviorInterface> {
        Some(self)
    }
}

impl Snapshotable for JetSlowDeathBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.timer_death_frame)
            .map_err(|err| err.to_string())?;
        xfer.xfer_unsigned_int(&mut self.timer_on_ground_frame)
            .map_err(|err| err.to_string())?;
        xfer.xfer_real(&mut self.roll_rate)
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct JetSlowDeathBehaviorModule {
    behavior: JetSlowDeathBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<JetSlowDeathBehaviorModuleData>,
}

impl JetSlowDeathBehaviorModule {
    pub fn new(
        behavior: JetSlowDeathBehavior,
        module_name: &AsciiString,
        module_data: Arc<JetSlowDeathBehaviorModuleData>,
    ) -> Self {
        Self {
            behavior,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            module_data,
        }
    }
}

impl EngineModule for JetSlowDeathBehaviorModule {
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

impl Snapshotable for JetSlowDeathBehaviorModule {
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

impl BehaviorModuleInterface for JetSlowDeathBehaviorModule {
    fn get_module_name(&self) -> &'static str {
        "JetSlowDeathBehavior"
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
