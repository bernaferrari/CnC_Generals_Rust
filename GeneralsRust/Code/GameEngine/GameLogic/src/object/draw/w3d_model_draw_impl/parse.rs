fn parse_model_draw_module_data_block(
    ini: &mut INI,
    data: &mut W3DModelDrawModuleData,
) -> Result<(), INIError> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini
            .get_line_tokens()
            .into_iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        let Some(key) = tokens.first().cloned() else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }

        let value_tokens = tokens
            .iter()
            .map(String::as_str)
            .skip(1)
            .filter(|token| *token != "=")
            .collect::<Vec<_>>();
        if !data.parse_ini_field(ini, key.as_str(), &value_tokens)? {
            return Err(INIError::UnknownToken);
        }
    }
    Ok(())
}

fn parse_model_condition_info_block(
    ini: &mut INI,
    info: &mut ModelConditionInfo,
) -> Result<(), INIError> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(key) = tokens.first().copied() else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let value_tokens: Vec<&str> = tokens
            .iter()
            .copied()
            .skip(1)
            .filter(|token| *token != "=")
            .collect();
        if !parse_model_condition_info_field(info, key, &value_tokens)? {
            return Err(INIError::UnknownToken);
        }
    }
    Ok(())
}

fn parse_model_condition_info_field(
    info: &mut ModelConditionInfo,
    key: &str,
    tokens: &[&str],
) -> Result<bool, INIError> {
    match key.to_ascii_uppercase().as_str() {
        "MODEL" => {
            let model = parse_ascii_lower(parse_required_value(tokens)?)?;
            info.model_name = AsciiString::from(model.as_str());
            Ok(true)
        }
        "TURRET" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 0);
            turret.turret_angle_name_key = bone_key;
            Ok(true)
        }
        "TURRETARTANGLE" => {
            let turret = ensure_turret_slot(info, 0);
            turret.turret_art_angle = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "TURRETPITCH" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 0);
            turret.turret_pitch_name_key = bone_key;
            Ok(true)
        }
        "TURRETARTPITCH" => {
            let turret = ensure_turret_slot(info, 0);
            turret.turret_art_pitch = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "ALTTURRET" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 1);
            turret.turret_angle_name_key = bone_key;
            Ok(true)
        }
        "ALTTURRETARTANGLE" => {
            let turret = ensure_turret_slot(info, 1);
            turret.turret_art_angle = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "ALTTURRETPITCH" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 1);
            turret.turret_pitch_name_key = bone_key;
            Ok(true)
        }
        "ALTTURRETARTPITCH" => {
            let turret = ensure_turret_slot(info, 1);
            turret.turret_art_pitch = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "SHOWSUBOBJECT" => {
            parse_show_hide_sub_objects(info, tokens, false)?;
            Ok(true)
        }
        "HIDESUBOBJECT" => {
            parse_show_hide_sub_objects(info, tokens, true)?;
            Ok(true)
        }
        "WEAPONFIREFXBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_fire_fx_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONRECOILBONE" => {
            parse_weapon_bone(tokens, &mut info.weapon_recoil_bone, &mut info.public_bones)?;
            Ok(true)
        }
        "WEAPONMUZZLEFLASH" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_muzzle_flash,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONLAUNCHBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_projectile_launch_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONHIDESHOWBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_projectile_hide_show_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "ANIMATION" => {
            parse_animation(info, tokens, false)?;
            Ok(true)
        }
        "IDLEANIMATION" => {
            parse_animation(info, tokens, true)?;
            Ok(true)
        }
        "ANIMATIONMODE" => {
            info.anim_mode = parse_anim_mode(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "TRANSITIONKEY" => {
            info.transition_key = parse_name_key_value(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "WAITFORSTATETOFINISHIFPOSSIBLE" => {
            info.allow_to_finish_key = parse_name_key_value(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "FLAGS" => {
            info.flags = parse_ac_bits_flags(tokens)?;
            Ok(true)
        }
        "PARTICLESYSBONE" => {
            let bone_name = parse_ascii_lower(parse_required_value(tokens)?)?;
            let particle_system = tokens
                .iter()
                .copied()
                .skip(1)
                .find(|token| !token.is_empty())
                .map(INI::parse_ascii_string)
                .transpose()?
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            info.particle_sys_bones.push(ParticleSysBoneInfo {
                bone_name: AsciiString::from(bone_name.as_str()),
                particle_system: AsciiString::from(particle_system.as_str()),
            });
            Ok(true)
        }
        "ANIMATIONSPEEDFACTORRANGE" => {
            let min_token = parse_required_value(tokens)?;
            let max_token = tokens
                .iter()
                .copied()
                .skip(1)
                .find(|token| !token.is_empty())
                .ok_or(INIError::InvalidData)?;
            info.anim_min_speed_factor = INI::parse_real(min_token)?;
            info.anim_max_speed_factor = INI::parse_real(max_token)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)
}

fn parse_ascii_lower(token: &str) -> Result<String, INIError> {
    Ok(INI::parse_ascii_string(token)?.to_ascii_lowercase())
}

fn parse_static_game_lod_level(token: &str) -> Result<i32, INIError> {
    let value = token.trim().to_ascii_uppercase();
    match value.as_str() {
        "LOW" => Ok(0),
        "MEDIUM" => Ok(1),
        "HIGH" => Ok(2),
        _ => INI::parse_int(token),
    }
}

fn parse_name_key_value(token: &str) -> Result<NameKeyType, INIError> {
    let value = parse_ascii_lower(token)?;
    if value.is_empty() || value == "none" {
        return Ok(NAMEKEY_INVALID);
    }
    Ok(name_key_generate(&value))
}

fn parse_weapon_slot_mask(tokens: &[&str]) -> u32 {
    let mut mask = 0u32;
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };
            if let Some(idx) = parse_weapon_slot_index(value) {
                let bit = 1u32 << idx;
                if clear {
                    mask &= !bit;
                } else {
                    mask |= bit;
                }
            } else {
                warn!("Unknown weapon slot token '{}'", value);
            }
        }
    }
    mask
}

fn parse_weapon_slot_index(token: &str) -> Option<usize> {
    let mut upper = token.trim().to_ascii_uppercase();
    if let Some(stripped) = upper.strip_prefix("WEAPONSLOT_") {
        upper = stripped.to_string();
    }
    match upper.as_str() {
        "PRIMARY" | "A" => Some(0),
        "SECONDARY" | "B" => Some(1),
        "TERTIARY" | "C" => Some(2),
        _ => None,
    }
}

fn parse_model_condition_flags_tokens(tokens: &[&str]) -> ModelConditionFlags {
    let mut flags = ModelConditionFlags::empty();
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };

            let normalized = value
                .trim()
                .to_ascii_uppercase()
                .trim_start_matches("MODELCONDITION_")
                .to_string();
            if normalized == "NONE" || normalized == "INVALID" {
                if clear {
                    continue;
                }
                flags = ModelConditionFlags::empty();
                continue;
            }

            match parse_model_condition_flag(value) {
                Some(flag) if clear => flags.remove(flag),
                Some(flag) => flags.insert(flag),
                None => warn!("Unknown model condition token '{}'", value),
            }
        }
    }
    flags
}

