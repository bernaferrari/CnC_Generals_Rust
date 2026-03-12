// FILE: ini_parsers.rs
// Author: Ported from C++ by Claude Code
// Desc: Type-specific INI field parsers
//
// This module contains all the type-specific parsing functions for INI fields.
// Each function corresponds to a C++ parse function in INI.cpp

use super::ini::*;
use std::str::FromStr;

/// Scan integer from token string
pub fn scan_int(token: &str) -> INIResult<Int> {
    token
        .parse::<Int>()
        .map_err(|_| INIError::InvalidData)
}

/// Scan unsigned integer from token string
pub fn scan_unsigned_int(token: &str) -> INIResult<UnsignedInt> {
    token
        .parse::<UnsignedInt>()
        .map_err(|_| INIError::InvalidData)
}

/// Scan real (float) from token string
pub fn scan_real(token: &str) -> INIResult<Real> {
    token
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)
}

/// Scan percent to real (divide by 100)
pub fn scan_percent_to_real(token: &str) -> INIResult<Real> {
    let value = token
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(value / 100.0)
}

/// Scan boolean from token string (Yes/No)
pub fn scan_bool(token: &str) -> INIResult<Bool> {
    if token.eq_ignore_ascii_case("yes") {
        Ok(true)
    } else if token.eq_ignore_ascii_case("no") {
        Ok(false)
    } else {
        eprintln!("invalid boolean token {} -- expected Yes or No", token);
        Err(INIError::InvalidData)
    }
}

/// Scan index from token in name list
pub fn scan_index_list(token: &str, name_list: &[&str]) -> INIResult<Int> {
    if name_list.is_empty() {
        eprintln!("INTERNAL ERROR! scanIndexList, invalid name list");
        return Err(INIError::InvalidNameList);
    }

    for (index, name) in name_list.iter().enumerate() {
        if token.eq_ignore_ascii_case(name) {
            return Ok(index as Int);
        }
    }

    eprintln!("token {} is not a valid member of the index list", token);
    Err(INIError::InvalidData)
}

/// Scan lookup list to get value
pub fn scan_lookup_list(token: &str, lookup_list: &[LookupListRec]) -> INIResult<Int> {
    if lookup_list.is_empty() {
        eprintln!("INTERNAL ERROR! scanLookupList, invalid name list");
        return Err(INIError::InvalidNameList);
    }

    for lookup in lookup_list {
        if token.eq_ignore_ascii_case(&lookup.name) {
            return Ok(lookup.value);
        }
    }

    eprintln!("token {} is not a valid member of the lookup list", token);
    Err(INIError::InvalidData)
}

/// Parse unsigned byte (0-255)
pub fn parse_unsigned_byte(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_int(token)?;

    if value < 0 || value > 255 {
        eprintln!("Bad value INI::parseUnsignedByte");
        return Err(INIError::InvalidData);
    }

    unsafe {
        *(store as *mut UnsignedByte) = value as UnsignedByte;
    }
    Ok(())
}

/// Parse signed short (-32768 to 32767)
pub fn parse_short(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_int(token)?;

    if value < -32768 || value > 32767 {
        eprintln!("Bad value INI::parseShort");
        return Err(INIError::InvalidData);
    }

    unsafe {
        *(store as *mut Short) = value as Short;
    }
    Ok(())
}

/// Parse unsigned short (0-65535)
pub fn parse_unsigned_short(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_int(token)?;

    if value < 0 || value > 65535 {
        eprintln!("Bad value INI::parseUnsignedShort");
        return Err(INIError::InvalidData);
    }

    unsafe {
        *(store as *mut UnsignedShort) = value as UnsignedShort;
    }
    Ok(())
}

/// Parse integer
pub fn parse_int(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_int(token)?;

    unsafe {
        *(store as *mut Int) = value;
    }
    Ok(())
}

