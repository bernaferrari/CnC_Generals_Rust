//! Authored armor definitions and damage adjustment.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use generals_game_domain::armor_ini::{ArmorRows, parse_coefficient_row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::damage::{DAMAGE_TYPE_COUNT, DamageType};

/// Per-damage-type coefficient table describing how incoming damage is reduced.
#[derive(Debug, Clone)]
pub struct ArmorTemplate {
    coefficients: [f32; DAMAGE_TYPE_COUNT],
}

impl Default for ArmorTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl ArmorTemplate {
    /// Create a template with all coefficients set to 1.0 (no mitigation).
    pub fn new() -> Self {
        Self {
            coefficients: [1.0; DAMAGE_TYPE_COUNT],
        }
    }

    /// Reset all coefficients back to the neutral 1.0 value.
    pub fn clear(&mut self) {
        self.coefficients.fill(1.0);
    }

    /// Set a default coefficient for every damage type.
    ///
    /// C++ ArmorTemplate::parseArmorCoefficients (Armor.cpp:66-71) stores the
    /// parsed percent unclamped. Result clamping happens in adjust_damage.
    pub fn set_default(&mut self, coefficient: f32) {
        self.coefficients.fill(coefficient);
    }

    /// Set a coefficient for a specific damage type.
    ///
    /// C++ ArmorTemplate::parseArmorCoefficients (Armor.cpp:75-76) stores pct
    /// unclamped. Only adjustDamage clamps the RESULT at 0 (Armor.cpp:50-53).
    pub fn set_coefficient(&mut self, damage_type: DamageType, coefficient: f32) {
        let index = damage_type as usize;
        if index < DAMAGE_TYPE_COUNT {
            self.coefficients[index] = coefficient;
        }
    }

    fn apply_coefficient(&mut self, name: &str, coefficient: f32) -> Result<(), ArmorParseError> {
        if name.eq_ignore_ascii_case("DEFAULT") {
            self.set_default(coefficient);
        } else {
            let damage_type = DamageType::from_str(name)
                .map_err(|_| ArmorParseError::UnknownDamageType(name.to_owned()))?;
            self.set_coefficient(damage_type, coefficient);
        }
        Ok(())
    }

    /// Apply the armor adjustment for a given damage payload.
    pub fn adjust_damage(&self, damage_type: DamageType, amount: f32) -> f32 {
        let index = damage_type as usize;
        if index >= DAMAGE_TYPE_COUNT {
            return if amount < 0.0 { 0.0 } else { amount };
        }
        match damage_type {
            DamageType::Unresistable | DamageType::SubdualUnresistable => amount,
            _ => {
                let scaled = amount * self.coefficients[index];
                if scaled < 0.0 { 0.0 } else { scaled }
            }
        }
    }
}

/// Lightweight armor instance referencing a shared template.
#[derive(Debug, Clone, Default)]
pub struct Armor {
    template: Option<Arc<ArmorTemplate>>,
}

impl Armor {
    /// Construct an armor instance from a shared template reference.
    pub fn from_template(template: Arc<ArmorTemplate>) -> Self {
        Self {
            template: Some(template),
        }
    }

    /// Remove the underlying template, reverting to neutral armour.
    pub fn clear(&mut self) {
        self.template = None;
    }

    /// Apply the armour modifiers to an incoming damage packet.
    pub fn adjust_damage(&self, damage_type: DamageType, amount: f32) -> f32 {
        self.template
            .as_ref()
            .map(|tmpl| tmpl.adjust_damage(damage_type, amount))
            .unwrap_or(amount)
    }

    /// Inspect the backing template if required by callers.
    pub fn template(&self) -> Option<&Arc<ArmorTemplate>> {
        self.template.as_ref()
    }
}

/// Immutable after admission; mutation methods operate on a detached content draft.
/// Owning sessions may share the completed catalog with their object templates.
#[derive(Debug, Clone, Default)]
pub struct ArmorDefinitions {
    templates: HashMap<String, Arc<ArmorTemplate>>,
}

