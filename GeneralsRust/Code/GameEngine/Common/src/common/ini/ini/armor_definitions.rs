//! Ordered Armor INI directives, independent of mutable engine registries.
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ArmorDefinitions {
    definitions: HashMap<String, Vec<(String, f32)>>,
}

impl ArmorDefinitions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
    pub fn len(&self) -> usize {
        self.definitions.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[(String, f32)])> {
        self.definitions
            .iter()
            .map(|(name, rows)| (name.as_str(), rows.as_slice()))
    }
    /// C++ parseArmorDefinition clears the named template before each block.
    pub fn register_definition(&mut self, name: &str, rows: Vec<(String, f32)>) {
        self.definitions.insert(name.to_ascii_lowercase(), rows);
    }
}

/// One C++ parseArmorCoefficients operation. Preserve row order: DEFAULT can
/// overwrite earlier typed rows, and a later typed row can overwrite DEFAULT.
pub fn parse_coefficient_row(line: &str) -> Result<(String, f32), String> {
    let tokens: Vec<_> = line
        .split(|c: char| c.is_whitespace() || c == '=')
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.len() != 3 || !tokens[0].eq_ignore_ascii_case("Armor") {
        return Err(format!("Malformed armor assignment '{line}'"));
    }
    let value = tokens[2]
        .trim_end_matches('%')
        .parse::<f32>()
        .map_err(|_| format!("Invalid percentage '{}'", tokens[2]))?;
    Ok((tokens[1].to_ascii_uppercase(), value / 100.0))
}
