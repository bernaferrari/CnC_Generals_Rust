use super::*;
use crate::common::ini::INILoadType;

fn data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BaseModuleData::new())
}
fn valid(ini: Option<&mut INI>) -> Result<Box<dyn ModuleData>, String> {
    Ok(data(ini))
}
fn rejected(_: Option<&mut INI>) -> Result<Box<dyn ModuleData>, String> {
    Err("authored module body rejected".into())
}

fn factory() -> ModuleFactory {
    let mut factory = ModuleFactory::new();
    for (name, kind, mask) in [
        ("StrictDraw", ModuleType::Draw, ModuleInterfaceType::DRAW),
        (
            "StrictOtherDraw",
            ModuleType::Draw,
            ModuleInterfaceType::DRAW,
        ),
        (
            "StrictClient",
            ModuleType::ClientUpdate,
            ModuleInterfaceType::CLIENT_UPDATE,
        ),
        (
            "StrictBody",
            ModuleType::Behavior,
            ModuleInterfaceType::BODY,
        ),
    ] {
        factory.add_module_internal(None, Some(data), kind, name, mask);
        factory.register_fallible_data_constructor(name, kind, valid);
    }
    factory
}
fn fields(body: &str) -> HashMap<String, String> {
    let text = format!("{body}\nEnd");
    let lines = text.lines().collect::<Vec<_>>();
    ObjectIniFields::capture_body(&lines, 0).0
}
fn prepare(
    base: &ThingTemplate,
    factory: &ModuleFactory,
    body: &str,
) -> Result<ReceivingTemplateCandidate, String> {
    base.prepare_named_receiving_candidate(
        "StrictReceiving",
        &fields(body),
        INILoadType::Overwrite,
        ReceivingTemplateKind::Object,
        factory,
    )
}
fn tags(template: &ThingTemplate) -> Vec<String> {
    template
        .get_draw_module_info()
        .iter()
        .map(|m| m.module_tag.to_string())
        .collect()
}

#[test]
fn strict_candidate_rejects_missing_and_cross_domain_duplicate_tags() {
    let factory = factory();
    let base = ThingTemplate::new();
    for body in [
        "Draw = StrictDraw\nEnd",
        "Draw = StrictDraw Same\nEnd\nClientUpdate = StrictClient Same\nEnd",
        "Body = StrictBody Same\nEnd\nDraw = StrictDraw Same\nEnd",
    ] {
        assert!(prepare(&base, &factory, body).is_err(), "{body}");
    }
    assert_eq!(base.get_draw_module_info().get_count(), 0);
}

#[test]
fn strict_candidate_rejects_unknown_class_and_unvalidated_legacy_constructor() {
    let mut factory = factory();
    let base = ThingTemplate::new();
    assert!(prepare(&base, &factory, "Draw = UnknownClass Tag\nEnd").is_err());
    factory.add_module_internal(
        None,
        Some(data),
        ModuleType::Draw,
        "LegacyDraw",
        ModuleInterfaceType::DRAW,
    );
    let error = prepare(&base, &factory, "Draw = LegacyDraw Tag\nEnd")
        .err()
        .unwrap();
    assert!(error.contains("fallible"), "{error}");
}

#[test]
fn strict_candidate_propagates_typed_constructor_error_without_fallback() {
    let mut factory = factory();
    factory.register_fallible_data_constructor("StrictDraw", ModuleType::Draw, rejected);
    let error = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw Tag\nEnd",
    )
    .err()
    .unwrap();
    assert!(error.contains("authored module body rejected"), "{error}");
}

#[test]
fn strict_candidate_rejects_missing_remove_replace_and_nested_wrappers() {
    let factory = factory();
    let base = ThingTemplate::new();
    for body in [
        "RemoveModule Missing",
        "ReplaceModule Missing\nDraw = StrictDraw New\nEnd\nEnd",
        "AddModule\nAddModule\nDraw = StrictDraw Tag\nEnd\nEnd\nEnd",
    ] {
        assert!(prepare(&base, &factory, body).is_err(), "{body}");
    }
}

#[test]
fn strict_candidate_replace_requires_same_class_and_new_tag() {
    let factory = factory();
    let base = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw Old\nEnd",
    )
    .unwrap()
    .into_parts()
    .0;
    for body in [
        "ReplaceModule Old\nDraw = StrictOtherDraw New\nEnd\nEnd",
        "ReplaceModule Old\nDraw = StrictDraw Old\nEnd\nEnd",
    ] {
        assert!(prepare(&base, &factory, body).is_err(), "{body}");
    }
    let candidate = prepare(
        &base,
        &factory,
        "ReplaceModule Old\nDraw = StrictDraw New\nEnd\nEnd",
    )
    .unwrap();
    assert_eq!(tags(candidate.template()), ["New"]);
    assert_eq!(tags(&base), ["Old"]);
}

