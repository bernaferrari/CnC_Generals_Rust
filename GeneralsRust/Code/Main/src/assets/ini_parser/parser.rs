use super::*;
/// One C++ `ModelConditionInfo` weapon-bone field. Kept private because the
/// public frozen record exposes the four exact base-name slots directly.
#[derive(Clone, Copy)]
enum DrawWeaponBoneField {
    FireFx,
    Recoil,
    MuzzleFlash,
    Launch,
    HideShow,
}

/// One module-scoped `W3DModelDrawModuleData` recoil field. Unlike the
/// weapon-bone fields above, these do not belong to a condition-state block.
#[derive(Clone, Copy)]
enum DrawRecoilKinematicField {
    InitialSpeed,
    MaxDistance,
    Damping,
    SettleSpeed,
}

/// INI Parser for Generals object definitions
pub struct IniParser {
    /// Loaded object definitions indexed by name
    definitions: HashMap<String, ObjectDefinition>,
}

impl IniParser {
    /// Create a new INI parser
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
        }
    }

    /// Parse INI content from bytes
    pub fn parse_ini_content(&mut self, content: &str, filename: &str) -> Result<usize> {
        debug!("Parsing INI file: {}", filename);

        let lines: Vec<&str> = content.lines().collect();
        let mut current_object: Option<ObjectDefinition> = None;
        // This parser intentionally remains lightweight, but Draw blocks have
        // a regular enough shape to preserve their authored ConditionState
        // table without mistaking nested Behavior/Body `End`s for Object ends.
        let mut active_draw_module: Option<usize> = None;
        let mut active_condition_state: Option<usize> = None;
        let mut active_behavior_module: Option<usize> = None;
        // A Behavior block may contain nested data blocks such as `Turret`.
        // Keep the owning module active until its own `End`; otherwise the
        // fields after a nested block (for example DeployStyle's PackTime)
        // leak out of the module and cannot be mapped to gameplay metadata.
        let mut active_behavior_depth = 0usize;
        // WeaponSet is an outer Object block, not a Behavior. Track it
        // explicitly so its nested `Weapon = PRIMARY ...` rows cannot be
        // mistaken for top-level compatibility fields.
        let mut active_weapon_set: Option<usize> = None;
        let mut active_weapon_set_depth = 0usize;
        let mut active_armor_set: Option<usize> = None;
        let mut active_armor_set_depth = 0usize;
        let mut active_prerequisites = false;

        let mut object_count = 0;
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();

            // Skip empty lines and comments
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }

            // Section headers like [ObjectList] are ignored
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }

            // Object definition header: Object/ChildObject/ObjectReskin
            if Self::is_object_header(trimmed) {
                // Save previous object if any
                if let Some(obj) = current_object.take() {
                    self.definitions.insert(obj.name.clone(), obj);
                    object_count += 1;
                }

                let (class_name, parent_name) = Self::parse_object_header(trimmed)
                    .unwrap_or_else(|| ("UnnamedObject".to_string(), None));
                let mut object = ObjectDefinition::new(class_name);
                object.parent_name = parent_name;
                current_object = Some(object);
                active_draw_module = None;
                active_condition_state = None;
                active_behavior_module = None;
                active_behavior_depth = 0;
                active_weapon_set = None;
                active_weapon_set_depth = 0;
                active_armor_set = None;
                active_armor_set_depth = 0;
                active_prerequisites = false;

                trace!("Found object: {}", current_object.as_ref().unwrap().name);
                continue;
            }

            // End of object definition
            if trimmed.eq_ignore_ascii_case("End") {
                if current_object.is_some()
                    && active_weapon_set.is_none()
                    && active_armor_set.is_none()
                    && !active_prerequisites
                    && Self::is_object_terminator(&lines, index + 1)
                {
                    if let Some(obj) = current_object.take() {
                        self.definitions.insert(obj.name.clone(), obj);
                        object_count += 1;
                    }
                    active_draw_module = None;
                    active_condition_state = None;
                    active_behavior_module = None;
                    active_behavior_depth = 0;
                    active_weapon_set = None;
                    active_weapon_set_depth = 0;
                    active_armor_set = None;
                    active_armor_set_depth = 0;
                    active_prerequisites = false;
                } else {
                    if active_weapon_set.is_some() {
                        active_weapon_set_depth = active_weapon_set_depth.saturating_sub(1);
                        if active_weapon_set_depth == 0 {
                            active_weapon_set = None;
                        }
                        continue;
                    }
                    if active_armor_set.is_some() {
                        active_armor_set_depth = active_armor_set_depth.saturating_sub(1);
                        if active_armor_set_depth == 0 {
                            active_armor_set = None;
                        }
                        continue;
                    }
                    if active_prerequisites {
                        active_prerequisites = false;
                        continue;
                    }

                    // Nested block terminator.  A condition-state block closes
                    // before its Draw module; other nested Object blocks do
                    // not affect the currently retained Draw data.
                    if active_condition_state.take().is_none() {
                        active_draw_module = None;
                    }
                    // Behavior blocks can themselves own nested blocks such
                    // as `Turret`.  Only their outer terminator ends the
                    // module attribution; an inner terminator must leave
                    // later direct fields on the same source module.
                    if active_behavior_module.is_some() {
                        active_behavior_depth = active_behavior_depth.saturating_sub(1);
                        if active_behavior_depth == 0 {
                            active_behavior_module = None;
                        }
                    }
                }
                continue;
            }

            // Parse key = value pairs within an object
            if let Some(obj) = &mut current_object {
                if Self::is_weapon_set_header(trimmed) {
                    obj.weapon_sets.push(WeaponSetDefinition::default());
                    active_weapon_set = obj.weapon_sets.len().checked_sub(1);
                    active_weapon_set_depth = usize::from(active_weapon_set.is_some());
                    active_draw_module = None;
                    active_condition_state = None;
                    continue;
                }
                if Self::is_armor_set_header(trimmed) {
                    obj.armor_sets.push(ArmorSetDefinition::default());
                    active_armor_set = obj.armor_sets.len().checked_sub(1);
                    active_armor_set_depth = usize::from(active_armor_set.is_some());
                    active_draw_module = None;
                    active_condition_state = None;
                    continue;
                }
                if Self::is_prerequisites_header(trimmed) {
                    active_prerequisites = true;
                    active_draw_module = None;
                    active_condition_state = None;
                    continue;
                }

                // Object INI nested module headers have no `=`.  The parser
                // does not need their individual schema here, but it must
                // count them so an `End` within a Behavior does not close the
                // entire Behavior module early.
                if active_behavior_module.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_behavior_depth = active_behavior_depth.saturating_add(1);
                }
                if active_weapon_set.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_weapon_set_depth = active_weapon_set_depth.saturating_add(1);
                }
                if active_armor_set.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_armor_set_depth = active_armor_set_depth.saturating_add(1);
                }
                if let Some((condition_state_key, condition_state_value)) =
                    Self::parse_condition_state_declaration(trimmed)
                {
                    if condition_state_key.eq_ignore_ascii_case("DefaultConditionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::default_state(),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("ConditionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::condition_state(Self::condition_tokens(
                                condition_state_value,
                            )),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("AliasConditionState") {
                        Self::append_draw_condition_alias(
                            obj,
                            active_draw_module,
                            Self::condition_tokens(condition_state_value),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("TransitionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::transition_state(Self::condition_tokens(
                                condition_state_value,
                            )),
                        );
                    }
                    continue;
                }

                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim();
                    let value = Self::strip_inline_comment(trimmed[eq_pos + 1..].trim()).trim();

                    // Remove quotes if present
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        &value[1..value.len() - 1]
                    } else {
                        value
                    };
                    if active_prerequisites {
                        if key.eq_ignore_ascii_case("Object") || key.eq_ignore_ascii_case("Science")
                        {
                            obj.prerequisite_lines
                                .push((key.to_string(), value.to_string()));
                        }
                        continue;
                    }

                    // Parse specific fields
                    let lower_key = key.to_lowercase();

                    if let Some(set) =
                        active_weapon_set.and_then(|index| obj.weapon_sets.get_mut(index))
                    {
                        match lower_key.as_str() {
                            "conditions" => {
                                set.conditions = Self::condition_tokens(value);
                            }
                            "weapon" => set.record_weapon(value),
                            "autochoosesources" => {
                                set.attributes.insert(key.to_string(), value.to_string());
                                if let Some(slot) = value.split_whitespace().next() {
                                    set.attributes.insert(
                                        format!("AutoChooseSources {slot}"),
                                        value.to_string(),
                                    );
                                }
                            }
                            _ => {
                                set.attributes.insert(key.to_string(), value.to_string());
                            }
                        }
                        // Nested WeaponSet fields are deliberately not copied
                        // into `ObjectDefinition::attributes`: that lossy map
                        // would otherwise turn a conditional mine primary into
                        // a normal Object-level weapon later in loading.
                        continue;
                    }
                    if let Some(set) =
                        active_armor_set.and_then(|index| obj.armor_sets.get_mut(index))
                    {
                        match lower_key.as_str() {
                            "conditions" => set.record_conditions(value),
                            "armor" => set.record_armor(value),
                            "damagefx" => set.record_damage_fx(value),
                            _ => {}
                        }
                        continue;
                    }

                    // Preserve every field under the active Behavior module
                    // before the generic object-level parser potentially
                    // overwrites a repeated raw key.
                    if lower_key != "behavior" {
                        if let Some(module) = active_behavior_module
                            .and_then(|index| obj.behavior_modules.get_mut(index))
                        {
                            module.insert_attribute(key.to_string(), value.to_string());
                        }
                    }

                    match lower_key.as_str() {
                        "type" => obj.object_type = value.to_string(),
                        "displayname" => {
                            obj.display_name = translate_object_display_name(value);
                        }
                        "model" | "modelname" | "w3dmodel" => {
                            Self::assign_model_name(obj, value);
                            Self::assign_draw_condition_model(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            );
                        }
                        "animation" => Self::assign_draw_condition_animation(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "idleanimation" => Self::assign_draw_condition_animation(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "animationmode" => Self::assign_draw_condition_animation_mode(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "transitionkey" => Self::assign_draw_condition_transition_key(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "waitforstatetofinishifpossible" => {
                            Self::assign_draw_condition_allow_to_finish_key(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            )
                        }
                        "flags" => Self::assign_draw_condition_flags(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "showsubobject" => Self::assign_draw_condition_subobject_visibility(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "hidesubobject" => Self::assign_draw_condition_subobject_visibility(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        // These four fields belong to C++
                        // W3DModelDrawModuleData, rather than any nested
                        // ModelConditionInfo. Retain them on the exact Draw
                        // module before freezing its selected state.
                        "initialrecoilspeed" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::InitialSpeed,
                        ),
                        "maxrecoildistance" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::MaxDistance,
                        ),
                        "recoildamping" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::Damping,
                        ),
                        "recoilsettlespeed" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::SettleSpeed,
                        ),
                        // C++ W3DModelDrawModuleData owns this bit mask,
                        // outside every ModelConditionInfo.  Do not promote a
                        // nested spelling into module-wide clip feedback.
                        "projectilebonefeedbackenabledslots" => {
                            Self::assign_draw_module_projectile_bone_feedback(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            )
                        }
                        // C++ W3DModelDrawModuleData::m_animationsRequirePower.
                        "animationsrequirepower" => {
                            Self::assign_draw_module_animations_require_power(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            )
                        }
                        // C++ W3DModelDraw stores the primary `m_turrets[0]`
                        // binding on every ModelConditionInfo. These fields
                        // are presentation-only but must survive the exact
                        // DefaultConditionState inheritance boundary before
                        // the active HLOD collector may use them.
                        "turret" => Self::assign_draw_condition_primary_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "turretpitch" => Self::assign_draw_condition_primary_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "turretartangle" => Self::assign_draw_condition_primary_turret_art(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "turretartpitch" => Self::assign_draw_condition_primary_turret_art(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        // Preserve whether C++ has an active second turret
                        // slot. The bounded Main renderer deliberately does
                        // not route primary gameplay angles into it.
                        "altturret" => Self::assign_draw_condition_alternate_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "altturretpitch" => Self::assign_draw_condition_alternate_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        // C++ W3DModelDraw retains four independent
                        // WeaponSlotType-indexed base names on each selected
                        // ModelConditionInfo. They delimit barrel topology;
                        // do not collapse them to the currently visible
                        // recoil/muzzle pair.
                        "particlesysbone" => Self::assign_draw_condition_particle_sys_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "weaponfirefxbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::FireFx,
                        ),
                        "weaponrecoilbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::Recoil,
                        ),
                        "weaponmuzzleflash" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::MuzzleFlash,
                        ),
                        "weaponlaunchbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::Launch,
                        ),
                        "weaponhideshowbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::HideShow,
                        ),
                        // AltTurretArtAngle/AltTurretArtPitch only affect an
                        // active alternate bone, which already makes the
                        // bounded primary-only renderer fail closed. Do not
                        // manufacture a second-turret runtime by retaining
                        // those offsets as primary values.
                        "altturretartangle" | "altturretartpitch" => {}
                        "draw" => {
                            obj.draw_module = Some(value.to_string());
                            obj.draw_modules
                                .push(DrawModuleDefinition::new(value.to_string()));
                            active_draw_module = obj.draw_modules.len().checked_sub(1);
                            active_condition_state = None;
                            active_behavior_module = None;
                            active_behavior_depth = 0;
                        }
                        "behavior" => {
                            active_draw_module = None;
                            active_condition_state = None;
                            if let Some(module) = BehaviorModuleDefinition::parse(value) {
                                obj.behavior_modules.push(module);
                                active_behavior_module = obj.behavior_modules.len().checked_sub(1);
                                active_behavior_depth =
                                    usize::from(active_behavior_module.is_some());
                            } else {
                                active_behavior_module = None;
                                active_behavior_depth = 0;
                            }
                            // Keep the legacy raw declaration available for
                            // diagnostics.  Module-aware gameplay must use
                            // `behavior_modules`, because this map is lossy.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "drawmodule" => obj.draw_module = Some(value.to_string()),
                        "ignoreconditionstates" => {
                            if let Some(draw_module) =
                                active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
                            {
                                draw_module.ignored_condition_tokens =
                                    Self::condition_tokens(value);
                            }
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "armortype" => obj.armor_type = Some(value.to_string()),
                        "subdualdamagecap" => {
                            obj.subdual_damage_cap = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|cap| cap.is_finite());
                        }
                        "subdualdamagehealrate" => {
                            obj.subdual_heal_rate_frames =
                                Self::parse_subdual_heal_rate_frames(value);
                        }
                        "subdualdamagehealamount" => {
                            obj.subdual_heal_amount = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|amount| amount.is_finite());
                        }
                        "hitpoints" | "health" | "maxhealth" => {
                            obj.hit_points = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|health| health.is_finite());
                        }
                        "scale" => {
                            if let Ok(scale) = value.trim().parse::<f32>() {
                                obj.scale = scale;
                                obj.scale_was_specified = true;
                            }
                        }
                        "owner" => obj.owner = Some(value.to_string()),
                        // Object INI: Weapon = PRIMARY / SECONDARY / TERTIARY Name.
                        // Each slot is independent; later lines must not wipe a
                        // different slot's declaration.
                        "weapon" => {
                            let mut parts = value.split_whitespace();
                            if let Some(slot) = parts.next() {
                                if let Some(wname) = parts.next() {
                                    if !wname.eq_ignore_ascii_case("None") {
                                        if slot.eq_ignore_ascii_case("PRIMARY") {
                                            obj.primary_weapon = Some(wname.to_string());
                                        } else if slot.eq_ignore_ascii_case("SECONDARY") {
                                            obj.secondary_weapon = Some(wname.to_string());
                                        } else if slot.eq_ignore_ascii_case("TERTIARY") {
                                            obj.tertiary_weapon = Some(wname.to_string());
                                        }
                                    }
                                }
                            }
                            // Keep raw attribute for diagnostics; last "Weapon =" wins in attributes map.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "locomotor" => {
                            let mut fields = value.split_whitespace();
                            let Some(set_name) = fields.next() else {
                                obj.attributes.insert(key.to_string(), value.to_string());
                                continue;
                            };
                            let locomotor_names = fields.map(str::to_string).collect::<Vec<_>>();
                            obj.locomotor_sets.push(LocomotorSetDefinition {
                                set_name: set_name.to_string(),
                                locomotor_names,
                            });
                            // Preserve the legacy last-row view for old
                            // consumers, but all behavior-sensitive callers
                            // must use `locomotor_sets` above.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        // Texture references (various formats used in C&C)
                        _ if lower_key.contains("texture") => {
                            obj.textures.insert(key.to_string(), value.to_string());
                        }
                        // Store other attributes
                        _ => {
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        // Don't forget the last object if file doesn't end with "End"
        if let Some(obj) = current_object.take() {
            self.definitions.insert(obj.name.clone(), obj);
            object_count += 1;
        }

        debug!("Parsed {} objects from {}", object_count, filename);
        Ok(object_count)
    }

    fn is_object_header(line: &str) -> bool {
        Self::parse_object_header(line).is_some()
    }

    fn is_prerequisites_header(line: &str) -> bool {
        !line.contains('=')
            && line
                .split_whitespace()
                .next()
                .is_some_and(|head| head.eq_ignore_ascii_case("Prerequisites"))
    }

    fn is_weapon_set_header(line: &str) -> bool {
        !line.contains('=')
            && line
                .split_whitespace()
                .next()
                .is_some_and(|head| head.eq_ignore_ascii_case("WeaponSet"))
    }

    fn is_armor_set_header(line: &str) -> bool {
        !line.contains('=')
            && line
                .split_whitespace()
                .next()
                .is_some_and(|head| head.eq_ignore_ascii_case("ArmorSet"))
    }

    /// C++ `INI::parseDurationUnsignedInt` → logic frames (`ceil(msec * 30 / 1000)`).
    fn parse_subdual_heal_rate_frames(value: &str) -> Option<u32> {
        let msec = value.split_whitespace().next()?.parse::<f32>().ok()?;
        if !msec.is_finite() {
            return None;
        }
        if msec <= 0.0 {
            return Some(0);
        }
        Some(((msec * 30.0) / 1000.0).ceil() as u32)
    }

    fn parse_object_header(line: &str) -> Option<(String, Option<String>)> {
        if line.contains('=') {
            return None;
        }

        let mut tokens = line.split_whitespace();
        let head = tokens.next()?;
        match head {
            "Object" => tokens.next().map(|name| (name.to_string(), None)),
            "ChildObject" | "ObjectReskin" => {
                let name = tokens.next()?.to_string();
                let parent_name = tokens.next().map(|s| s.to_string());
                Some((name, parent_name))
            }
            _ => None,
        }
    }

    fn is_object_terminator(lines: &[&str], start_idx: usize) -> bool {
        for line in lines.iter().skip(start_idx) {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }
            return Self::is_object_header(trimmed);
        }
        true
    }

    fn strip_inline_comment(value: &str) -> &str {
        let bytes = value.as_bytes();
        let mut in_single = false;
        let mut in_double = false;
        let mut i = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b'\'' if !in_double => in_single = !in_single,
                b'"' if !in_single => in_double = !in_double,
                b';' | b'#' if !in_single && !in_double => return value[..i].trim_end(),
                b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                    return value[..i].trim_end();
                }
                _ => {}
            }
            i += 1;
        }

        value
    }

    pub(super) fn condition_tokens(value: &str) -> Vec<String> {
        value
            .split_whitespace()
            .map(|token| token.trim().trim_matches(',').to_string())
            .filter(|token| !token.is_empty())
            .collect()
    }

    /// Recognize both retail spellings accepted by C++ INI: the usual
    /// `ConditionState = DAMAGED` and the compact
    /// `AliasConditionState WEAPONSET_PLAYER_UPGRADE` form.  We do this
    /// before generic key/value parsing so aliases without `=` do not vanish.
    fn parse_condition_state_declaration(line: &str) -> Option<(&str, &str)> {
        let (key, value) = if let Some(eq) = line.find('=') {
            (line[..eq].trim(), line[eq + 1..].trim())
        } else {
            let mut parts = line.splitn(2, char::is_whitespace);
            let key = parts.next()?.trim();
            let value = parts.next().unwrap_or_default().trim();
            (key, value)
        };
        if key.eq_ignore_ascii_case("DefaultConditionState")
            || key.eq_ignore_ascii_case("ConditionState")
            || key.eq_ignore_ascii_case("AliasConditionState")
            || key.eq_ignore_ascii_case("TransitionState")
        {
            Some((key, value))
        } else {
            None
        }
    }

    fn start_draw_condition_state(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        mut state: DrawConditionStateDefinition,
    ) -> Option<usize> {
        let module = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))?;
        // `W3DModelDrawModuleData::parseConditionState` starts both normal
        // and transition states as a copy of DefaultConditionState before it
        // reads the new block.  Keep that source-authored inherited Model so
        // a valid ConditionState which changes only animation/bones still
        // resolves to its default mesh rather than disappearing.
        if !state.is_default {
            if let Some(default) = module
                .condition_states
                .iter()
                .find(|candidate| candidate.is_default)
            {
                state.model = default.model.clone();
                state.animations = default.animations.clone();
                state.animation_mode = default.animation_mode.clone();
                state.subobject_visibility = default.subobject_visibility.clone();
                state.primary_turret = default.primary_turret.clone();
                state.weapon_bone_bindings = default.weapon_bone_bindings.clone();
                state.particle_sys_bones = default.particle_sys_bones.clone();
                state.transition_key = default.transition_key.clone();
                state.allow_to_finish_key = default.allow_to_finish_key.clone();
                state.flags = default.flags;
                if state.is_transition {
                    // C++ PARSE_TRANSITION copies Default then clears keys.
                    state.transition_key.clear();
                    state.allow_to_finish_key.clear();
                }
                state.animations_copied_from_default = true;
            }
        }
        module.condition_states.push(state);
        module.condition_states.len().checked_sub(1)
    }

    fn append_draw_condition_alias(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        condition_tokens: Vec<String>,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ does not insert a TransitionState into m_conditionStates, so an
        // AliasConditionState after a transition still aliases the most recent
        // selectable state.  Preserve that behavior while retaining the
        // transition in source order for diagnostics.
        let Some(state) = module
            .condition_states
            .iter_mut()
            .rev()
            .find(|candidate| !candidate.is_transition)
        else {
            return;
        };
        state.condition_sets.push(condition_tokens);
    }

    fn assign_draw_condition_model(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ W3DTreeDraw / W3DPropDraw store ModelName on the module
        // (`W3DTreeDrawModuleData::m_modelName`), not inside a ConditionState.
        // Promote that name to a DefaultConditionState so drawable lookup uses
        // the reskin mesh (PTXPine03) instead of the ObjectReskin identity
        // (TreeSpruce03). W3DModelDraw blocks that already have a state keep
        // writing the active ConditionState Model as before.
        let state = if let Some(index) = active_condition_state {
            module.condition_states.get_mut(index)
        } else {
            if !module.condition_states.iter().any(|s| s.is_default) {
                module
                    .condition_states
                    .insert(0, DrawConditionStateDefinition::default_state());
            }
            module.condition_states.iter_mut().find(|s| s.is_default)
        };
        if let Some(state) = state {
            state.set_model(value);
        }
    }

    /// Preserve C++ `parseAnimation`: the first local Animation field clears
    /// a list inherited from DefaultConditionState, lowercases the source
    /// identity, and repeats a non-`None` entry at least once.
    fn assign_draw_condition_animation(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_idle: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };

        if state.animations_copied_from_default {
            state.animations.clear();
            state.animations_copied_from_default = false;
        }

        let mut tokens = value.split_whitespace();
        let Some(name) = tokens.next() else {
            return;
        };
        let distance_covered_token = tokens.next().map(str::to_string);
        let times_to_repeat = tokens
            .next()
            .and_then(|token| token.parse::<i64>().ok())
            .filter(|times| *times >= 1)
            .and_then(|times| usize::try_from(times).ok())
            .unwrap_or(1);

        let name = name.to_ascii_lowercase();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            return;
        }
        let animation = AuthoredDrawAnimation {
            name,
            is_idle,
            distance_covered_token,
        };
        state
            .animations
            .extend(std::iter::repeat(animation).take(times_to_repeat));
    }

    fn assign_draw_condition_animation_mode(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        state.animation_mode = AuthoredDrawAnimationMode::parse(value);
    }

    fn assign_draw_condition_transition_key(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let key = value
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        state.transition_key = if key.is_empty() || key == "none" {
            String::new()
        } else {
            key
        };
    }

    fn assign_draw_condition_allow_to_finish_key(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let key = value
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        state.allow_to_finish_key = if key.is_empty() || key == "none" {
            String::new()
        } else {
            key
        };
    }

    fn assign_draw_condition_flags(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        state.flags = parse_ac_bits_flags(value);
    }

    /// Parse one `INI::scanReal`-style numeric token. Module recoil fields
    /// consume only their first source token; trailing text belongs to later
    /// parser fields in C++, not to an invented compound value here.
    fn parse_draw_recoil_real(value: &str) -> Option<f32> {
        value.split_whitespace().next()?.parse::<f32>().ok()
    }

    /// Retain one `W3DModelDrawModuleData` recoil coefficient on the active
    /// Draw module. C++ parses the two speed fields through
    /// `INI::parseVelocityReal`, which converts authored source units to
    /// per-logic-frame values; the record's setters reproduce that conversion
    /// while preserving constructor defaults unchanged.
    fn assign_draw_module_recoil_kinematics(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        field: DrawRecoilKinematicField,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ switches to `parseConditionState` for nested state fields;
        // none of these W3DModelDrawModuleData keys belongs there. Keep the
        // lightweight parser from silently promoting malformed nested source
        // into a module-wide kinetic override.
        if active_condition_state.is_some() {
            module.recoil_kinematics.source_fields_valid = false;
            return;
        }
        let Some(value) = Self::parse_draw_recoil_real(value) else {
            module.recoil_kinematics.source_fields_valid = false;
            return;
        };
        match field {
            DrawRecoilKinematicField::InitialSpeed => {
                module.recoil_kinematics.set_initial_recoil_speed(value)
            }
            DrawRecoilKinematicField::MaxDistance => {
                module.recoil_kinematics.set_max_recoil_distance(value)
            }
            DrawRecoilKinematicField::Damping => module.recoil_kinematics.set_recoil_damping(value),
            DrawRecoilKinematicField::SettleSpeed => {
                module.recoil_kinematics.set_recoil_settle_speed(value)
            }
        }
    }

    fn assign_draw_module_animations_require_power(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        if active_condition_state.is_some() {
            return;
        }
        let token = value.split_whitespace().next().unwrap_or("").trim();
        // C++ INI::parseBool: Yes/True/1.
        let parsed =
            token.eq_ignore_ascii_case("yes") || token.eq_ignore_ascii_case("true") || token == "1";
        let no =
            token.eq_ignore_ascii_case("no") || token.eq_ignore_ascii_case("false") || token == "0";
        if parsed || no {
            module.animations_require_power = AuthoredAnimationsRequirePower(parsed);
        }
    }

    /// Parse C++ `INI::parseBitString32` using `TheWeaponSlotTypeNames`.
    ///
    /// The engine distinguishes a normal list (which replaces the old mask)
    /// from `+`/`-` edits (which retain it), permits `NONE` only by itself,
    /// and treats any unknown slot as an INI error.  This retained source
    /// record uses `None` for that error so presentation can fail closed while
    /// the broad Object parser continues preserving other valid data.
    fn parse_draw_projectile_bone_feedback_slots(value: &str, previous: u32) -> Option<u32> {
        let mut mask = previous;
        let mut found_normal = false;
        let mut found_add_or_sub = false;

        for token in value.split_whitespace() {
            if token.eq_ignore_ascii_case("none") {
                if found_normal || found_add_or_sub {
                    return None;
                }
                return Some(0);
            }

            let (operation, slot_name) = match token.as_bytes().first().copied() {
                Some(b'+') => (Some(true), &token[1..]),
                Some(b'-') => (Some(false), &token[1..]),
                _ => (None, token),
            };
            let slot = match slot_name.to_ascii_uppercase().as_str() {
                "PRIMARY" => 0u32,
                "SECONDARY" => 1u32,
                "TERTIARY" => 2u32,
                _ => return None,
            };
            let bit = 1u32 << slot;

            match operation {
                Some(add) => {
                    if found_normal {
                        return None;
                    }
                    if add {
                        mask |= bit;
                    } else {
                        mask &= !bit;
                    }
                    found_add_or_sub = true;
                }
                None => {
                    if found_add_or_sub {
                        return None;
                    }
                    if !found_normal {
                        mask = 0;
                    }
                    mask |= bit;
                    found_normal = true;
                }
            }
        }

        Some(mask)
    }

    /// Retain C++ `ProjectileBoneFeedbackEnabledSlots` on the exact active
    /// Draw module.  It is not a ModelConditionInfo field, so recognizing it
    /// inside a state would silently apply a malformed module-wide override.
    fn assign_draw_module_projectile_bone_feedback(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        if active_condition_state.is_some() {
            module.projectile_bone_feedback.source_fields_valid = false;
            return;
        }
        let Some(mask) = Self::parse_draw_projectile_bone_feedback_slots(
            value,
            module.projectile_bone_feedback.enabled_slots,
        ) else {
            module.projectile_bone_feedback.source_fields_valid = false;
            return;
        };
        module.projectile_bone_feedback.enabled_slots = mask;
    }

    /// Preserve C++ `parseShowHideSubObject`: one declaration may name several
    /// subobjects, `None` as its first token clears the inherited vector, and
    /// a duplicate rewrites its existing value without changing its source
    /// order.  C++ normalizes the name to lowercase at parse time.
    fn assign_draw_condition_subobject_visibility(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        hidden: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };

        let mut names = value.split_whitespace();
        let Some(first_name) = names.next() else {
            return;
        };
        if first_name.eq_ignore_ascii_case("none") {
            state.subobject_visibility.clear();
            return;
        }

        for name in std::iter::once(first_name).chain(names) {
            let name = name.trim().trim_matches(',').to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            if let Some(existing) = state
                .subobject_visibility
                .iter_mut()
                .find(|existing| existing.name.eq_ignore_ascii_case(name.as_str()))
            {
                existing.hidden = hidden;
            } else {
                state
                    .subobject_visibility
                    .push(AuthoredDrawSubobjectVisibility { name, hidden });
            }
        }
    }

    /// C++ `parseBoneNameKey` lowercases one token and clears it for empty or
    /// `None`. Keep that behavior independent for the yaw and pitch entries.
    fn parse_draw_turret_bone_name(value: &str) -> Option<String> {
        let name = value.split_whitespace().next()?.trim();
        (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_ascii_lowercase())
    }

    fn assign_draw_condition_primary_turret_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let bone = Self::parse_draw_turret_bone_name(value);
        if is_yaw {
            state.primary_turret.yaw_bone = bone;
        } else {
            state.primary_turret.pitch_bone = bone;
        }
    }

    fn assign_draw_condition_primary_turret_art(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        if is_yaw {
            state.primary_turret.set_yaw_art_angle_degrees(value);
        } else {
            state.primary_turret.set_pitch_art_angle_degrees(value);
        }
    }

    fn assign_draw_condition_alternate_turret_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let active = Self::parse_draw_turret_bone_name(value).is_some();
        if is_yaw {
            state.primary_turret.alternate_yaw_bone_present = active;
        } else {
            state.primary_turret.alternate_pitch_bone_present = active;
        }
    }

    /// Read one C++ `INI::getNextAsciiString`-style token. Bone names may be
    /// quoted and contain spaces (for example `"UIMOB03 R HAND"`), whereas a
    /// bare token ends at whitespace. The caller intentionally ignores any
    /// later text just as the C++ field parser consumes its next token only.
    fn next_draw_ascii_token(value: &str) -> Option<(&str, &str)> {
        let value = value.trim_start();
        let first = *value.as_bytes().first()?;
        if first == b'\'' || first == b'"' {
            let quote = first as char;
            let tail = &value[1..];
            let end = tail.find(quote)?;
            return Some((&tail[..end], &tail[end + 1..]));
        }
        let end = value.find(char::is_whitespace).unwrap_or(value.len());
        Some((&value[..end], &value[end..]))
    }

    /// C++ `parseWeaponBoneName` takes `WeaponSlotType` then one ASCII bone
    /// token, lowercases it, and clears that exact slot for `None`. Do not
    /// accept an unknown slot by redirecting it to PRIMARY: the frozen source
    /// state must be rejected later rather than inventing a recoil topology.
    fn parse_draw_weapon_bone(value: &str) -> Option<(usize, Option<String>)> {
        let (slot, rest) = Self::next_draw_ascii_token(value)?;
        let slot = match slot.to_ascii_uppercase().as_str() {
            "PRIMARY" => 0,
            "SECONDARY" => 1,
            "TERTIARY" => 2,
            _ => return None,
        };
        let (bone, _) = Self::next_draw_ascii_token(rest)?;
        let bone = bone.trim();
        let bone = (!bone.is_empty() && !bone.eq_ignore_ascii_case("none"))
            .then(|| bone.to_ascii_lowercase());
        Some((slot, bone))
    }

    /// C++ `parseParticleSysBone`: bone name + ParticleSystem template.
    fn assign_draw_condition_particle_sys_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let mut tokens = value.split_whitespace();
        let Some(bone) = tokens.next() else {
            return;
        };
        let Some(system) = tokens.next() else {
            return;
        };
        let bone = bone.to_ascii_lowercase();
        if bone.is_empty() || system.eq_ignore_ascii_case("none") {
            return;
        }
        state.particle_sys_bones.push((bone, system.to_string()));
    }

    fn assign_draw_condition_weapon_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        field: DrawWeaponBoneField,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let Some((slot, bone)) = Self::parse_draw_weapon_bone(value) else {
            state.weapon_bone_bindings.source_fields_valid = false;
            return;
        };
        let Some(bindings) = state.weapon_bone_bindings.slots.get_mut(slot) else {
            // The parser only accepts the three C++ WeaponSlotType names, but
            // preserve fail-closed behavior if this record is ever widened.
            state.weapon_bone_bindings.source_fields_valid = false;
            return;
        };
        match field {
            DrawWeaponBoneField::FireFx => bindings.fire_fx_bone_base = bone,
            DrawWeaponBoneField::Recoil => bindings.recoil_bone_base = bone,
            DrawWeaponBoneField::MuzzleFlash => bindings.muzzle_flash_bone_base = bone,
            DrawWeaponBoneField::Launch => bindings.launch_bone_base = bone,
            DrawWeaponBoneField::HideShow => bindings.projectile_hide_show_bone = bone,
        }
    }

    fn assign_model_name(obj: &mut ObjectDefinition, value: &str) {
        if value.is_empty() || value.eq_ignore_ascii_case("none") {
            return;
        }

        if obj.model_name.is_none() {
            obj.model_name = Some(value.to_string());
        }
    }

    /// Get an object definition by name
    pub fn get_definition(&self, name: &str) -> Option<&ObjectDefinition> {
        self.definitions.get(name)
    }

    /// Get all definitions
    pub fn get_all_definitions(&self) -> &HashMap<String, ObjectDefinition> {
        &self.definitions
    }

    /// Get total number of definitions loaded
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    /// Clear all loaded definitions
    pub fn clear(&mut self) {
        self.definitions.clear();
    }
}

impl Default for IniParser {
    fn default() -> Self {
        Self::new()
    }
}
