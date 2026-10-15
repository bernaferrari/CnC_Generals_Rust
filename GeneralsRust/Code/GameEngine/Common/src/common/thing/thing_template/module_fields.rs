//! CPP ThingTemplate.cpp512-816: module field admission and override operations.
//! Mask validation precedes inherited clearing and actual data construction.
//! Both supplied catalog and compatibility adapters execute this one algorithm.
use super::*;

const CPP_RESKIN_FIELDS: &[&str] = &[
    "Draw",
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
    "FenceWidth",
    "FenceXOffset",
    "MaxSimultaneousOfType",
    "MaxSimultaneousLinkKey",
];

fn is_reskin_property(key: &str) -> bool {
    if let Some(header) = key.strip_suffix(".__declaration_order") {
        return is_reskin_property(header);
    }
    let base = property_base_key(key);
    if CPP_RESKIN_FIELDS
        .iter()
        .any(|field| field.eq_ignore_ascii_case(base))
    {
        return true;
    }
    key.split_once('.')
        .map(|(header, _)| property_base_key(header).eq_ignore_ascii_case("Draw"))
        .unwrap_or(false)
}

pub(super) fn validate_receiving_reskin_fields(
    properties: &HashMap<String, String>,
) -> Result<(), String> {
    for field in ObjectIniFields::new(properties).declarations() {
        if !CPP_RESKIN_FIELDS
            .iter()
            .any(|name| name.eq_ignore_ascii_case(field.name()))
        {
            return Err(format!(
                "Field '{}' is not permitted in ObjectReskin",
                field.name()
            ));
        }
    }
    Ok(())
}

fn collect_module_body(
    properties: &HashMap<String, String>,
    header_key: &str,
) -> (String, HashMap<String, String>) {
    let prefix = format!("{}.", header_key);
    let mut fields = HashMap::new();
    let mut raw_body = String::new();
    for (key, value) in properties {
        // Actual top-level declarations are dispatched by the Object parser.
        // Explicit flattened module fields have no own ordinal and stay here.
        if key.ends_with(".__declaration_order")
            || properties.contains_key(&format!("{key}.__declaration_order"))
        {
            continue;
        }
        let Some(rest) = key.strip_prefix(&prefix) else {
            continue;
        };
        if rest == "__body" {
            raw_body = value.clone();
            continue;
        }
        if rest == "__declaration_order" || rest == "<raw>" || rest.starts_with("<raw>#") {
            continue;
        }
        fields.insert(rest.to_string(), value.clone());
    }
    if raw_body.is_empty() && !fields.is_empty() {
        raw_body = fields
            .iter()
            .map(|(key, value)| format!("{} = {}", key, value))
            .collect::<Vec<_>>()
            .join("\n");
    }
    (raw_body, fields)
}

fn module_data_from_body(
    module_name: &str,
    module_type: ModuleType,
    module_tag: &str,
    raw_body: &str,
    fields: HashMap<String, String>,
    module_factory: Option<&mut ModuleFactory>,
) -> Arc<dyn ModuleData> {
    if let Some(data) = try_factory_module_data(
        module_name,
        module_type,
        module_tag,
        raw_body,
        module_factory,
    ) {
        return data;
    }
    if !raw_body.trim().is_empty() || !fields.is_empty() {
        return Arc::new(CapturedModuleData::new(
            module_tag,
            raw_body.to_string(),
            fields,
        ));
    }
    let mut data = BaseModuleData::new();
    data.set_module_tag_name_key(NameKeyGenerator::name_to_key(module_tag));
    Arc::new(data)
}

fn try_factory_module_data(
    module_name: &str,
    module_type: ModuleType,
    module_tag: &str,
    raw_body: &str,
    module_factory: Option<&mut ModuleFactory>,
) -> Option<Arc<dyn ModuleData>> {
    if let Some(factory) = module_factory {
        return factory_module_data(factory, module_name, module_type, module_tag, raw_body);
    }
    let mut guard = get_module_factory().ok()?;
    factory_module_data(
        guard.as_mut()?,
        module_name,
        module_type,
        module_tag,
        raw_body,
    )
}