/// Parse unsigned integer
pub fn parse_unsigned_int(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_unsigned_int(token)?;

    unsafe {
        *(store as *mut UnsignedInt) = value;
    }
    Ok(())
}

/// Parse real (float)
pub fn parse_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_real(token)?;

    unsafe {
        *(store as *mut Real) = value;
    }
    Ok(())
}

/// Parse positive non-zero real
pub fn parse_positive_non_zero_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_real(token)?;

    if value <= 0.0 {
        eprintln!("invalid Real value {} -- expected > 0", value);
        return Err(INIError::InvalidData);
    }

    unsafe {
        *(store as *mut Real) = value;
    }
    Ok(())
}

/// Parse angle in degrees and convert to radians
pub fn parse_angle_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_real(token)?;

    let rads_per_degree = PI / 180.0;
    unsafe {
        *(store as *mut Real) = value * rads_per_degree;
    }
    Ok(())
}

/// Parse angular velocity in degrees/sec and convert to radians/frame
pub fn parse_angular_velocity_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_real(token)?;

    unsafe {
        *(store as *mut Real) = convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(value);
    }
    Ok(())
}

/// Parse boolean (Yes/No)
pub fn parse_bool(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_bool(token)?;

    unsafe {
        *(store as *mut Bool) = value;
    }
    Ok(())
}

/// Parse boolean as bit in Int32
pub fn parse_bit_in_int32(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let value = scan_bool(token)?;
    let mask = user_data as UnsignedInt;

    unsafe {
        let s = store as *mut UnsignedInt;
        if value {
            *s |= mask;
        } else {
            *s &= !mask;
        }
    }
    Ok(())
}

/// Parse ASCII string
pub fn parse_ascii_string(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let value = ini.get_next_ascii_string()?;

    unsafe {
        let s = store as *mut String;
        *s = value;
    }
    Ok(())
}

/// Parse quoted ASCII string (better quote handling)
pub fn parse_quoted_ascii_string(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let value = ini.get_next_quoted_ascii_string()?;

    unsafe {
        let s = store as *mut String;
        *s = value;
    }
    Ok(())
}

/// Parse ASCII string vector
pub fn parse_ascii_string_vector(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    unsafe {
        let vec = store as *mut Vec<String>;
        (*vec).clear();

        while let Some(token) = ini.get_next_token_or_null(None)? {
            (*vec).push(token.to_string());
        }
    }
    Ok(())
}

/// Parse ASCII string vector (append mode)
pub fn parse_ascii_string_vector_append(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    unsafe {
        let vec = store as *mut Vec<String>;
        // Don't clear - append mode

        while let Some(token) = ini.get_next_token_or_null(None)? {
            (*vec).push(token.to_string());
        }
    }
    Ok(())
}

/// Parse percent to real (0-100 to 0.0-1.0)
pub fn parse_percent_to_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(Some(ini.seps_percent()))?;
    let value = scan_percent_to_real(token)?;

    unsafe {
        *(store as *mut Real) = value;
    }
    Ok(())
}

/// Parse RGB color (R:100 G:114 B:245)
pub fn parse_rgb_color(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let names = ["R", "G", "B"];
    let mut colors = [0i32; 3];

    for i in 0..3 {
        let value_str = ini.get_next_sub_token(names[i])?;
        colors[i] = scan_int(&value_str)?;

        if colors[i] < 0 || colors[i] > 255 {
            return Err(INIError::InvalidData);
        }
    }

    unsafe {
        let color = store as *mut RGBColor;
        (*color).red = colors[0] as Real / 255.0;
        (*color).green = colors[1] as Real / 255.0;
        (*color).blue = colors[2] as Real / 255.0;
    }
    Ok(())
}