fn does_state_exist(states: &[ModelConditionInfo], flags: ModelConditionFlags) -> bool {
    states.iter().any(|state| {
        state
            .conditions_yes
            .iter()
            .any(|existing| *existing == flags)
    })
}

fn ensure_turret_slot(info: &mut ModelConditionInfo, index: usize) -> &mut TurretInfo {
    if info.turrets.len() <= index {
        info.turrets.resize_with(index + 1, TurretInfo::new);
    }
    &mut info.turrets[index]
}

fn add_public_bone(public_bones: &mut Vec<AsciiString>, bone_name: &str) {
    if bone_name.is_empty() || bone_name.eq_ignore_ascii_case("none") {
        return;
    }
    if public_bones
        .iter()
        .any(|bone| bone.as_str().eq_ignore_ascii_case(bone_name))
    {
        return;
    }
    public_bones.push(AsciiString::from(bone_name));
}

fn parse_bone_name_key(
    public_bones: &mut Vec<AsciiString>,
    tokens: &[&str],
) -> Result<NameKeyType, INIError> {
    let value = parse_ascii_lower(parse_required_value(tokens)?)?;
    add_public_bone(public_bones, &value);
    if value.is_empty() || value == "none" {
        return Ok(NAMEKEY_INVALID);
    }
    Ok(name_key_generate(&value))
}

