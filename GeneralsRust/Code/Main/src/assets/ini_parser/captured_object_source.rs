//! Exact receiving Object grammar, before the convenience attribute projections.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapturedObjectKind {
    Object,
    ChildObject,
    ObjectReskin,
}

/// One original declaration/override, not a reconstruction of the merged row.
#[derive(Debug, Clone)]
pub(crate) struct CapturedObjectSource {
    pub(crate) kind: CapturedObjectKind,
    pub(crate) create_overrides: bool,
    pub(crate) filename: String,
    pub(crate) line: usize,
    pub(crate) name: String,
    properties: HashMap<String, String>,
}

impl CapturedObjectSource {
    pub(crate) fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
}

impl ObjectDefinition {
    pub(super) fn capture_object_source(
        &mut self,
        properties: HashMap<String, String>,
        header: &str,
        filename: &str,
        line: usize,
        create_overrides: bool,
    ) {
        let kind = match header.split_whitespace().next() {
            Some("ChildObject") => CapturedObjectKind::ChildObject,
            Some("ObjectReskin") => CapturedObjectKind::ObjectReskin,
            _ => CapturedObjectKind::Object,
        };
        self.source_declarations = Some(vec![CapturedObjectSource {
            kind,
            create_overrides,
            filename: filename.to_owned(),
            line,
            name: self.name.clone(),
            properties,
        }]);
    }

    pub(super) fn capture_object_override(&mut self, properties: &HashMap<String, String>) {
        // An uncaptured programmatic baseline cannot become validated merely
        // because a later override has exact grammar.
        let Some(history) = self.source_declarations.as_mut() else {
            return;
        };
        history.push(CapturedObjectSource {
            kind: CapturedObjectKind::Object,
            create_overrides: true,
            filename: "<Object CREATE_OVERRIDES>".into(),
            line: 0,
            name: self.name.clone(),
            properties: properties.clone(),
        });
    }

    pub(crate) fn inherited_source_declarations(
        parent: &Self,
        child: &Self,
    ) -> Option<Vec<CapturedObjectSource>> {
        let mut history = parent.source_declarations.as_ref()?.clone();
        history.extend_from_slice(child.source_declarations.as_ref()?);
        Some(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::thing::thing_template::ObjectIniFields;

    #[test]
    fn captured_source_keeps_cross_domain_grammar_and_reskin_identity() {
        let mut parser = IniParser::new();
        parser.parse_ini_content("Object Parent\nKindOf = VEHICLE\nBody = ActiveBody SharedTag\nMaxHealth = 40\nEnd\nDraw = W3DModelDraw SharedTag\nDefaultConditionState\nModel = None\nAnimation = A IDLE\nAnimation = B IDLE\nEnd\nEnd\nEnd\nObjectReskin Child Parent\nClientUpdate = SwayClientUpdate Forbidden\nEnd\nEnd", "receiving.ini").unwrap();
        let parent = parser.get_definition("Parent").unwrap();
        let source = &parent.source_declarations.as_ref().unwrap()[0];
        assert_eq!(source.kind, CapturedObjectKind::Object);
        assert_eq!(
            (source.filename.as_str(), source.line),
            ("receiving.ini", 1)
        );
        let fields = ObjectIniFields::new(source.properties());
        let declarations = fields.declarations().collect::<Vec<_>>();
        assert_eq!(
            declarations.iter().map(|d| d.name()).collect::<Vec<_>>(),
            ["KindOf", "Body", "Draw"]
        );
        assert_eq!(declarations[1].value(), "ActiveBody SharedTag");
        assert_eq!(declarations[2].value(), "W3DModelDraw SharedTag");
        assert!(
            source
                .properties()
                .get(&format!("{}.__body", declarations[2].key()))
                .unwrap()
                .contains("Animation = A IDLE\nAnimation = B IDLE")
        );
        let reskin = &parser
            .get_definition("Child")
            .unwrap()
            .source_declarations
            .as_ref()
            .unwrap()[0];
        assert_eq!(reskin.kind, CapturedObjectKind::ObjectReskin);
        assert!(!reskin.create_overrides);
        assert_eq!(
            ObjectIniFields::new(reskin.properties())
                .declarations()
                .next()
                .unwrap()
                .name(),
            "ClientUpdate"
        );
    }

    #[test]
    fn captured_source_retains_override_order_without_certifying_partial_baselines() {
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(
                "Object Target\nClientUpdate = SwayClientUpdate Original\nEnd\nEnd",
                "base.ini",
            )
            .unwrap();
        let mut receiving = parser.get_definition("Target").unwrap().clone();
        let peer = receiving.clone();
        let lines = [
            "RemoveModule Original",
            "AddModule",
            "ClientUpdate = SwayClientUpdate New",
            "End",
            "End",
            "End",
        ];
        let (properties, _) = ObjectIniFields::capture_body(&lines, 0);
        receiving.apply_create_override_properties(&properties);
        let history = receiving.source_declarations.as_ref().unwrap();
        assert_eq!(history.len(), 2);
        assert!(!history[0].create_overrides);
        assert!(history[1].create_overrides);
        assert_eq!(
            ObjectIniFields::new(history[1].properties())
                .declarations()
                .map(|d| d.name())
                .collect::<Vec<_>>(),
            ["RemoveModule", "AddModule"]
        );
        assert_eq!(peer.source_declarations.as_ref().unwrap().len(), 1);
        let mut uncaptured = ObjectDefinition::new("Target".into());
        uncaptured.apply_create_override_properties(&properties);
        assert!(uncaptured.source_declarations.is_none());
    }
}

impl ObjectDefinition {
    /// Validate exact module declarations using only the supplied receiving
    /// catalog. The opaque result cannot be used to instantiate runtime data.
    pub(crate) fn validate_receiving_module_source(
        &self,
        factory: &game_engine::common::thing::module_factory::ModuleFactory,
    ) -> std::result::Result<
        game_engine::common::thing::thing_template::ReceivingTemplateCandidate,
        String,
    > {
        use game_engine::common::ini::INILoadType;
        use game_engine::common::thing::thing_template::{
            ReceivingTemplateCandidate, ReceivingTemplateKind, ThingTemplate,
        };
        let history = self
            .source_declarations
            .as_ref()
            .filter(|history| !history.is_empty())
            .ok_or_else(|| {
                format!(
                    "{} has no complete captured Object declaration history",
                    self.name
                )
            })?;
        let mut candidate: Option<ReceivingTemplateCandidate> = None;
        for source in history {
            let kind = if source.kind == CapturedObjectKind::ObjectReskin {
                ReceivingTemplateKind::Reskin
            } else {
                ReceivingTemplateKind::Object
            };
            let load = if source.create_overrides {
                INILoadType::CreateOverrides
            } else {
                INILoadType::Overwrite
            };
            let next = if let Some(previous) = candidate.as_ref() {
                previous.prepare_named_receiving_candidate(
                    &source.name,
                    source.properties(),
                    load,
                    kind,
                )
            } else {
                ThingTemplate::new().prepare_named_receiving_candidate(
                    &source.name,
                    source.properties(),
                    load,
                    kind,
                    factory,
                )
            };
            candidate = Some(next.map_err(|error| {
                format!(
                    "{}:{} Object {}: {error}",
                    source.filename, source.line, source.name
                )
            })?);
        }
        candidate.ok_or_else(|| "Object declaration history is empty".to_owned())
    }
}
