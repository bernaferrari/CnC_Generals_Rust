#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyCppUnit {
    pub relative_path: &'static str,
    pub module_name: &'static str,
    pub display_name: &'static str,
    pub group: &'static str,
    pub category: &'static str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LegacyLifecycle {
    pub init: bool,
    pub update: bool,
    pub shutdown: bool,
    pub system: bool,
    pub input: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyScreenDescriptor {
    pub name: &'static str,
    pub group: &'static str,
    pub lifecycle: LegacyLifecycle,
}

include!(concat!(env!("OUT_DIR"), "/legacy_registry.rs"));

pub fn groups() -> Vec<&'static str> {
    let mut groups = Vec::new();
    for unit in LEGACY_CPP_UNITS {
        if !groups.contains(&unit.group) {
            groups.push(unit.group);
        }
    }
    groups
}

pub fn units_in_group(group: &str) -> Vec<&'static LegacyCppUnit> {
    LEGACY_CPP_UNITS
        .iter()
        .filter(|unit| unit.group == group)
        .collect()
}

pub fn unit_by_path(path: &str) -> Option<&'static LegacyCppUnit> {
    LEGACY_CPP_UNITS
        .iter()
        .find(|unit| unit.relative_path == path)
}

pub fn screen_by_name(name: &str) -> Option<&'static LegacyScreenDescriptor> {
    LEGACY_SCREENS.iter().find(|screen| screen.name == name)
}