fn parse_show_hide_sub_objects(
    info: &mut ModelConditionInfo,
    tokens: &[&str],
    hide: bool,
) -> Result<(), INIError> {
    let mut values = tokens
        .iter()
        .copied()
        .map(INI::parse_ascii_string)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if values.len() == 1 && values[0].eq_ignore_ascii_case("none") {
        info.hide_show_list.clear();
        return Ok(());
    }

    for sub_object in values.drain(..) {
        if let Some(existing) = info.hide_show_list.iter_mut().find(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(&sub_object)
        }) {
            existing.hide = hide;
            continue;
        }
        info.hide_show_list.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from(sub_object.as_str()),
            hide,
        });
    }

    Ok(())
}

fn parse_weapon_bone(
    tokens: &[&str],
    target: &mut [AsciiString; WEAPONSLOT_COUNT],
    public_bones: &mut Vec<AsciiString>,
) -> Result<(), INIError> {
    let slot_token = parse_required_value(tokens)?;
    let slot_index = parse_weapon_slot_index(slot_token).ok_or(INIError::InvalidData)?;
    let bone_token = tokens
        .iter()
        .copied()
        .skip(1)
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)?;
    let bone_name = parse_ascii_lower(bone_token)?;
    if bone_name == "none" {
        target[slot_index] = AsciiString::new();
        return Ok(());
    }
    target[slot_index] = AsciiString::from(bone_name.as_str());
    add_public_bone(public_bones, &bone_name);
    Ok(())
}

fn parse_animation(
    info: &mut ModelConditionInfo,
    tokens: &[&str],
    idle: bool,
) -> Result<(), INIError> {
    let anim_name = parse_ascii_lower(parse_required_value(tokens)?)?;
    let distance_covered = tokens
        .iter()
        .copied()
        .skip(1)
        .find(|token| !token.is_empty())
        .map(INI::parse_real)
        .transpose()?
        .unwrap_or(0.0);
    let repeat_count = tokens
        .iter()
        .copied()
        .skip(2)
        .find(|token| !token.is_empty())
        .map(INI::parse_int)
        .transpose()?
        .unwrap_or(1)
        .max(1) as usize;

    if (info.ini_read_flags & INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT) != 0 {
        info.ini_read_flags &= !(INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT
            | INI_READ_FLAG_GOT_IDLE_ANIMS
            | INI_READ_FLAG_GOT_NONIDLE_ANIMS);
        info.animations.clear();
    }

    if idle {
        info.ini_read_flags |= INI_READ_FLAG_GOT_IDLE_ANIMS;
    } else {
        info.ini_read_flags |= INI_READ_FLAG_GOT_NONIDLE_ANIMS;
    }

    if anim_name.is_empty() || anim_name.eq_ignore_ascii_case("none") {
        return Ok(());
    }

    for _ in 0..repeat_count {
        info.animations.push(W3DAnimationInfo::new(
            AsciiString::from(anim_name.as_str()),
            idle,
            distance_covered,
        ));
    }

    Ok(())
}

fn parse_anim_mode(token: &str) -> Result<AnimMode, INIError> {
    let value = token.trim().to_ascii_uppercase();
    match value.as_str() {
        "MANUAL" => Ok(AnimMode::Manual),
        "LOOP" => Ok(AnimMode::Loop),
        "ONCE" => Ok(AnimMode::Once),
        "LOOP_PING_PONG" | "LOOPPINGPONG" => Ok(AnimMode::LoopPingPong),
        "LOOP_BACKWARDS" | "LOOPBACKWARDS" => Ok(AnimMode::LoopBackwards),
        "ONCE_BACKWARDS" | "ONCEBACKWARDS" => Ok(AnimMode::OnceBackwards),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_ac_bits_flags(tokens: &[&str]) -> Result<u32, INIError> {
    let mut bits = 0u32;
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };
            let index = AC_BITS_NAMES
                .iter()
                .position(|name| name.eq_ignore_ascii_case(value))
                .ok_or(INIError::InvalidData)?;
            let mask = 1u32 << index;
            if clear {
                bits &= !mask;
            } else {
                bits |= mask;
            }
        }
    }
    Ok(bits)
}
