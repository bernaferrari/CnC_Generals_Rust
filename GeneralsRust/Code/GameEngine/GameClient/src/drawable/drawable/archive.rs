//! Typed serde continuation for the BasicDrawable-owned portion of
//! `CHUNK_GameClient`.
//!
//! The positional Drawable stream interleaves value state, template lookups,
//! module callbacks, and audio side effects. This archive keeps the value
//! domain explicit while preserving the original application order. Nested
//! logic-module continuations are still explicit byte capsules: their payloads
//! belong to the broader lifecycle/module conversion and are not opaque at the
//! archive boundary — each is identified by module bucket and module tag.

use super::*;
use game_engine::common::audio::audio_event_rts::AudioPriority;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::audio::game_audio::get_global_audio_manager;
use game_engine::common::bit_flags::create_model_condition_flags;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::system::Xfer;
use serde::{Deserialize, Serialize};
use super::icons::DrawableIconArchive;
use std::io::Cursor;

pub type Matrix3x4Archive = [[f32; 4]; 3];

fn matrix_to_archive(matrix: &Matrix4) -> Matrix3x4Archive {
    [
        matrix.elements[0],
        matrix.elements[1],
        matrix.elements[2],
    ]
}

fn matrix_from_archive(archive: Matrix3x4Archive) -> Matrix4 {
    Matrix4 {
        elements: [
            archive[0],
            archive[1],
            archive[2],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

/// Explicit override values persisted by `DynamicAudioEventInfo::xfer_no_name`.
/// `Some` means the corresponding override bit was set; the customized audio
/// name itself is deterministic from the drawable ID and base event name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicAudioOverrideArchive {
    pub loop_flag: Option<bool>,
    pub loop_count: Option<i32>,
    pub volume: Option<f32>,
    pub min_volume: Option<f32>,
    pub min_range: Option<f32>,
    pub max_range: Option<f32>,
    pub priority: Option<AudioPriority>,
}

impl DynamicAudioOverrideArchive {
    fn capture(info: &DynamicAudioEventInfo) -> Self {
        Self {
            loop_flag: info
                .was_loop_flag_overridden()
                .then(|| (info.audio_event_info.control & game_engine::common::audio::audio_event_rts::AC_LOOP) != 0),
            loop_count: info
                .was_loop_count_overridden()
                .then_some(info.audio_event_info.loop_count),
            volume: info
                .was_volume_overridden()
                .then_some(info.audio_event_info.volume),
            min_volume: info
                .was_min_volume_overridden()
                .then_some(info.audio_event_info.min_volume),
            min_range: info
                .was_min_range_overridden()
                .then_some(info.audio_event_info.min_distance),
            max_range: info
                .was_max_range_overridden()
                .then_some(info.audio_event_info.max_distance),
            priority: info
                .was_priority_overridden()
                .then_some(info.audio_event_info.priority),
        }
    }

    fn apply_to(&self, info: &mut DynamicAudioEventInfo) {
        if let Some(value) = self.loop_flag {
            info.override_loop_flag(value);
        }
        if let Some(value) = self.loop_count {
            info.override_loop_count(value);
        }
        if let Some(value) = self.volume {
            info.override_volume(value);
        }
        if let Some(value) = self.min_volume {
            info.override_min_volume(value);
        }
        if let Some(value) = self.min_range {
            info.override_min_range(value);
        }
        if let Some(value) = self.max_range {
            info.override_max_range(value);
        }
        if let Some(value) = self.priority {
            info.override_priority(value);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CustomAmbientArchive {
    None,
    Silence,
    Custom {
        base_name: String,
        overrides: DynamicAudioOverrideArchive,
    },
}

/// Identified continuation for one drawable module. The payload is the exact
/// module-owned continuation bytes for now; unlike the former top-level opaque
/// companion, ownership, ordering, bucket, and identity are explicit here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawableModuleArchive {
    pub module_type_index: u16,
    pub identifier: String,
    pub payload: Vec<u8>,
}

/// Semantic BasicDrawable continuation. Fields whose wire representation is
/// quantized by C++ (notably flash color) are stored in that quantized domain
/// so a typed round trip does not resurrect precision the native load lost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicDrawableArchive {
    pub drawable_id: u32,
    pub model_condition_bits: Vec<bool>,
    pub transform: Matrix3x4Archive,
    pub selection_flash_envelope: Option<TintEnvelope>,
    pub tint_envelope: Option<TintEnvelope>,
    pub terrain_decal_type: TerrainDecalType,
    pub explicit_opacity: f32,
    pub stealth_opacity: f32,
    pub effective_stealth_opacity: f32,
    pub decal_opacity_fade_target: f32,
    pub decal_opacity_fade_rate: f32,
    pub decal_opacity: f32,
    pub object_id: u32,
    pub status_bits: u32,
    pub tint_status_bits: u32,
    pub prev_tint_status_bits: u32,
    pub fade_mode: FadingMode,
    pub time_elapsed_fade: u32,
    pub time_to_fade: u32,
    pub loco_info: Option<LocoInfo>,
    pub modules: Vec<DrawableModuleArchive>,
    pub flash_count: u32,
    pub flash_color_argb: i32,
    pub hidden: bool,
    pub hidden_by_stealth: bool,
    pub second_material_pass_opacity: f32,
    pub instance_transform: Matrix3x4Archive,
    pub instance_scale: f32,
    pub shroud_status_object_id: u32,
    pub expiration_frame: Option<u32>,
    pub icons: Vec<DrawableIconArchive>,
    pub ambient_sound_enabled: bool,
    pub ambient_sound_enabled_from_script: bool,
    pub custom_ambient: CustomAmbientArchive,
}

impl Default for BasicDrawableArchive {
    fn default() -> Self {
        Self {
            drawable_id: 1,
            model_condition_bits: vec![false; 128],
            transform: matrix_to_archive(&Matrix4::identity()),
            selection_flash_envelope: None,
            tint_envelope: None,
            terrain_decal_type: TerrainDecalType::None,
            explicit_opacity: 1.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            decal_opacity_fade_target: 1.0,
            decal_opacity_fade_rate: 0.0,
            decal_opacity: 1.0,
            object_id: 0,
            status_bits: 0,
            tint_status_bits: 0,
            prev_tint_status_bits: 0,
            fade_mode: FadingMode::None,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            loco_info: None,
            modules: Vec::new(),
            flash_count: 0,
            flash_color_argb: 0,
            hidden: false,
            hidden_by_stealth: false,
            second_material_pass_opacity: 1.0,
            instance_transform: matrix_to_archive(&Matrix4::identity()),
            instance_scale: 1.0,
            shroud_status_object_id: 0,
            expiration_frame: None,
            icons: Vec::new(),
            ambient_sound_enabled: false,
            ambient_sound_enabled_from_script: false,
            custom_ambient: CustomAmbientArchive::None,
        }
    }
}


impl BasicDrawable {
    /// Capture the state owned by C++ Drawable::xfer v7. Runtime-only native
    /// resources, receiving-template ownership, presentation residuals, and
    /// live audio objects are deliberately outside this continuation.
    pub fn capture_archive(&mut self) -> Result<BasicDrawableArchive, String> {
        let mut modules = Vec::new();
        for module_type in 0..2u16 {
            for module in &mut self.draw_modules {
                if module.drawable_module_type_index() != module_type as usize {
                    continue;
                }
                let Some(identifier) = module.snapshot_module_identifier().map(str::to_owned) else {
                    continue;
                };
                let mut payload = Vec::new();
                {
                    let cursor = Cursor::new(&mut payload);
                    let mut xfer = XferSave::new(cursor, 1);
                    module.xfer_snapshot(&mut xfer)?;
                }
                modules.push(DrawableModuleArchive {
                    module_type_index: module_type,
                    identifier,
                    payload,
                });
            }
        }

        let custom_ambient = if self.custom_sound_ambient_off {
            CustomAmbientArchive::Silence
        } else if let Some(info) = self.custom_sound_ambient_dynamic_info.as_ref() {
            CustomAmbientArchive::Custom {
                base_name: self
                    .custom_sound_ambient_base_name
                    .clone()
                    .unwrap_or_else(|| info.get_original_name().to_string()),
                overrides: DynamicAudioOverrideArchive::capture(info),
            }
        } else {
            CustomAmbientArchive::None
        };

        Ok(BasicDrawableArchive {
            drawable_id: self.id.0,
            model_condition_bits: (0..self.model_condition_flags.size())
                .map(|index| self.model_condition_flags.test(index))
                .collect(),
            transform: matrix_to_archive(&Matrix4::translation(self.position).mul(&self.base_transform)),
            selection_flash_envelope: self.selection_flash_envelope.clone(),
            tint_envelope: self.tint_envelope.clone(),
            terrain_decal_type: self.terrain_decal_type,
            explicit_opacity: self.explicit_opacity,
            stealth_opacity: self.stealth_opacity,
            effective_stealth_opacity: self.effective_stealth_opacity,
            decal_opacity_fade_target: self.decal_opacity_fade_target,
            decal_opacity_fade_rate: self.decal_opacity_fade_rate,
            decal_opacity: self.decal_opacity,
            object_id: self.object_id.unwrap_or(0),
            status_bits: self.status.bits,
            tint_status_bits: self.tint_status.bits,
            prev_tint_status_bits: self.prev_tint_status.bits,
            fade_mode: self.fade_mode,
            time_elapsed_fade: self.time_elapsed_fade,
            time_to_fade: self.time_to_fade,
            loco_info: self.loco_info.clone().map(|mut info| {
                // DrawableLocoInfo::xfer does not persist the modulator pair.
                // Canonicalize them away so typed restore cannot resurrect
                // state the native continuation would have dropped.
                info.yaw_modulator = 0.0;
                info.pitch_modulator = 0.0;
                info
            }),
            modules,
            flash_count: self.flash_count,
            flash_color_argb: vector3_to_color_bits(self.flash_color),
            hidden: self.hidden,
            hidden_by_stealth: self.hidden_by_stealth,
            second_material_pass_opacity: self.second_material_pass_opacity,
            instance_transform: matrix_to_archive(&self.instance_transform),
            instance_scale: self.instance_scale,
            shroud_status_object_id: self.drawable_info.shroud_status_object_id,
            expiration_frame: self.expiration_frame,
            icons: self
                .icon_info
                .as_ref()
                .map(IconInfo::capture_archive)
                .transpose()?
                .unwrap_or_default(),
            ambient_sound_enabled: self.ambient_sound_enabled,
            ambient_sound_enabled_from_script: self.ambient_sound_enabled_from_script,
            custom_ambient,
        })
    }

    /// Apply a typed continuation in the same order as `Drawable::xfer` v7.
    /// `client_frame` reproduces the frame context supplied to nested module
    /// continuations by the outer GameClient positional stream.
    pub fn apply_archive(
        &mut self,
        archive: &BasicDrawableArchive,
        client_frame: u32,
        receiving_audio_device: Option<
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
        native_receiving_context: bool,
        disabled_content: DisabledIconContent<'_>,
    ) -> Result<(), String> {
        self.id = DrawableId(archive.drawable_id);
        self.drawable_info.set_drawable_id(archive.drawable_id);

        let mut flags = create_model_condition_flags();
        for (index, value) in archive.model_condition_bits.iter().copied().enumerate() {
            if index < flags.size() {
                flags.set(index, value);
            }
        }
        self.replace_model_condition_flags(flags, true);

        let mut transform = matrix_from_archive(archive.transform);
        self.position = Vector3::new(
            transform.elements[0][3],
            transform.elements[1][3],
            transform.elements[2][3],
        );
        transform.elements[0][3] = 0.0;
        transform.elements[1][3] = 0.0;
        transform.elements[2][3] = 0.0;
        self.base_transform = transform;

        self.selection_flash_envelope = archive.selection_flash_envelope.clone();
        self.tint_envelope = archive.tint_envelope.clone();

        self.set_terrain_decal(archive.terrain_decal_type);
        self.explicit_opacity = archive.explicit_opacity;
        self.stealth_opacity = archive.stealth_opacity;
        self.effective_stealth_opacity = archive.effective_stealth_opacity;
        self.decal_opacity_fade_target = archive.decal_opacity_fade_target;
        self.decal_opacity_fade_rate = archive.decal_opacity_fade_rate;
        self.decal_opacity = archive.decal_opacity;
        self.object_id = (archive.object_id != 0).then_some(archive.object_id);

        self.status.bits = archive.status_bits;
        self.tint_status.bits = archive.tint_status_bits;
        self.prev_tint_status.bits = archive.prev_tint_status_bits;
        self.fade_mode = archive.fade_mode;
        self.time_elapsed_fade = archive.time_elapsed_fade;
        self.time_to_fade = archive.time_to_fade;
        self.loco_info = archive.loco_info.clone();

        // Drawable::xferDrawableModules walks bucket 0 then bucket 1. Unknown
        // saved modules are ignored, matching the positional block skip.
        for module_type in 0..2u16 {
            for module_archive in archive
                .modules
                .iter()
                .filter(|module| module.module_type_index == module_type)
            {
                let Some(module) = self.draw_modules.iter_mut().find(|module| {
                    module.drawable_module_type_index() == module_type as usize
                        && module.snapshot_module_identifier() == Some(module_archive.identifier.as_str())
                }) else {
                    continue;
                };
                let cursor = Cursor::new(module_archive.payload.clone());
                let mut xfer = XferLoad::new(cursor, 1);
                xfer.set_client_frame_context(game_engine::ClientFrameContext::Frame(client_frame))
                    .map_err(|error| format!("module client frame context: {error:?}"))?;
                module.xfer_snapshot(&mut xfer)?;
            }
        }

        // The positional stream carries stealth look here, then resets it to
        // None before returning. Omit the dead value and preserve the result.
        self.stealth_look = StealthLook::None;
        self.flash_count = archive.flash_count;
        self.flash_color = color_bits_to_vector3(archive.flash_color_argb);
        self.hidden = archive.hidden;
        self.hidden_by_stealth = archive.hidden_by_stealth;
        self.second_material_pass_opacity = archive.second_material_pass_opacity;
        self.instance_transform = matrix_from_archive(archive.instance_transform);
        self.instance_scale = archive.instance_scale;
        self.drawable_info.shroud_status_object_id = archive.shroud_status_object_id;
        self.expiration_frame = archive.expiration_frame;

        self.icon_info = IconInfo::from_archive(&archive.icons, disabled_content)?;

        if self.hidden || self.hidden_by_stealth {
            self.update_hidden_status();
        }
        if let Some(device) = receiving_audio_device {
            let mut audio = device
                .lock()
                .map_err(|_| "receiving drawable audio manager poisoned".to_string())?;
            self.stop_ambient_sound_with_audio(&mut audio);
        } else if !native_receiving_context {
            self.stop_ambient_sound();
        }

        self.ambient_sound_enabled = archive.ambient_sound_enabled;
        self.ambient_sound_enabled_from_script = archive.ambient_sound_enabled_from_script;

        match &archive.custom_ambient {
            CustomAmbientArchive::None => {
                self.custom_sound_ambient_off = false;
                self.custom_sound_ambient_base_name = None;
                self.custom_sound_ambient_dynamic_info = None;
                self.custom_sound_ambient_base_resolved = false;
            }
            CustomAmbientArchive::Silence => {
                self.custom_sound_ambient_off = true;
                self.custom_sound_ambient_base_name = None;
                self.custom_sound_ambient_dynamic_info = None;
                self.custom_sound_ambient_base_resolved = false;
            }
            CustomAmbientArchive::Custom { base_name, overrides } => {
                self.custom_sound_ambient_off = false;
                let compatibility_manager =
                    if receiving_audio_device.is_none() && !native_receiving_context {
                        get_global_audio_manager()
                    } else {
                        None
                    };
                let base = if let Some(device) = receiving_audio_device {
                    device
                        .lock()
                        .map_err(|_| "receiving drawable audio manager poisoned".to_string())?
                        .find_audio_event_info(base_name)
                } else if !native_receiving_context {
                    match compatibility_manager.as_ref() {
                        Some(manager) => manager
                            .lock()
                            .map_err(|_| "ambient audio catalog lock poisoned".to_string())?
                            .find_audio_event_info(base_name),
                        None => None,
                    }
                } else {
                    None
                };

                let mut custom_info = base
                    .as_deref()
                    .map(DynamicAudioEventInfo::from_base_info)
                    .unwrap_or_default();
                let customized_name = format!(" CUSTOM {} {}", self.id.0 as i32, base_name);
                custom_info.override_audio_name(&customized_name);
                overrides.apply_to(&mut custom_info);

                if base.is_some() || native_receiving_context {
                    if !native_receiving_context {
                        if let Some(manager) = compatibility_manager.as_ref() {
                            manager
                                .lock()
                                .map_err(|_| "ambient audio catalog lock poisoned".to_string())?
                                .register_audio_event_info(
                                    custom_info.get_audio_event_info().clone(),
                                );
                        }
                    }
                    self.custom_sound_ambient_base_name = Some(base_name.clone());
                    self.custom_sound_ambient_dynamic_info = Some(custom_info);
                    self.custom_sound_ambient_base_resolved = base.is_some();
                } else {
                    self.custom_sound_ambient_base_name = None;
                    self.custom_sound_ambient_dynamic_info = None;
                    self.custom_sound_ambient_base_resolved = false;
                }
            }
        }

        self.reset_volatile_shroud_state();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_bytes(drawable: &mut BasicDrawable) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = XferSave::new(cursor, 1);
            drawable
                .xfer_with_audio(&mut xfer, None, false, DisabledIconContent::Compatibility)
                .unwrap();
        }
        bytes
    }

    fn populated_source() -> BasicDrawable {
        let mut drawable = BasicDrawable::new(DrawableId(44));
        drawable.position = Vector3::new(9.0, 8.0, 7.0);
        drawable.base_transform = Matrix4::rotation_z(0.25);
        drawable.instance_transform = Matrix4::rotation_x(0.125);
        drawable.instance_scale = 1.75;
        drawable.explicit_opacity = 0.8;
        drawable.stealth_opacity = 0.6;
        drawable.effective_stealth_opacity = 0.5;
        drawable.decal_opacity = 0.4;
        drawable.decal_opacity_fade_target = 0.3;
        drawable.decal_opacity_fade_rate = 0.2;
        drawable.status.bits = 0x1234;
        drawable.tint_status.bits = 0x2345;
        drawable.prev_tint_status.bits = 0x3456;
        drawable.fade_mode = FadingMode::FadingOut;
        drawable.time_elapsed_fade = 11;
        drawable.time_to_fade = 12;
        drawable.flash_count = 13;
        drawable.flash_color = Vector3::new(0.25, 0.5, 0.75);
        drawable.hidden = true;
        drawable.hidden_by_stealth = true;
        drawable.second_material_pass_opacity = 0.15;
        drawable.drawable_info.shroud_status_object_id = 77;
        drawable.expiration_frame = Some(99);
        drawable.ambient_sound_enabled = true;
        drawable.ambient_sound_enabled_from_script = true;
        drawable.selection_flash_envelope = Some(TintEnvelope {
            attack_rate: Vector3::new(1.0, 0.0, 0.0),
            decay_rate: Vector3::new(0.0, 1.0, 0.0),
            peak_color: Vector3::new(0.0, 0.0, 1.0),
            current_color: Vector3::new(0.5, 0.5, 0.5),
            sustain_counter: 5,
            state: EnvelopeState::Sustain,
            is_effective: true,
        });
        drawable.loco_info = Some(LocoInfo {
            pitch: 1.0,
            pitch_rate: 2.0,
            roll: 3.0,
            roll_rate: 4.0,
            yaw: 5.0,
            acceleration_pitch: 6.0,
            acceleration_pitch_rate: 7.0,
            acceleration_roll: 8.0,
            acceleration_roll_rate: 9.0,
            overlap_z_velocity: 10.0,
            overlap_z: 11.0,
            wobble: 12.0,
            yaw_modulator: 13.0,
            pitch_modulator: 14.0,
            wheel_info: WheelInfo {
                front_left_height_offset: 1.0,
                front_right_height_offset: 2.0,
                rear_left_height_offset: 3.0,
                rear_right_height_offset: 4.0,
                wheel_angle: 5.0,
                frames_airborne_counter: 6,
                frames_airborne: 7,
            },
        });
        drawable.set_object_id(Some(73));
        drawable
    }

    #[test]
    fn typed_archive_application_recreates_legacy_drawable_bytes() {
        let mut source = populated_source();
        let expected = legacy_bytes(&mut source);
        let archive = source.capture_archive().unwrap();

        let mut restored = BasicDrawable::new(DrawableId(1));
        restored
            .apply_archive(&archive, 900, None, false, DisabledIconContent::Compatibility)
            .unwrap();
        assert_eq!(legacy_bytes(&mut restored), expected);
    }

    #[test]
    fn legacy_load_then_typed_capture_matches_source_archive() {
        let mut source = populated_source();
        let bytes = legacy_bytes(&mut source);
        let expected = source.capture_archive().unwrap();

        let mut restored = BasicDrawable::new(DrawableId(1));
        let cursor = Cursor::new(bytes);
        let mut xfer = XferLoad::new(cursor, 1);
        restored
            .xfer_with_audio(&mut xfer, None, false, DisabledIconContent::Compatibility)
            .unwrap();
        assert_eq!(restored.capture_archive().unwrap(), expected);
    }

    #[test]
    fn absent_custom_ambient_is_distinct_from_silenced_ambient() {
        let mut source = BasicDrawable::new(DrawableId(3));
        source.custom_sound_ambient_off = true;
        let archive = source.capture_archive().unwrap();
        assert_eq!(archive.custom_ambient, CustomAmbientArchive::Silence);

        let mut restored = BasicDrawable::new(DrawableId(1));
        restored
            .apply_archive(&archive, 0, None, false, DisabledIconContent::Compatibility)
            .unwrap();
        assert!(restored.custom_sound_ambient_off);
        assert_eq!(
            restored.capture_archive().unwrap().custom_ambient,
            CustomAmbientArchive::Silence
        );
    }
}