impl ArmorDefinitions {
    pub fn new() -> Self {
        Self::default()
    }
    /// Preserve keys from Core's native body-definition admission.
    pub fn from_exact_templates(templates: HashMap<String, Arc<ArmorTemplate>>) -> Self {
        Self { templates }
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
    pub fn find_template(&self, name: &str) -> Option<&ArmorTemplate> {
        self.templates
            .get(&name.to_ascii_lowercase())
            .map(Arc::as_ref)
    }

    /// Retain an immutable row beyond a mutable catalog admission.
    pub fn shared_template(&self, name: &str) -> Option<Arc<ArmorTemplate>> {
        self.templates.get(&name.to_ascii_lowercase()).cloned()
    }

    /// Core native body definitions already use authored exact keys.
    pub fn shared_template_exact(&self, name: &str) -> Option<Arc<ArmorTemplate>> {
        self.templates.get(name).cloned()
    }

    pub fn register_template(&mut self, name: &str, template: ArmorTemplate) {
        self.register_shared_template(name, Arc::new(template));
    }

    pub fn register_shared_template(&mut self, name: &str, template: Arc<ArmorTemplate>) {
        self.templates.insert(name.to_ascii_lowercase(), template);
    }

    pub fn clear(&mut self) {
        self.templates.clear();
    }

    pub fn overlay_rows(&mut self, parsed: &ArmorRows) -> Result<(), ArmorParseError> {
        // Validate all changed definitions before committing any of them.
        let mut changed = Vec::new();
        for (name, rows) in parsed.iter() {
            let mut template = ArmorTemplate::new();
            for (damage_name, coefficient) in rows {
                template.apply_coefficient(damage_name, *coefficient)?;
            }
            changed.push((name.to_string(), Arc::new(template)));
        }
        self.templates.extend(changed);
        Ok(())
    }
    /// Armor-only loader: unrelated INI blocks never execute or publish effects.
    pub fn load_from_str(content: &str) -> Result<Self, ArmorParseError> {
        let parsed = parse_armor_templates(content)?;
        if parsed.is_empty() {
            return Err(ArmorParseError::Empty);
        }
        let mut definitions = Self::new();
        for (name, template) in parsed {
            definitions.register_template(&name, template);
        }
        Ok(definitions)
    }
}

#[derive(Debug, Error)]
pub enum ArmorParseError {
    #[error("Armor parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("Unknown damage type '{0}' in armor definition")]
    UnknownDamageType(String),
    #[error("Armor block '{0}' missing END statement")]
    UnterminatedBlock(String),
    #[error("Armor definition file contained no templates")]
    Empty,
}

/// Read typed templates in source order, including repeated declarations.
pub fn parse_armor_templates(
    content: &str,
) -> Result<Vec<(String, ArmorTemplate)>, ArmorParseError> {
    let mut results = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_template = ArmorTemplate::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        // Common INI dispatch matches declaration tokens case-insensitively;
        // its separators include tabs and '='. Keep this Armor-only envelope
        // strict about unrelated blocks while sharing coefficient parsing.
        let armor_rest = line
            .get(..5)
            .filter(|token| token.eq_ignore_ascii_case("Armor"))
            .and_then(|_| line.get(5..))
            .filter(|rest| {
                rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('=')
            });
        if let Some(rest) = armor_rest {
            if rest.trim_start().starts_with('=') {
                // This is an armor assignment ("Armor = ..."), not a block header.
            } else {
                if current_name.is_some() {
                    return Err(ArmorParseError::Parse {
                        line: line_no,
                        message: "Nested Armor block encountered".into(),
                    });
                }
                let name = rest.trim();
                if name.is_empty() {
                    return Err(ArmorParseError::Parse {
                        line: line_no,
                        message: "Missing armor name".into(),
                    });
                }
                current_name = Some(name.to_string());
                current_template = ArmorTemplate::new();
                continue;
            }
        }

        if line.eq_ignore_ascii_case("End") {
            let name = current_name.take().ok_or_else(|| ArmorParseError::Parse {
                line: line_no,
                message: "END without matching Armor block".into(),
            })?;
            let template = std::mem::take(&mut current_template);
            results.push((name, template));
            continue;
        }

        if armor_rest.is_none() {
            return Err(ArmorParseError::Parse {
                line: line_no,
                message: format!("Unrecognised line '{line}'"),
            });
        }

        if current_name.is_none() {
            return Err(ArmorParseError::Parse {
                line: line_no,
                message: "Armor assignment outside Armor block".into(),
            });
        }

        let (damage_name, coefficient) =
            parse_coefficient_row(line).map_err(|message| ArmorParseError::Parse {
                line: line_no,
                message,
            })?;
        current_template.apply_coefficient(&damage_name, coefficient)?;
    }

