//! Ordered native client module-data capture and copied-parent resolution.
use super::*;

impl ObjectDefinition {
    pub(super) fn capture_client_modules(
        &mut self,
        properties: &HashMap<String, String>,
        create_overrides: bool,
    ) {
        self.client_module_operations.clear();
        self.capture_client_module_operations(properties, None, create_overrides);
        self.resolve_client_modules(&[], ClientModuleParseContext::default());
    }

    pub(super) fn capture_client_module_operations(
        &mut self,
        properties: &HashMap<String, String>,
        wrapper: Option<&str>,
        create_overrides: bool,
    ) {
        use game_engine::common::thing::thing_template::ObjectIniFields;
        let fields = ObjectIniFields::new(properties);
        for field in fields.declarations() {
            let name = field.name();
            let value = field.value();
            if name.eq_ignore_ascii_case("KindOf") {
                self.client_module_operations
                    .push(ClientModuleOperation::KindOf(value.to_owned()));
            }
            if name.eq_ignore_ascii_case("IsTrainable") {
                self.client_module_operations
                    .push(ClientModuleOperation::Trainable(
                        value.eq_ignore_ascii_case("yes"),
                    ));
            }
            if name == "RemoveModule" || name == "ReplaceModule" {
                self.client_module_operations
                    .push(ClientModuleOperation::Remove(value.to_owned()));
            }
            if let Some(kind) = match name {
                "Draw" => Some(ClientModuleKind::Draw),
                "ClientUpdate" => Some(ClientModuleKind::ClientUpdate),
                _ => None,
            } {
                let mut tokens = value.split_whitespace();
                if let Some(class_name) = tokens.next() {
                    self.client_module_operations
                        .push(ClientModuleOperation::Add {
                            declaration: ClientModuleDeclaration {
                                kind,
                                class_name: class_name.to_owned(),
                                module_tag: tokens.next().map(str::to_owned),
                                body: properties
                                    .get(&format!("{}.__body", field.key()))
                                    .cloned()
                                    .unwrap_or_default(),
                                inheritable: wrapper == Some("InheritableModule"),
                                overrideable_by_like_kind: wrapper
                                    == Some("OverrideableByLikeKind"),
                            },
                            clear_copied: !create_overrides,
                        });
                }
            }
            if matches!(
                name,
                "AddModule" | "InheritableModule" | "OverrideableByLikeKind" | "ReplaceModule"
            ) {
                self.capture_client_module_operations(
                    &fields.nested_properties(field.key()),
                    Some(name),
                    create_overrides,
                );
            }
        }
    }

    /// Replay C++ parseModuleName/clearCopiedFromDefaultEntries for client
    /// interfaces. Parent entries are copied; newly authored entries are not.
    pub(crate) fn resolve_client_modules(
        &mut self,
        parent: &[ClientModuleDeclaration],
        context: ClientModuleParseContext,
    ) {
        use game_engine::common::system::kind_of::KindOfMask;
        let mut kind_of = context.kind_of;
        let mut trainable = context.trainable;
        let mut resolved = parent
            .iter()
            .cloned()
            .map(|module| (module, true))
            .collect::<Vec<_>>();
        for operation in &self.client_module_operations {
            match operation {
                ClientModuleOperation::KindOf(value) => {
                    if let Ok(updated) = KindOfMask::parse_ini(kind_of, value) {
                        kind_of = updated;
                    }
                }
                ClientModuleOperation::Trainable(value) => {
                    trainable = *value;
                }
                ClientModuleOperation::Remove(tag) => {
                    resolved.retain(|(module, _)| module.module_tag.as_ref() != Some(tag))
                }
                ClientModuleOperation::Add {
                    declaration,
                    clear_copied,
                } => {
                    if *clear_copied {
                        let tokens = kind_of.to_string_list();
                        let has = |name: &str| {
                            tokens.iter().any(|token| token.eq_ignore_ascii_case(name))
                        };
                        let disallowed = [
                            "AIRCRAFT",
                            "SHRUBBERY",
                            "OPTIMIZED_TREE",
                            "STRUCTURE",
                            "DRAWABLE_ONLY",
                            "MOB_NEXUS",
                            "IGNORED_IN_GUI",
                            "CLEARED_BY_BUILD",
                            "DEFENSIVE_WALL",
                            "BALLISTIC_MISSILE",
                            "SUPPLY_SOURCE",
                            "BOAT",
                            "INERT",
                            "BRIDGE",
                            "LANDMARK_BRIDGE",
                            "BRIDGE_TOWER",
                        ]
                        .iter()
                        .any(|name| has(name));
                        let candidate = ["SCORE", "VEHICLE", "INFANTRY", "PORTABLE_STRUCTURE"]
                            .iter()
                            .any(|name| has(name));
                        resolved.retain(|(module, copied)| {
                            if !copied || module.kind != declaration.kind {
                                return true;
                            }
                            game_engine::common::thing::thing_template::CopiedModulePolicy {
                                name: &module.class_name,
                                tag: module.module_tag.as_deref().unwrap_or(""),
                                inheritable: module.inheritable,
                                overrideable_by_like_kind: module.overrideable_by_like_kind,
                            }
                            .survives_override(
                                &declaration.class_name,
                                trainable,
                                disallowed,
                                candidate,
                            )
                        });
                    }
                    resolved.push((declaration.clone(), false));
                }
            }
        }
        self.client_module_parse_context = ClientModuleParseContext { kind_of, trainable };
        self.client_modules = resolved.into_iter().map(|(module, _)| module).collect();
    }
}
