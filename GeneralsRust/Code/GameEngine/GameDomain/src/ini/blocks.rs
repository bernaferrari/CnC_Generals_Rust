//! Ordered INI blocks used by startup and map catalogs.

/// An ordered sequence of INI assignments.
///
/// Most consumers only need C++'s usual last-assignment-wins behavior and can
/// collapse this into a `HashMap`. Weapon veterancy fields are different:
/// `VeterancyFireFX`, `VeterancyProjectileExhaust`, and their OCL/detonation
/// equivalents may legitimately occur more than once in one block. Retaining
/// source order is therefore necessary to reproduce `Weapon.cpp`'s parser.
pub type OrderedIniProperties = Vec<(String, String)>;
/// Parse a block of INI text without discarding repeated assignments.
///
/// Returns `(block_type, block_name, ordered_properties)` tuples. Callers that
/// need only the last assignment can collect the properties into a map.
pub fn parse_ini_sections_ordered(content: &str) -> Vec<(String, String, OrderedIniProperties)> {
    let mut sections: Vec<(String, String, OrderedIniProperties)> = Vec::new();
    let mut current_type: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut current_props: OrderedIniProperties = Vec::new();
    let mut depth: u32 = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty()
            || trimmed.starts_with(';')
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
        {
            continue;
        }

        // Skip [Section] headers (e.g. [WeaponSystem], [ScienceSystem])
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            continue;
        }

        // Detect block headers: "Weapon", "Upgrade", "Science", "Object", etc.
        if is_ini_block_header(trimmed) && depth == 0 {
            // Save previous section if any
            if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
                if !current_props.is_empty() {
                    sections.push((t, n, std::mem::take(&mut current_props)));
                }
            }

            if let Some((block_type, block_name)) = parse_block_header(trimmed) {
                current_type = Some(block_type);
                current_name = Some(block_name);
                current_props.clear();
                depth = 1;
            }
            continue;
        }

        // Track nested End keywords (e.g. WeaponSet blocks inside objects)
        if trimmed.eq_ignore_ascii_case("End") {
            if depth > 1 {
                // Nested block terminator - just decrease depth
                depth -= 1;
                continue;
            }
            // Top-level End - finalize the current section
            if depth == 1 {
                if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
                    sections.push((t, n, std::mem::take(&mut current_props)));
                }
                depth = 0;
            }
            continue;
        }

        // Track nested block opens (Behavior, Draw, WeaponSet, etc.)
        if depth > 0 && is_nested_block_header(trimmed) {
            depth += 1;
            continue;
        }

        // Parse key = value inside a block
        if depth > 0 {
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let mut value = trimmed[eq_pos + 1..].trim().to_string();

                // Remove quotes
                if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value = value[1..value.len() - 1].to_string();
                }

                // Handle inline comments
                value = strip_inline_comment(&value).to_string();

                if !key.is_empty() {
                    current_props.push((key, value));
                }
            }
        }
    }

    // Handle last section if file doesn't end with End
    if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
        if !current_props.is_empty() {
            sections.push((t, n, current_props));
        }
    }

    sections
}

/// Check if a line is a top-level INI block header like "Weapon", "Upgrade", "Science".
fn is_ini_block_header(line: &str) -> bool {
    let first_word = line.split_whitespace().next().unwrap_or("");
    matches!(
        first_word.to_lowercase().as_str(),
        "weapon" | "upgrade" | "science" | "object" | "childobject" | "objectreskin"
    )
}

/// Check if a line starts a nested block (e.g., "Behavior", "Draw", "WeaponSet").
fn is_nested_block_header(line: &str) -> bool {
    let first_word = line.split_whitespace().next().unwrap_or("");
    matches!(
        first_word.to_lowercase().as_str(),
        "behavior"
            | "body"
            | "draw"
            | "weaponset"
            | "armorset"
            | "locomotorset"
            | "contain"
            | "physics"
            | "sound"
            | "clientupdate"
            | "moduletag"
            | "conditionstate"
            | "transitionstate"
            | "anim"
            | "particlesystem"
            | "fxlist"
            | "objectcreationlist"
            | "script"
            | "playertemplate"
            | "commandset"
            | "commandbutton"
            | "specialpower"
    )
}

/// Parse a block header line into (type, name).
fn parse_block_header(line: &str) -> Option<(String, String)> {
    let mut tokens = line.split_whitespace();
    let block_type = tokens.next()?.to_string();
    let block_name = tokens.next()?.to_string();
    Some((block_type, block_name))
}

/// Strip inline comments from a value string.
fn strip_inline_comment(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b';' | b'#' if !in_single && !in_double => return value[..i].trim_end().to_string(),
            b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return value[..i].trim_end().to_string();
            }
            _ => {}
        }
        i += 1;
    }

    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ini_block_header() {
        assert!(is_ini_block_header("Weapon TestWeapon"));
        assert!(is_ini_block_header("Upgrade TestUpgrade"));
        assert!(is_ini_block_header("Science TestScience"));
        assert!(is_ini_block_header("Object TestObject"));
        assert!(!is_ini_block_header("Behavior AIUpdate"));
        assert!(!is_ini_block_header("WeaponSet"));
    }

    #[test]
    fn test_strip_inline_comment() {
        assert_eq!(strip_inline_comment("Explosion ; comment"), "Explosion");
        assert_eq!(strip_inline_comment("100.0 // another comment"), "100.0");
        assert_eq!(strip_inline_comment("NoComment"), "NoComment");
    }
}
