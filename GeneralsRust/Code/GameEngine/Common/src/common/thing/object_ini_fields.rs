//! Borrowed view of the Common Object reader's captured properties.
//! Both template initialization and the live host override consume this lane.
use super::{
    indexed_subblock_field_order, object_properties_in_declaration_order, property_base_key,
    property_repeat_index,
};
use std::collections::HashMap;

pub struct ObjectIniFields<'a> {
    properties: &'a HashMap<String, String>,
}

pub struct ObjectIniField<'a> {
    properties: &'a HashMap<String, String>,
    key: &'a str,
    value: &'a str,
}

impl<'a> ObjectIniFields<'a> {
    pub fn new(properties: &'a HashMap<String, String>) -> Self {
        Self { properties }
    }

    pub fn declarations(&self) -> impl Iterator<Item = ObjectIniField<'a>> + use<'a> {
        let properties = self.properties;
        object_properties_in_declaration_order(properties)
            .into_iter()
            .map(move |(key, value)| ObjectIniField {
                properties,
                key,
                value,
            })
    }

    /// Existing explicit flattened maps have no authored sequence.
    pub fn has_captured_declarations(&self) -> bool {
        self.properties
            .keys()
            .any(|key| key.ends_with(".__declaration_order"))
    }

    /// Compatibility for callers providing the old flattened map surface.
    pub fn flattened_properties(&self) -> Vec<(&'a str, &'a str)> {
        let mut fields = self
            .properties
            .iter()
            .filter(|(key, _)| !key.ends_with(".__declaration_order"))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        fields.sort_by_key(|(key, _)| *key);
        fields
    }

    /// Wrappers use the same Object grammar as ordinary template initialization.
    pub fn nested_properties(&self, prefix: &str) -> HashMap<String, String> {
        if let Some(body) = self.properties.get(&format!("{prefix}.__body")) {
            let lines = body.lines().collect::<Vec<_>>();
            super::super::thing_factory::parse_object_block_properties(&lines, 0).0
        } else {
            let dotted = format!("{prefix}.");
            self.properties
                .iter()
                .filter_map(|(key, value)| {
                    let rest = key.strip_prefix(&dotted)?;
                    (rest != "__declaration_order").then(|| (rest.to_owned(), value.clone()))
                })
                .collect()
        }
    }
}

impl<'a> ObjectIniField<'a> {
    pub fn key(&self) -> &'a str {
        self.key
    }
    pub fn name(&self) -> &'a str {
        property_base_key(self.key)
    }
    pub fn value(&self) -> &'a str {
        self.value
    }

    pub fn subblock_fields(&self) -> Vec<(&'a str, &'a str)> {
        let prefix = match self.name() {
            "WeaponSet" | "ArmorSet" => {
                format!("{}{}", self.name(), property_repeat_index(self.key))
            }
            _ => self.key.to_owned(),
        };
        named_subblock_fields(self.properties, &prefix)
            .into_iter()
            .filter(|(name, _)| *name != "__body" && !name.starts_with("<raw>"))
            .map(|(name, value)| (property_base_key(name), value))
            .collect()
    }
}

pub(super) fn named_subblock_fields<'a>(
    properties: &'a HashMap<String, String>,
    prefix: &str,
) -> Vec<(&'a str, &'a str)> {
    let dotted = format!("{prefix}.");
    let mut fields = properties
        .iter()
        .filter_map(|(key, value)| {
            if key.ends_with(".__declaration_order")
                || properties.contains_key(&format!("{key}.__declaration_order"))
            {
                return None;
            }
            let field = key.strip_prefix(&dotted)?;
            Some((field, value.as_str()))
        })
        .collect::<Vec<_>>();
    fields.sort_by_key(|(field, _)| {
        let base = property_base_key(field);
        (
            property_repeat_index(field),
            indexed_subblock_field_order(base),
            base,
            *field,
        )
    });
    fields
}
