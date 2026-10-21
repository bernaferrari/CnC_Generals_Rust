//! Original ThingTemplate.cpp clearCopiedFromDefaultEntries + inherited flags.
use super::*;
use crate::assets::ReceivingObjectContent;

fn draw(class: &str, tag: &str, model: &str) -> String {
    if class == "W3DDefaultDraw" {
        format!("Draw = {class} {tag}\nEnd\n")
    } else {
        format!("Draw = {class} {tag}\nDefaultConditionState\nModel = {model}\nEnd\nEnd\n")
    }
}
fn tags(content: &ReceivingObjectContent, name: &str) -> (Vec<String>, Vec<String>) {
    let plan = content.module_content().admitted_template(name).unwrap();
    let typed = plan
        .draw_modules()
        .iter()
        .map(|entry| entry.module_tag.to_string())
        .collect();
    let projection = content.projections().get(name).unwrap();
    let models = projection
        .draw_modules
        .iter()
        .map(|entry| {
            entry
                .declaration
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_owned()
        })
        .collect();
    (typed, models)
}
fn assert_tags(content: &ReceivingObjectContent, name: &str, expected: &[&str]) {
    let (typed, models) = tags(content, name);
    let expected = expected
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    assert_eq!(typed, expected, "actual typed original-policy module order");
    assert_eq!(
        models, expected,
        "Draw body projection must retain exactly the typed survivors"
    );
}
#[test]
fn default_draw_projection_keeps_inheritable_body_in_actual_typed_order() {
    let source = format!(
        "Object DefaultThingTemplate\nInheritableModule\n{}End\nEnd\nObject Child\n{}End\n",
        draw("W3DModelDraw", "KeepTag", "KeepModel"),
        draw("W3DModelDraw", "ChildTag", "ChildModel")
    );
    let content =
        ReceivingObjectContent::from_sources([("owned-default.ini".into(), source)]).unwrap();
    assert_tags(&content, "Child", &["KeepTag", "ChildTag"]);
    let child = content.projections().get("Child").unwrap();
    let models = child.select_draw_models_for_conditions(0).unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.model_key.as_str())
            .collect::<Vec<_>>(),
        vec!["KeepModel", "ChildModel"],
        "both actual Draw condition bodies survive"
    );
    assert_tags(&content, "DefaultThingTemplate", &["KeepTag"]);
}
#[test]
fn parent_draw_projection_keeps_like_kind_survivor_but_drops_replaced_bodies() {
    let source = format!(
        "Object Parent\nKindOf = VEHICLE\n{}OverrideableByLikeKind\n{}End\nEnd\nChildObject Different Parent\n{}End\nChildObject Same Parent\n{}End\n",
        draw("W3DModelDraw", "DropTag", "DropModel"),
        draw("W3DModelDraw", "LikeTag", "LikeModel"),
        draw("W3DDefaultDraw", "DifferentTag", ""),
        draw("W3DModelDraw", "SameTag", "SameModel")
    );
    // Original ThingTemplate.cpp382-455: LikeKind retention also requires
    // a SCORE/VEHICLE/INFANTRY/PORTABLE_STRUCTURE candidate, not just a
    // different Draw class. Preserve the exact candidate=false countercase.
    let noncandidate_source = source.replace("KindOf = VEHICLE\n", "");
    let content =
        ReceivingObjectContent::from_sources([("owned-parent.ini".into(), source)]).unwrap();
    assert_tags(&content, "Parent", &["DropTag", "LikeTag"]);
    assert_tags(&content, "Different", &["LikeTag", "DifferentTag"]);
    assert_tags(&content, "Same", &["SameTag"]);
    let different = content.projections().get("Different").unwrap();
    let models = different.select_draw_models_for_conditions(0).unwrap();
    assert!(models.iter().any(|model| model.model_key == "LikeModel"));
    assert!(!models.iter().any(|model| model.model_key == "DropModel"));
    let same = content.projections().get("Same").unwrap();
    assert_eq!(
        same.select_draw_models_for_conditions(0)
            .unwrap()
            .iter()
            .map(|model| model.model_key.as_str())
            .collect::<Vec<_>>(),
        vec!["SameModel"]
    );
    let noncandidate = ReceivingObjectContent::from_sources([(
        "owned-noncandidate.ini".into(),
        noncandidate_source,
    )])
    .unwrap();
    assert_tags(&noncandidate, "Different", &["DifferentTag"]);
    assert_tags(&noncandidate, "Same", &["SameTag"]);
    assert_tags(&content, "Different", &["LikeTag", "DifferentTag"]);
}