    if let Some(name) = current_name {
        return Err(ArmorParseError::UnterminatedBlock(name));
    }

    Ok(results)
}

/// One selectable Object INI armor and damage FX row.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ArmorSet {
    /// C++ `ArmorSetFlags` mask (`ArmorSetType` bits).
    #[serde(default)]
    pub conditions: u8,
    /// Armor.ini template name (`Armor = ...`).
    #[serde(default)]
    pub armor: Option<String>,
    /// DamageFX.ini name (`DamageFX = ...`).
    #[serde(default)]
    pub damage_fx: Option<String>,
}

/// C++ `ArmorSetType` bits (`ArmorSet.h`).
pub const ARMORSET_VETERAN: u8 = 1 << 0;
pub const ARMORSET_ELITE: u8 = 1 << 1;
pub const ARMORSET_HERO: u8 = 1 << 2;
pub const ARMORSET_PLAYER_UPGRADE: u8 = 1 << 3;
pub const ARMORSET_WEAK_VERSUS_BASEDEFENSES: u8 = 1 << 4;
pub const ARMORSET_SECOND_LIFE: u8 = 1 << 5;
pub const ARMORSET_CRATE_UPGRADE_ONE: u8 = 1 << 6;
pub const ARMORSET_CRATE_UPGRADE_TWO: u8 = 1 << 7;

/// Parse one Object INI `ArmorSet Conditions` token into an `ArmorSetType` bit.
pub fn armor_set_condition_bit(token: &str) -> Option<u8> {
    match token
        .trim()
        .trim_matches(',')
        .to_ascii_uppercase()
        .replace('-', "_")
        .as_str()
    {
        "" | "NONE" => None,
        "VETERAN" => Some(ARMORSET_VETERAN),
        "ELITE" => Some(ARMORSET_ELITE),
        "HERO" | "HEROIC" => Some(ARMORSET_HERO),
        "PLAYER_UPGRADE" => Some(ARMORSET_PLAYER_UPGRADE),
        "WEAK_VERSUS_BASEDEFENSES" => Some(ARMORSET_WEAK_VERSUS_BASEDEFENSES),
        "SECOND_LIFE" => Some(ARMORSET_SECOND_LIFE),
        "CRATE_UPGRADE_ONE" | "CRATEUPGRADE_ONE" => Some(ARMORSET_CRATE_UPGRADE_ONE),
        "CRATE_UPGRADE_TWO" | "CRATEUPGRADE_TWO" => Some(ARMORSET_CRATE_UPGRADE_TWO),
        _ => None,
    }
}