fn factory_module_data(
    factory: &mut ModuleFactory,
    module_name: &str,
    module_type: ModuleType,
    module_tag: &str,
    raw_body: &str,
) -> Option<Arc<dyn ModuleData>> {
    if !factory.has_module_data_proc(module_name, module_type) {
        return None;
    }
    let synthetic = if raw_body.trim().is_empty() {
        "End\n".to_string()
    } else {
        format!("{}\nEnd\n", raw_body.trim_end())
    };
    let mut ini = INI::new();
    ini.with_inline_source(&synthetic, |ini| {
        factory
            .try_new_module_data_from_ini(Some(ini), module_name, module_type, module_tag)
            .ok_or(crate::common::ini::INIError::InvalidData)
    })
    .ok()
}

impl ThingTemplate {
    fn add_module_from_property(
        &mut self,
        field_name: &str,
        property_key: &str,
        value: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        module_factory: &mut Option<&mut ModuleFactory>,
        strict: bool,
    ) -> Result<(), String> {
        let mut tokens = value.split_whitespace();
        let module_name = tokens
            .next()
            .ok_or_else(|| format!("Missing module name for '{}'", field_name))?;
        let module_tag = tokens
            .next()
            .ok_or_else(|| format!("Missing module tag for '{}'", module_name))?;

        let (module_type, fallback_mask) = match field_name {
            "Behavior" => (ModuleType::Behavior, ModuleInterfaceType::UPDATE),
            "Body" => (ModuleType::Behavior, ModuleInterfaceType::BODY),
            "Draw" => (ModuleType::Draw, ModuleInterfaceType::DRAW),
            "ClientUpdate" => (ModuleType::ClientUpdate, ModuleInterfaceType::CLIENT_UPDATE),
            other => return Err(format!("Unknown module field '{}'", other)),
        };

        let interface_mask = if strict {
            let factory = module_factory
                .as_deref()
                .ok_or("Strict module admission requires an explicit factory")?;
            if !factory.has_module_data_proc(module_name, module_type) {
                return Err(format!(
                    "Module data constructor '{module_name}' ({module_type:?}) not registered"
                ));
            }
            let mask = factory.find_module_interface_mask(module_name, module_type);
            if mask == ModuleInterfaceType::NONE {
                return Err(format!(
                    "Module '{module_name}' ({module_type:?}) has no registered interface mask"
                ));
            }
            mask
        } else {
            lookup_module_interface_mask_with_factory(
                module_name,
                module_type,
                fallback_mask,
                module_factory.as_deref(),
            )
        };
        // ThingTemplate.cpp:512-533 validates the field before clearing
        // inherited modules or invoking the registered data constructor.
        let is_body = interface_mask.0 & ModuleInterfaceType::BODY.0 != 0;
        if field_name == "Body" && !is_body {
            return Err("Only Body allowed here".to_string());
        }
        if field_name != "Body" && is_body {
            return Err("No Body allowed here".to_string());
        }

        if load_type != crate::common::ini::INILoadType::CreateOverrides {
            let new_name = AsciiString::from(module_name);
            let mask = interface_mask.0 as i32;
            let (is_trainable, disallowed, candidate) = self.gps_scrambler_inherit_flags();
            self.behavior_module_info.clear_copied_from_default_entries(
                mask,
                &new_name,
                is_trainable,
                disallowed,
                candidate,
            );
            self.draw_module_info.clear_copied_from_default_entries(
                mask,
                &new_name,
                is_trainable,
                disallowed,
                candidate,
            );
            self.client_update_module_info
                .clear_copied_from_default_entries(
                    mask,
                    &new_name,
                    is_trainable,
                    disallowed,
                    candidate,
                );
        }

        if self.module_parsing_mode == ModuleParseMode::AddRemoveReplace
            && !self.module_being_replaced_name.is_empty()
            && self.module_being_replaced_name.as_str() != module_name
        {
            return Err(format!(
                "ReplaceModule must replace modules with another module of the same type, but you are attempting to replace a {} with a {}",
                self.module_being_replaced_name, module_name
            ));
        }
        if self.module_parsing_mode == ModuleParseMode::AddRemoveReplace
            && !self.module_being_replaced_tag.is_empty()
            && self.module_being_replaced_tag.as_str() == module_tag
        {
            return Err(format!(
                "ReplaceModule must specify a new, unique tag for the replaced module, but you are not doing so for {} ({})",
                module_tag, self.module_being_replaced_name
            ));
        }

        let (raw_body, fields) = collect_module_body(properties, property_key);
        let data = if strict {
            let factory = module_factory
                .as_deref_mut()
                .ok_or("Strict module admission requires an explicit factory")?;
            let synthetic = format!("{}\nEnd\n", raw_body.trim_end());
            let mut ini = INI::new();
            let mut data = None;
            let mut error = None;
            ini.with_inline_source(&synthetic, |ini| {
                match factory.new_module_data_from_ini_strict(
                    Some(ini),
                    module_name,
                    module_type,
                    module_tag,
                ) {
                    Ok(created) => data = Some(created),
                    Err(message) => error = Some(message),
                }
                Ok(())
            })
            .map_err(|error| error.to_string())?;
            if let Some(error) = error {
                return Err(error);
            }
            data.ok_or_else(|| format!("Module constructor '{module_name}' produced no data"))?
        } else {
            module_data_from_body(
                module_name,
                module_type,
                module_tag,
                &raw_body,
                fields,
                module_factory.as_deref_mut(),
            )
        };
        if data.is_ai_module_data() {
            let target_info = match module_type {
                ModuleType::Behavior => &mut self.behavior_module_info,
                ModuleType::Draw => &mut self.draw_module_info,
                ModuleType::ClientUpdate => &mut self.client_update_module_info,
            };
            target_info.clear_ai_module_info();
        }
        if strict {
            let tag = AsciiString::from(module_tag);
            if self
                .behavior_module_info
                .get_nugget_with_tag(&tag)
                .is_some()
                || self.draw_module_info.get_nugget_with_tag(&tag).is_some()
                || self
                    .client_update_module_info
                    .get_nugget_with_tag(&tag)
                    .is_some()
            {
                return Err(format!(
                    "Duplicate module tag '{module_tag}' on '{}'",
                    self.get_name()
                ));
            }
        }
        let target_info = match module_type {
            ModuleType::Behavior => &mut self.behavior_module_info,
            ModuleType::Draw => &mut self.draw_module_info,
            ModuleType::ClientUpdate => &mut self.client_update_module_info,
        };
        let inheritable = self.module_parsing_mode == ModuleParseMode::Inheritable;
        let overrideable = self.module_parsing_mode == ModuleParseMode::OverrideableByLikeKind;
        target_info.add_module_info(
            AsciiString::from(module_name),
            AsciiString::from(module_tag),
            data,
            interface_mask.0 as i32,
            inheritable,
            overrideable,
        );

        Ok(())
    }

