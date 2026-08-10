/// W3DModelDraw module data
///
/// Reference: W3DModelDrawModuleData in W3DModelDraw.h
#[derive(Debug, Clone)]
pub struct W3DModelDrawModuleData {
    /// Module tag name key
    pub module_tag_name_key: NameKeyType,

    /// Model condition states
    pub condition_states: Vec<ModelConditionInfo>,

    /// Transition states (`TransitionState` in INI), keyed at runtime by from/to pair.
    pub transition_states: Vec<ModelConditionInfo>,

    /// Track file for leaving marks on terrain
    pub track_file: AsciiString,

    /// Bone to attach this drawable to (on parent)
    pub attach_to_drawable_bone: AsciiString,

    /// Cached attach bone offset
    pub attach_to_drawable_bone_offset: Coord3D,

    /// Default state index
    pub default_state: i32,

    /// Which weapon slots have projectile bone feedback enabled
    pub projectile_bone_feedback_enabled_slots: u32,

    /// Initial recoil amount
    pub initial_recoil: Real,

    /// Maximum recoil distance
    pub max_recoil: Real,

    /// Recoil damping factor
    pub recoil_damping: Real,

    /// Recoil settle speed
    pub recoil_settle: Real,

    /// Minimum LOD level required
    pub min_lod_required: i32,

    /// Model conditions to ignore
    pub ignore_condition_states: ModelConditionFlags,

    /// Whether model color can be changed
    pub ok_to_change_model_color: bool,

    /// Whether animations require power
    pub animations_require_power: bool,

    /// Whether particles are attached to animated bones
    pub particles_attached_to_animated_bones: bool,

    /// Whether object receives dynamic lights
    pub receives_dynamic_lights: bool,

    /// Extra public bones
    pub extra_public_bones: Vec<AsciiString>,
}