/// Parse RGBA color (R:100 G:114 B:245 [A:233])
pub fn parse_rgba_color_int(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let names = ["R", "G", "B", "A"];
    let mut colors = [0i32; 4];

    for i in 0..4 {
        match ini.get_next_token_or_null(Some(ini.seps_colon()))? {
            None => {
                if i < 3 {
                    return Err(INIError::InvalidData);
                } else {
                    // A is optional, default to 255
                    colors[i] = 255;
                }
            }
            Some(token) => {
                if !token.eq_ignore_ascii_case(names[i]) {
                    return Err(INIError::InvalidData);
                }
                let value_str = ini.get_next_token(Some(ini.seps_colon()))?;
                colors[i] = scan_int(value_str)?;
            }
        }

        if colors[i] < 0 || colors[i] > 255 {
            return Err(INIError::InvalidData);
        }
    }

    unsafe {
        let color = store as *mut RGBAColorInt;
        (*color).red = colors[0];
        (*color).green = colors[1];
        (*color).blue = colors[2];
        (*color).alpha = colors[3];
    }
    Ok(())
}

/// Parse 3D coordinate (X:400 Y:-214.3 Z:8.6)
pub fn parse_coord_3d(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let x_str = ini.get_next_sub_token("X")?;
    let y_str = ini.get_next_sub_token("Y")?;
    let z_str = ini.get_next_sub_token("Z")?;

    let x = scan_real(&x_str)?;
    let y = scan_real(&y_str)?;
    let z = scan_real(&z_str)?;

    unsafe {
        let coord = store as *mut Coord3D;
        (*coord).x = x;
        (*coord).y = y;
        (*coord).z = z;
    }
    Ok(())
}

/// Parse 2D coordinate (X:400 Y:-214.3)
pub fn parse_coord_2d(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let x_str = ini.get_next_sub_token("X")?;
    let y_str = ini.get_next_sub_token("Y")?;

    let x = scan_real(&x_str)?;
    let y = scan_real(&y_str)?;

    unsafe {
        let coord = store as *mut Coord2D;
        (*coord).x = x;
        (*coord).y = y;
    }
    Ok(())
}

/// Parse 2D integer coordinate (X:400 Y:-214)
pub fn parse_i_coord_2d(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let x_str = ini.get_next_sub_token("X")?;
    let y_str = ini.get_next_sub_token("Y")?;

    let x = scan_int(&x_str)?;
    let y = scan_int(&y_str)?;

    unsafe {
        let coord = store as *mut ICoord2D;
        (*coord).x = x;
        (*coord).y = y;
    }
    Ok(())
}

/// Parse 8-bit bitstring
pub fn parse_bit_string_8(
    ini: &mut INI,
    instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    let mut tmp: UnsignedInt = 0;
    parse_bit_string_32_impl(ini, &mut tmp, user_data)?;

    if tmp & 0xffffff00 != 0 {
        eprintln!("Bad bitstring list INI::parseBitString8");
        return Err(INIError::InvalidData);
    }

    unsafe {
        *(store as *mut UnsignedByte) = tmp as UnsignedByte;
    }
    Ok(())
}

/// Parse 32-bit bitstring
pub fn parse_bit_string_32(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    unsafe {
        let bits = store as *mut UnsignedInt;
        parse_bit_string_32_impl(ini, &mut *bits, user_data)
    }
}