    pub(super) fn load_module_field(
        &mut self,
        field: &str,
        key: &str,
        value: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        module_factory: &mut Option<&mut ModuleFactory>,
        strict: bool,
    ) -> Result<(), String> {
        if is_module_object_field(field) {
            if load_type == crate::common::ini::INILoadType::CreateOverrides
                && self.module_parsing_mode != ModuleParseMode::AddRemoveReplace
            {
                return Err(format!(
                    "Use AddModule or ReplaceModule for '{field}' in override INI files"
                ));
            }
            self.add_module_from_property(
                field,
                key,
                value.trim(),
                properties,
                load_type,
                module_factory,
                strict,
            )
        } else {
            self.load_module_operation(
                field,
                key,
                value.trim(),
                properties,
                load_type,
                module_factory,
                strict,
            )
        }
    }

    fn remove_module_info(&mut self, tag: &AsciiString) -> Option<AsciiString> {
        if let Some(name) = self.behavior_module_info.clear_module_data_with_tag(tag) {
            return Some(name);
        }
        if let Some(name) = self.draw_module_info.clear_module_data_with_tag(tag) {
            return Some(name);
        }
        self.client_update_module_info
            .clear_module_data_with_tag(tag)
    }

    fn load_modules_from_override_prefix(
        &mut self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        prefix: &str,
        module_factory: &mut Option<&mut ModuleFactory>,
        strict: bool,
    ) -> Result<(), String> {
        let nested = ObjectIniFields::new(properties).nested_properties(prefix);
        // C++ wrappers recurse into the entire Object field table, not just
        // module declarations. Unknown Object fields must remain errors.
        self.parse_object_fields_with_policy(
            &nested,
            load_type,
            module_factory.as_deref_mut(),
            strict,
        )
    }