impl W3DModelDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            condition_states: Vec::new(),
            transition_states: Vec::new(),
            track_file: AsciiString::new(),
            attach_to_drawable_bone: AsciiString::new(),
            attach_to_drawable_bone_offset: Coord3D::origin(),
            default_state: -1,
            projectile_bone_feedback_enabled_slots: 0,
            initial_recoil: 2.0,
            max_recoil: 3.0,
            recoil_damping: 0.4,
            recoil_settle: 0.065,
            min_lod_required: 0,
            ignore_condition_states: ModelConditionFlags::empty(),
            ok_to_change_model_color: false,
            animations_require_power: true,
            particles_attached_to_animated_bones: false,
            receives_dynamic_lights: true,
            extra_public_bones: Vec::new(),
        }
    }

    /// Parse module data from an INI block.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        parse_model_draw_module_data_block(ini, self)
    }

    /// Parse a single key/value field in this module block.
    pub(crate) fn parse_ini_field(
        &mut self,
        ini: &mut INI,
        key: &str,
        tokens: &[&str],
    ) -> Result<bool, INIError> {
        match key.to_ascii_uppercase().as_str() {
            "INITIALRECOILSPEED" => {
                self.initial_recoil = INI::parse_velocity_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "MAXRECOILDISTANCE" => {
                self.max_recoil = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "RECOILDAMPING" => {
                self.recoil_damping = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "RECOILSETTLESPEED" => {
                self.recoil_settle = INI::parse_velocity_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "OKTOCHANGEMODELCOLOR" => {
                self.ok_to_change_model_color = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "ANIMATIONSREQUIREPOWER" => {
                self.animations_require_power = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "PARTICLESATTACHEDTOANIMATEDBONES" => {
                self.particles_attached_to_animated_bones =
                    INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "MINLODREQUIRED" => {
                self.min_lod_required = parse_static_game_lod_level(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "PROJECTILEBONEFEEDBACKENABLEDSLOTS" => {
                self.projectile_bone_feedback_enabled_slots = parse_weapon_slot_mask(tokens);
                Ok(true)
            }
            "DEFAULTCONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Default)?;
                Ok(true)
            }
            "CONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Normal)?;
                Ok(true)
            }
            "ALIASCONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Alias)?;
                Ok(true)
            }
            "TRANSITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Transition)?;
                Ok(true)
            }
            "TRACKMARKS" => {
                let track = parse_ascii_lower(parse_required_value(tokens)?)?;
                self.track_file = AsciiString::from(track.as_str());
                Ok(true)
            }
            "EXTRAPUBLICBONE" => {
                for token in tokens {
                    let value = INI::parse_ascii_string(token)?;
                    if value.is_empty() {
                        continue;
                    }
                    self.extra_public_bones
                        .push(AsciiString::from(value.as_str()));
                }
                Ok(true)
            }
            "ATTACHTOBONEINANOTHERMODULE" => {
                let bone = parse_ascii_lower(parse_required_value(tokens)?)?;
                self.attach_to_drawable_bone = AsciiString::from(bone.as_str());
                Ok(true)
            }
            "IGNORECONDITIONSTATES" => {
                self.ignore_condition_states = parse_model_condition_flags_tokens(tokens);
                Ok(true)
            }
            "RECEIVESDYNAMICLIGHTS" => {
                self.receives_dynamic_lights = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn parse_condition_state(
        &mut self,
        ini: &mut INI,
        tokens: &[&str],
        state_type: ParseCondStateType,
    ) -> Result<(), INIError> {
        match state_type {
            ParseCondStateType::Alias => {
                if self.condition_states.is_empty() {
                    return Err(INIError::InvalidData);
                }
                let conditions_yes = parse_model_condition_flags_tokens(tokens);
                if conditions_yes.intersects(self.ignore_condition_states) {
                    return Err(INIError::InvalidData);
                }
                if does_state_exist(&self.condition_states, conditions_yes) {
                    return Err(INIError::InvalidData);
                }
                if conditions_yes.is_empty() && self.default_state >= 0 {
                    return Err(INIError::InvalidData);
                }
                if let Some(last) = self.condition_states.last_mut() {
                    last.conditions_yes.push(conditions_yes);
                    return Ok(());
                }
                Err(INIError::InvalidData)
            }
            _ => {
                let mut info = ModelConditionInfo::new();
                match state_type {
                    ParseCondStateType::Default => {
                        if self.default_state >= 0
                            || !tokens.is_empty()
                            || !self.condition_states.is_empty()
                        {
                            return Err(INIError::InvalidData);
                        }
                        self.default_state = self.condition_states.len() as i32;
                        info.conditions_yes.push(ModelConditionFlags::empty());
                    }
                    ParseCondStateType::Transition => {
                        let from_name = parse_ascii_lower(parse_required_value(tokens)?)?;
                        let to_name = parse_ascii_lower(
                            tokens
                                .iter()
                                .copied()
                                .skip(1)
                                .find(|token| !token.is_empty())
                                .ok_or(INIError::InvalidData)?,
                        )?;
                        if from_name == to_name {
                            return Err(INIError::InvalidData);
                        }
                        if self.default_state >= 0 {
                            let idx = self.default_state as usize;
                            if let Some(default_state) = self.condition_states.get(idx) {
                                info = default_state.clone();
                                info.ini_read_flags |= INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT;
                                info.transition_key = NAMEKEY_INVALID;
                                info.allow_to_finish_key = NAMEKEY_INVALID;
                            }
                        }
                        info.transition_from_key = if from_name.is_empty() || from_name == "none" {
                            NAMEKEY_INVALID
                        } else {
                            name_key_generate(&from_name)
                        };
                        info.transition_to_key = if to_name.is_empty() || to_name == "none" {
                            NAMEKEY_INVALID
                        } else {
                            name_key_generate(&to_name)
                        };
                    }
                    ParseCondStateType::Normal => {
                        if self.default_state >= 0 {
                            let idx = self.default_state as usize;
                            if let Some(default_state) = self.condition_states.get(idx) {
                                info = default_state.clone();
                                info.ini_read_flags |= INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT;
                                info.conditions_yes.clear();
                            }
                        }
                        let conditions_yes = parse_model_condition_flags_tokens(tokens);
                        if conditions_yes.intersects(self.ignore_condition_states) {
                            return Err(INIError::InvalidData);
                        }
                        if self.default_state < 0
                            && self.condition_states.is_empty()
                            && !conditions_yes.is_empty()
                        {
                            return Err(INIError::InvalidData);
                        }
                        if conditions_yes.is_empty() && self.default_state >= 0 {
                            return Err(INIError::InvalidData);
                        }
                        if does_state_exist(&self.condition_states, conditions_yes) {
                            return Err(INIError::InvalidData);
                        }
                        info.conditions_yes.push(conditions_yes);
                    }
                    ParseCondStateType::Alias => unreachable!(),
                }

                parse_model_condition_info_block(ini, &mut info)?;

                if info.model_name.is_empty() {
                    return Err(INIError::InvalidData);
                }
                if info.model_name.is_none() {
                    info.model_name.clear();
                }
                if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0
                    && (info.ini_read_flags & INI_READ_FLAG_GOT_NONIDLE_ANIMS) != 0
                {
                    return Err(INIError::InvalidData);
                }
                if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0
                    && info.anim_mode != AnimMode::Once
                    && info.anim_mode != AnimMode::OnceBackwards
                {
                    return Err(INIError::InvalidData);
                }
                info.refresh_projectile_valid_bit();

                if state_type == ParseCondStateType::Transition {
                    if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0 {
                        return Err(INIError::InvalidData);
                    }
                    if info.anim_mode != AnimMode::Once && info.anim_mode != AnimMode::OnceBackwards
                    {
                        return Err(INIError::InvalidData);
                    }
                    if info.transition_key != NAMEKEY_INVALID
                        || info.allow_to_finish_key != NAMEKEY_INVALID
                    {
                        return Err(INIError::InvalidData);
                    }
                    self.transition_states.push(info);
                } else {
                    self.condition_states.push(info);
                }

                Ok(())
            }
        }
    }

    /// Find best model condition info matching given conditions
    ///
    /// Implements the sparse matching algorithm from C++ SparseMatchFinder.h
    /// Reference: /GeneralsMD/Code/GameEngine/Include/Common/SparseMatchFinder.h:99-162
    ///
    /// The algorithm finds the ModelConditionInfo that best matches the given conditions by:
    /// 1. Maximizing the number of matching "yes" bits
    /// 2. Minimizing extraneous "yes" bits (bits set in the state but not in the query)
    pub fn find_best_info(&self, conditions: &ModelConditionFlags) -> Option<&ModelConditionInfo> {
        let filtered_conditions = *conditions & !self.ignore_condition_states;
        let mut best_match: Option<&ModelConditionInfo> = None;
        let mut best_yes_match = 0;
        let mut best_yes_extraneous_bits = i32::MAX;

        // Iterate through all condition states
        for state in &self.condition_states {
            // Each state can have multiple condition flag combinations (conditions_yes)
            for yes_flags in &state.conditions_yes {
                // Count how many bits match between query and state
                let yes_match = (filtered_conditions.bits() & yes_flags.bits()).count_ones() as i32;

                // Count extraneous bits: bits set in state but not in query
                let yes_extraneous_bits =
                    (yes_flags.bits() & !filtered_conditions.bits()).count_ones() as i32;

                // Select best match:
                // - Prefer more matching bits
                // - If tied, prefer fewer extraneous bits
                // Reference: W3DModelDraw.cpp:133-143
                if yes_match > best_yes_match
                    || (yes_match == best_yes_match
                        && yes_extraneous_bits < best_yes_extraneous_bits)
                {
                    best_match = Some(state);
                    best_yes_match = yes_match;
                    best_yes_extraneous_bits = yes_extraneous_bits;
                }
            }
        }

        // If no match found, return default state or first state
        best_match.or_else(|| {
            if self.default_state >= 0
                && (self.default_state as usize) < self.condition_states.len()
            {
                self.condition_states.get(self.default_state as usize)
            } else {
                self.condition_states.first()
            }
        })
    }
}

impl Default for W3DModelDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DModelDrawModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl DrawModuleData for W3DModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DModelDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut clone = self.clone();
        clone.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DModelDrawModuleData::xfer (version 1) persists validated
        // runtime caches (pristine bones/turret bones/barrel launch matrices).
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        for state in &mut self.condition_states {
            let mut valid_stuff = model_condition_valid_stuff(state) as i8;
            xfer.xfer_byte(&mut valid_stuff)
                .map_err(|e| e.to_string())?;
            if xfer.is_reading() {
                state.valid_stuff = valid_stuff as u8;
            }

            if valid_stuff == 0 {
                continue;
            }

            let mut pristine_keys: Vec<NameKeyType> =
                state.pristine_bones.keys().copied().collect();
            pristine_keys.sort_unstable();
            for key in pristine_keys {
                if let Some(bone) = state.pristine_bones.get_mut(&key) {
                    xfer.xfer_int(&mut bone.bone_index)
                        .map_err(|e| e.to_string())?;
                    xfer_matrix3d_values(xfer, &mut bone.transform)?;
                }
            }

            for turret_index in 0..MAX_TURRETS {
                let mut turret_angle_bone = state
                    .turrets
                    .get(turret_index)
                    .map(|turret| turret.turret_angle_bone)
                    .unwrap_or(0);
                let mut turret_pitch_bone = state
                    .turrets
                    .get(turret_index)
                    .map(|turret| turret.turret_pitch_bone)
                    .unwrap_or(0);
                xfer.xfer_int(&mut turret_angle_bone)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut turret_pitch_bone)
                    .map_err(|e| e.to_string())?;
                if xfer.is_reading() {
                    if state.turrets.len() <= turret_index {
                        state.turrets.resize_with(turret_index + 1, TurretInfo::new);
                    }
                    if let Some(turret) = state.turrets.get_mut(turret_index) {
                        turret.turret_angle_bone = turret_angle_bone;
                        turret.turret_pitch_bone = turret_pitch_bone;
                    }
                }
            }

            for barrels in &mut state.weapon_barrels {
                for barrel in barrels {
                    xfer_matrix3d_values(xfer, &mut barrel.projectile_offset_mtx)?;
                }
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