#[test]
fn strict_candidate_reskin_rejects_forbidden_fields_instead_of_filtering() {
    let factory = factory();
    let base = ThingTemplate::new();
    for body in [
        "KindOf = VEHICLE",
        "ClientUpdate = StrictClient Tag\nEnd",
        "AddModule\nDraw = StrictDraw Tag\nEnd\nEnd",
    ] {
        assert!(
            base.prepare_named_receiving_candidate(
                "StrictReceiving",
                &fields(body),
                INILoadType::Overwrite,
                ReceivingTemplateKind::Reskin,
                &factory
            )
            .is_err(),
            "{body}"
        );
    }
    let candidate = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &fields("Draw = StrictDraw Tag\nEnd"),
            INILoadType::Overwrite,
            ReceivingTemplateKind::Reskin,
            &factory,
        )
        .unwrap();
    assert_eq!(tags(candidate.template()), ["Tag"]);
}

#[test]
fn strict_candidate_loadtype_controls_copied_clearing_and_override_grammar() {
    let factory = factory();
    let base = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw Old\nEnd",
    )
    .unwrap()
    .into_parts()
    .0;
    let properties = fields("AddModule\nDraw = StrictDraw New\nEnd\nEnd");
    let normal = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &properties,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &factory,
        )
        .unwrap();
    let overridden = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &properties,
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &factory,
        )
        .unwrap();
    assert_eq!(tags(normal.template()), ["New"]);
    assert_eq!(tags(overridden.template()), ["Old", "New"]);
    assert!(
        base.prepare_named_receiving_candidate(
            "StrictReceiving",
            &fields("Draw = StrictDraw Invalid\nEnd"),
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &factory
        )
        .is_err()
    );
}

#[test]
fn strict_candidate_failed_peer_admission_preserves_receiver_and_catalog() {
    let factory_a = factory();
    let mut factory_b = factory();
    factory_b.register_fallible_data_constructor(
        "StrictClient",
        ModuleType::ClientUpdate,
        rejected,
    );
    let base = prepare(
        &ThingTemplate::new(),
        &factory_a,
        "Draw = StrictDraw Stable\nEnd",
    )
    .unwrap()
    .into_parts()
    .0;
    let input = fields("AddModule\nClientUpdate = StrictClient Added\nEnd\nEnd");
    assert!(
        base.prepare_named_receiving_candidate(
            "StrictReceiving",
            &input,
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &factory_b
        )
        .is_err()
    );
    let a = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &input,
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &factory_a,
        )
        .unwrap();
    assert_eq!(a.template().get_client_update_module_info().get_count(), 1);
    assert_eq!(base.get_client_update_module_info().get_count(), 0);
    assert_eq!(tags(&base), ["Stable"]);
}

#[test]
fn strict_candidate_child_identity_is_set_before_parsing_without_changing_parent() {
    let factory = factory();
    let base = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw Old\nEnd",
    )
    .unwrap()
    .into_parts()
    .0;
    let child = base
        .prepare_named_receiving_candidate(
            "ChildReceiving",
            &fields("Draw = StrictDraw New\nEnd"),
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &factory,
        )
        .unwrap();
    assert_eq!(child.template().get_name().as_str(), "ChildReceiving");
    assert_eq!(base.get_name().as_str(), "StrictReceiving");
    assert_eq!(tags(&base), ["Old"]);
    assert_eq!(tags(child.template()), ["New"]);
}