/// Internal implementation for parsing bitstrings
fn parse_bit_string_32_impl(
    ini: &mut INI,
    bits: &mut UnsignedInt,
    user_data: *const u8,
) -> INIResult<()> {
    // user_data points to array of flag names
    // For safety, we'll need the caller to provide the list properly

    let mut found_normal = false;
    let mut found_add_or_sub = false;

    while let Some(token) = ini.get_next_token_or_null(None)? {
        if token.eq_ignore_ascii_case("NONE") {
            if found_normal || found_add_or_sub {
                eprintln!("you may not mix normal and +- ops in bitstring lists");
                return Err(INIError::InvalidNameList);
            }
            *bits = 0;
            break;
        }

        if token.starts_with('+') {
            if found_normal {
                eprintln!("you may not mix normal and +- ops in bitstring lists");
                return Err(INIError::InvalidNameList);
            }
            // Would need to scan index list here
            // *bits |= (1 << bit_index);
            found_add_or_sub = true;
        } else if token.starts_with('-') {
            if found_normal {
                eprintln!("you may not mix normal and +- ops in bitstring lists");
                return Err(INIError::InvalidNameList);
            }
            // Would need to scan index list here
            // *bits &= !(1 << bit_index);
            found_add_or_sub = true;
        } else {
            if found_add_or_sub {
                eprintln!("you may not mix normal and +- ops in bitstring lists");
                return Err(INIError::InvalidNameList);
            }

            if !found_normal {
                *bits = 0;
            }

            // Would need to scan index list here
            // *bits |= (1 << bit_index);
            found_normal = true;
        }
    }

    Ok(())
}

/// Parse index list
pub fn parse_index_list(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    // Would need name list from user_data
    // let value = scan_index_list(token, name_list)?;

    unsafe {
        // *(store as *mut Int) = value;
    }
    Ok(())
}

/// Parse byte-sized index list
pub fn parse_byte_sized_index_list(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    // Would need name list from user_data
    // let value = scan_index_list(token, name_list)?;

    // if value < 0 || value > 255 {
    //     eprintln!("Bad index list INI::parseByteSizedIndexList");
    //     return Err(INIError::InvalidData);
    // }

    unsafe {
        // *(store as *mut UnsignedByte) = value as UnsignedByte;
    }
    Ok(())
}

/// Parse lookup list
pub fn parse_lookup_list(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    // Would need lookup list from user_data
    // let value = scan_lookup_list(token, lookup_list)?;

    unsafe {
        // *(store as *mut Int) = value;
    }
    Ok(())
}

/// Parse duration in msec and convert to frames (Real)
pub fn parse_duration_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let val = scan_real(token)?;

    unsafe {
        *(store as *mut Real) = convert_duration_from_msecs_to_frames(val);
    }
    Ok(())
}

/// Parse duration in msec and convert to frames (UnsignedInt, rounding up)
pub fn parse_duration_unsigned_int(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let val = scan_unsigned_int(token)?;

    unsafe {
        *(store as *mut UnsignedInt) = convert_duration_from_msecs_to_frames(val as Real).ceil() as UnsignedInt;
    }
    Ok(())
}

/// Parse duration in msec and convert to frames (UnsignedShort, rounding up)
pub fn parse_duration_unsigned_short(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let val = scan_unsigned_int(token)?;

    unsafe {
        *(store as *mut UnsignedShort) = convert_duration_from_msecs_to_frames(val as Real).ceil() as UnsignedShort;
    }
    Ok(())
}

/// Parse velocity in dist/sec and convert to dist/frame
pub fn parse_velocity_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let val = scan_real(token)?;

    unsafe {
        *(store as *mut Real) = convert_velocity_in_secs_to_frames(val);
    }
    Ok(())
}

/// Parse acceleration in dist/sec^2 and convert to dist/frame^2
pub fn parse_acceleration_real(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    let token = ini.get_next_token(None)?;
    let val = scan_real(token)?;

    unsafe {
        *(store as *mut Real) = convert_acceleration_in_secs_to_frames(val);
    }
    Ok(())
}

/// Parse sounds list (comma/space separated)
pub fn parse_sounds_list(
    ini: &mut INI,
    _instance: *mut u8,
    store: *mut u8,
    _user_data: *const u8,
) -> INIResult<()> {
    unsafe {
        let vec = store as *mut Vec<String>;
        (*vec).clear();

        // Use custom separators for sounds list
        let seps = " \t,=";
        while let Some(token) = ini.get_next_token_or_null(Some(seps))? {
            (*vec).push(token.to_string());
        }
    }
    Ok(())
}