/// C++ SparseMatchFinder keeps the first authored row when scores tie.
pub fn best_armor_set(sets: &[ArmorSet], flags: u8) -> Option<&ArmorSet> {
    let mut best: Option<&ArmorSet> = None;
    let mut best_yes = 0u32;
    let mut best_extra = u32::MAX;
    for set in sets {
        let yes = u32::from(set.conditions & flags).count_ones();
        let extra = u32::from(set.conditions & !flags).count_ones();
        if yes > best_yes || (yes >= best_yes && extra < best_extra) {
            best = Some(set);
            best_yes = yes;
            best_extra = extra;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_armor_loader_accepts_original_case_and_tab_tokens_but_rejects_bad_headers() {
        // C++ INI.cpp findBlockParse uses stricmp; m_seps includes whitespace
        // and '='. Armor.cpp parseArmorCoefficients also compares Default
        // case-insensitively. Every variant is the same actual named block.
        for source in [
            "Armor Mixed\n Armor = Default 33%\nEnd\n",
            "aRmOr Mixed\n aRmOr = dEfAuLt 33%\neNd\n",
            "Armor\tMixed\n\tArmor\t=\tDefault\t33%\nEnd\n",
            "ARMOR\tMixed\n armor=DEFAULT\t33%\nEND\n",
        ] {
            let catalog = ArmorDefinitions::load_from_str(source).unwrap();
            assert_eq!(catalog.len(), 1);
            assert_eq!(
                catalog
                    .find_template("mixed")
                    .unwrap()
                    .adjust_damage(DamageType::Explosion, 1.0)
                    .to_bits(),
                (33.0f32 / 100.0).to_bits()
            );
        }
        for source in [
            "Armor\nEnd\n",
            "ArMoR\t \nEnd\n",
            "Armored WrongToken\nEnd\n",
            "Armor Outer\n Armor Inner\nEnd\nEnd\n",
            "Object UnrelatedBlock\nEnd\n",
        ] {
            assert!(
                matches!(
                    ArmorDefinitions::load_from_str(source),
                    Err(ArmorParseError::Parse { .. })
                ),
                "Armor-only admission rejects malformed/unrelated header: {source:?}"
            );
        }
    }

    #[test]
    fn invalid_owned_armor_overlay_is_transactional() {
        let mut catalog =
            ArmorDefinitions::load_from_str("Armor Shared\n Armor = DEFAULT 50%\nEnd\n").unwrap();
        let mut rows = ArmorRows::new();
        rows.register_definition("Shared", vec![("DEFAULT".into(), 0.1)]);
        rows.register_definition("Bad", vec![("NOT_A_DAMAGE_TYPE".into(), 0.5)]);
        assert!(catalog.overlay_rows(&rows).is_err());
        assert_eq!(catalog.len(), 1);
        assert_eq!(
            catalog
                .find_template("Shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            50.0
        );
        assert!(catalog.find_template("Bad").is_none());
    }

    #[test]
    fn parse_simple_armor_block() {
        let ini = "Armor Test\n  Armor = DEFAULT 100%\n  Armor = FLAME 50%\nEnd\n";
        let parsed = parse_armor_templates(ini).expect("failed to parse armor");
        assert_eq!(parsed.len(), 1);
        let (name, template) = parsed.into_iter().next().unwrap();
        assert_eq!(name, "Test");
        let damage = template.adjust_damage(DamageType::Flame, 10.0);
        assert!((damage - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cpp_parity_armor_stores_unclamped_coefficients_clamps_result() {
        // C++ Armor.cpp:60-81 stores coefficients as parsed; Armor.cpp:42-55
        // ArmorTemplate::adjustDamage multiplies then clamps the RESULT at 0.
        let mut template = ArmorTemplate::new();
        template.set_coefficient(DamageType::Flame, -0.5);

        let healed = template.adjust_damage(DamageType::Flame, -5.0);
        assert!(
            (healed - 2.5).abs() < f32::EPSILON,
            "negative amount * negative coeff must stay positive ({healed})"
        );

        template.set_coefficient(DamageType::Flame, 0.5);
        let clamped = template.adjust_damage(DamageType::Flame, -5.0);
        assert_eq!(clamped, 0.0, "negative result must clamp at 0");

        template.set_default(-0.25);
        let scaled = template.adjust_damage(DamageType::SmallArms, -8.0);
        assert!(
            (scaled - 2.0).abs() < f32::EPSILON,
            "set_default must also store a negative coefficient ({scaled})"
        );
    }
}