#[test]
fn strict_candidate_new_override_marks_copied_entries_without_allocating_tag_identity() {
    let factory = factory();
    let (base, owner) = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw Old\nEnd",
    )
    .unwrap()
    .into_parts();
    assert!(
        !base
            .get_draw_module_info()
            .iter()
            .next()
            .unwrap()
            .copied_from_default
    );
    let candidate = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &fields("AddModule\nClientUpdate = StrictClient Added\nEnd\nEnd"),
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &owner,
        )
        .unwrap();
    let old = candidate
        .template()
        .get_draw_module_info()
        .iter()
        .next()
        .unwrap();
    assert!(old.copied_from_default);
    assert!(
        !candidate
            .template()
            .get_client_update_module_info()
            .iter()
            .next()
            .unwrap()
            .copied_from_default
    );
    let key = candidate
        .template()
        .get_client_update_module_info()
        .get_nth_data(0)
        .unwrap()
        .get_module_tag_name_key();
    assert_eq!(key, crate::common::name_key_generator::NAMEKEY_INVALID);
    // Unkeyed validation data remains private to the capability. Exact tag
    // strings, rather than provisional numeric identities, drive validation.
    let repeated = base
        .prepare_named_receiving_candidate(
            "StrictReceiving",
            &fields("AddModule\nClientUpdate = StrictClient Added\nEnd\nEnd"),
            INILoadType::CreateOverrides,
            ReceivingTemplateKind::Object,
            &owner,
        )
        .unwrap();
    assert_eq!(
        key,
        repeated
            .template()
            .get_client_update_module_info()
            .get_nth_data(0)
            .unwrap()
            .get_module_tag_name_key()
    );
    assert_eq!(base.get_client_update_module_info().get_count(), 0);
}

#[test]
fn strict_candidate_tag_validation_does_not_advance_ambient_name_key_sequence() {
    use crate::common::name_key_generator::NameKeyGenerator;
    let before = NameKeyGenerator::name_to_key("strict_candidate_tag_sequence_before");
    let factory = factory();
    let candidate = prepare(
        &ThingTemplate::new(),
        &factory,
        "Draw = StrictDraw DetachedTagNeverAdmitted\nEnd",
    )
    .unwrap();
    assert_eq!(
        candidate.summary().draw[0].module_tag.as_str(),
        "DetachedTagNeverAdmitted"
    );
    assert!(
        prepare(
            &ThingTemplate::new(),
            &factory,
            "Draw = StrictDraw FailedTagNeverAdmitted\nEnd\nRemoveModule Missing"
        )
        .is_err()
    );
    let after = NameKeyGenerator::name_to_key("strict_candidate_tag_sequence_after");
    assert_eq!(after, before.wrapping_add(1));
}

#[derive(Debug)]
struct AiData(BaseModuleData);
impl Snapshotable for AiData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.0.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.0.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.0.load_post_process()
    }
}
impl ModuleData for AiData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.0.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.0.get_module_tag_name_key()
    }
    fn is_ai_module_data(&self) -> bool {
        true
    }
}
fn ai_data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(AiData(BaseModuleData::new()))
}
fn valid_ai(ini: Option<&mut INI>) -> Result<Box<dyn ModuleData>, String> {
    Ok(ai_data(ini))
}

#[test]
fn strict_candidate_ai_replacement_clears_previous_ai_before_tag_validation() {
    let mut factory = factory();
    factory.add_module_internal(
        None,
        Some(ai_data),
        ModuleType::Behavior,
        "StrictAI",
        ModuleInterfaceType::UPDATE,
    );
    factory.register_fallible_data_constructor("StrictAI", ModuleType::Behavior, valid_ai);
    // CPP constructs data, clears previous AI, then addModuleInfo validates tags.
    let candidate = prepare(
        &ThingTemplate::new(),
        &factory,
        "Behavior = StrictAI Same\nEnd\nBehavior = StrictAI Same\nEnd",
    )
    .unwrap();
    assert_eq!(candidate.summary().behavior.len(), 1);
    assert_eq!(candidate.summary().behavior[0].module_tag.as_str(), "Same");
    assert!(
        prepare(
            &ThingTemplate::new(),
            &factory,
            "Behavior = StrictAI Same\nEnd\nDraw = StrictDraw Same\nEnd"
        )
        .is_err()
    );
}

#[test]
fn strict_candidate_body_never_trusts_authored_field_as_missing_catalog_mask() {
    let mut factory = factory();
    factory.add_module_internal(
        None,
        Some(data),
        ModuleType::Behavior,
        "NoMask",
        ModuleInterfaceType::NONE,
    );
    factory.register_fallible_data_constructor("NoMask", ModuleType::Behavior, valid);
    let error = prepare(&ThingTemplate::new(), &factory, "Body = NoMask Tag\nEnd")
        .err()
        .unwrap();
    assert!(error.contains("registered interface mask"), "{error}");
    assert!(
        prepare(
            &ThingTemplate::new(),
            &factory,
            "Body = StrictDraw Tag\nEnd"
        )
        .is_err()
    );
    assert!(
        prepare(
            &ThingTemplate::new(),
            &factory,
            "Behavior = StrictBody Tag\nEnd"
        )
        .is_err()
    );
}