    fn load_module_operation(
        &mut self,
        field: &str,
        key: &str,
        value: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        module_factory: &mut Option<&mut ModuleFactory>,
        strict: bool,
    ) -> Result<(), String> {
        // ThingTemplate.cpp:715-816 permits wrappers only in NORMAL mode.
        if self.module_parsing_mode != ModuleParseMode::Normal {
            return Err(format!(
                "{field} cannot be nested inside another module operation"
            ));
        }
        let old_mode = self.module_parsing_mode;
        self.module_parsing_mode = match field {
            "InheritableModule" => ModuleParseMode::Inheritable,
            "OverrideableByLikeKind" => ModuleParseMode::OverrideableByLikeKind,
            _ => ModuleParseMode::AddRemoveReplace,
        };
        let result = (|| {
            if matches!(field, "RemoveModule" | "ReplaceModule") {
                let tag = AsciiString::from(value.split_whitespace().next().unwrap_or(""));
                let name = self
                    .remove_module_info(&tag)
                    .ok_or_else(|| format!("{field} tag '{tag}' was not found"))?;
                if field == "RemoveModule" {
                    return Ok(());
                }
                self.module_being_replaced_name = name;
                self.module_being_replaced_tag = tag;
            }
            self.load_modules_from_override_prefix(
                properties,
                load_type,
                key,
                module_factory,
                strict,
            )
        })();
        self.module_being_replaced_name.clear();
        self.module_being_replaced_tag.clear();
        self.module_parsing_mode = old_mode;
        result
    }

    /// C++ `getReskinFieldParse()`: Draw, Geometry*, Fence*, MaxSimultaneous*.
    pub fn parse_reskin_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.parse_reskin_fields_for_load(properties, crate::common::ini::INILoadType::Overwrite)
    }

    pub(in super::super) fn parse_reskin_fields_for_load(
        &mut self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
    ) -> Result<(), String> {
        self.parse_reskin_fields_for_load_with_factory(properties, load_type, None)
    }

    pub(in super::super) fn parse_reskin_fields_for_load_with_factory(
        &mut self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        module_factory: Option<&mut ModuleFactory>,
    ) -> Result<(), String> {
        let filtered: HashMap<String, String> = properties
            .iter()
            .filter(|(key, _)| is_reskin_property(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.parse_object_fields_for_load_with_factory(&filtered, load_type, module_factory)
    }
}

#[cfg(test)]
pub(super) fn lookup_module_interface_mask(
    module_name: &str,
    module_type: ModuleType,
    fallback_mask: ModuleInterfaceType,
) -> ModuleInterfaceType {
    lookup_module_interface_mask_with_factory(module_name, module_type, fallback_mask, None)
}

fn lookup_module_interface_mask_with_factory(
    module_name: &str,
    module_type: ModuleType,
    fallback_mask: ModuleInterfaceType,
    module_factory: Option<&ModuleFactory>,
) -> ModuleInterfaceType {
    if let Some(factory) = module_factory {
        let mask = factory.find_module_interface_mask(module_name, module_type);
        return if mask == ModuleInterfaceType::NONE {
            fallback_mask
        } else {
            mask
        };
    }
    if let Ok(factory_guard) = get_module_factory() {
        if let Some(factory) = factory_guard.as_ref() {
            let mask = factory.find_module_interface_mask(module_name, module_type);
            if mask != ModuleInterfaceType::NONE {
                return mask;
            }
        }
    }

    let mask = ModuleFactory::find_builtin_module_interface_mask(module_name, module_type);
    if mask != ModuleInterfaceType::NONE {
        mask
    } else {
        fallback_mask
    }
}
