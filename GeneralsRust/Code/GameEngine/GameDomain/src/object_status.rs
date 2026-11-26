//! Object-status token and modifier decoding from BitFlagsIO.

pub fn object_status_bit_by_name(name: &str) -> Option<u64> {
    STATUS_FLAG_LOOKUP
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, bit)| *bit)
}

/// Parse a sequence of status-bit tokens into a mask.
///
/// Mirrors the legacy BitFlags::parse semantics:
/// - `NONE` clears the mask and cannot be combined with other tokens.
/// - `+FLAG` and `-FLAG` incrementally set or clear bits; these cannot be
///   mixed with bare flag names.
/// - Bare flag names replace the mask (first token clears, subsequent add).
pub fn parse_object_status_bits<'a, I>(tokens: I) -> Result<u64, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut mask = 0;
    let mut found_normal = false;
    let mut found_add_or_sub = false;

    for raw in tokens {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }

        if token.eq_ignore_ascii_case("NONE") {
            if found_normal || found_add_or_sub {
                return Err("cannot mix NONE with other object status tokens".to_string());
            }
            mask = 0;
            return Ok(mask);
        }

        let (op, name) = match token.chars().next() {
            Some('+') | Some('-') => (token.chars().next(), &token[1..]),
            _ => (None, token),
        };

        // BitFlagsIO checks the listing mode before scanning the name,
        // including unknown/empty names in a conflicting mode.
        match op {
            Some('+') if found_normal => {
                return Err(
                    "cannot mix additive object status tokens with direct listings".to_string(),
                );
            }
            Some('-') if found_normal => {
                return Err(
                    "cannot mix subtractive object status tokens with direct listings".to_string(),
                );
            }
            None if found_add_or_sub => {
                return Err(
                    "cannot mix direct object status listings with +/- modifiers".to_string(),
                );
            }
            _ => {}
        }

        if name.is_empty() {
            return Err(format!("invalid object status token '{token}'"));
        }

        let flag = object_status_bit_by_name(name)
            .ok_or_else(|| format!("unknown object status '{name}'"))?;

        match op {
            Some('+') => {
                mask |= flag;
                found_add_or_sub = true;
            }
            Some('-') => {
                mask &= !flag;
                found_add_or_sub = true;
            }
            None => {
                if !found_normal {
                    mask = 0;
                }
                mask |= flag;
                found_normal = true;
            }
            Some(other) => {
                return Err(format!(
                    "unsupported object status modifier '{}{}'",
                    other, name
                ));
            }
        }
    }

    Ok(mask)
}

const STATUS_FLAG_LOOKUP: &[(&str, u64)] = &[
    ("DESTROYED", 1 << 1),
    ("CAN_ATTACK", 1 << 2),
    ("UNDER_CONSTRUCTION", 1 << 3),
    ("UNSELECTABLE", 1 << 4),
    ("NO_COLLISIONS", 1 << 5),
    ("NO_ATTACK", 1 << 6),
    ("AIRBORNE_TARGET", 1 << 7),
    ("PARACHUTING", 1 << 8),
    ("REPULSOR", 1 << 9),
    ("HIJACKED", 1 << 10),
    ("AFLAME", 1 << 11),
    ("BURNED", 1 << 12),
    ("WET", 1 << 13),
    ("IS_FIRING_WEAPON", 1 << 14),
    ("IS_BRAKING", 1 << 15),
    ("STEALTHED", 1 << 16),
    ("DETECTED", 1 << 17),
    ("CAN_STEALTH", 1 << 18),
    ("SOLD", 1 << 19),
    ("UNDERGOING_REPAIR", 1 << 20),
    ("RECONSTRUCTING", 1 << 21),
    ("MASKED", 1 << 22),
    ("IS_ATTACKING", 1 << 23),
    ("USING_ABILITY", 1 << 24),
    ("IS_AIMING_WEAPON", 1 << 25),
    ("NO_ATTACK_FROM_AI", 1 << 26),
    ("IGNORING_STEALTH", 1 << 27),
    ("IS_CARBOMB", 1 << 28),
    ("DECK_HEIGHT_OFFSET", 1 << 29),
    ("STATUS_RIDER1", 1 << 30),
    ("STATUS_RIDER2", 1 << 31),
    ("STATUS_RIDER3", 1 << 32),
    ("STATUS_RIDER4", 1 << 33),
    ("STATUS_RIDER5", 1 << 34),
    ("STATUS_RIDER6", 1 << 35),
    ("STATUS_RIDER7", 1 << 36),
    ("STATUS_RIDER8", 1 << 37),
    ("FAERIE_FIRE", 1 << 38),
    ("KILLING_SELF", 1 << 39),
    ("REASSIGN_PARKING", 1 << 40),
    ("BOOBY_TRAPPED", 1 << 41),
    ("IMMOBILE", 1 << 42),
    ("DISGUISED", 1 << 43),
    ("DEPLOYED", 1 << 44),
];
