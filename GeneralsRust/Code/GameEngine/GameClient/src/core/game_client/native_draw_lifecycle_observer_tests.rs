use super::*;

#[test]
fn native_mirror_observer_admits_creation_and_both_transform_phases_before_first_draw() {
    let mut client = GameClient::new().unwrap();
    let mut entry = tests::presentation_drawable_sync_for_test(
        42,
        19,
        "ObserverOwned",
        true,
        false,
        [3., 5., 7.],
        0.25,
    );
    entry.model_condition_bits = 1 << game_engine::common::bit_flags::ModelConditionFlags::MOVING;
    let mut phases = Vec::new();
    let result = client
        .sync_presentation_drawables_observed([entry.clone()], 321, |event| match event {
            PresentationDrawableMutation::Creating {
                initial_transform,
                sync_ms,
                ..
            } => {
                assert_eq!(sync_ms, 321);
                assert_eq!(initial_transform, crate::drawable::Matrix4::identity());
                phases.push("create");
            }
            PresentationDrawableMutation::TransformInstalled { transform, .. } => {
                // Custom Matrix4 is row-major and this native mirror already uses
                // Main's Y-up: no source-coordinate swap belongs in this fact.
                assert_eq!(
                    [
                        transform.elements[0][3],
                        transform.elements[1][3],
                        transform.elements[2][3]
                    ],
                    [3., 5., 7.]
                );
                phases.push("transform");
            }
            PresentationDrawableMutation::ConditionsMarked { bits, .. } => {
                assert_ne!(bits & entry.model_condition_bits, 0);
                phases.push("conditions");
            }
            PresentationDrawableMutation::Destroyed { .. } => {
                panic!("new binding cannot retire itself")
            }
        })
        .unwrap();
    assert_eq!(result, (1, 0, 0));
    assert_eq!(phases, ["create", "transform", "transform", "conditions"]);
    phases.clear();
    entry.position = [11., 13., 17.];
    client
        .sync_presentation_drawables_observed([entry], 999, |event| match event {
            PresentationDrawableMutation::TransformInstalled { .. } => phases.push("transform"),
            PresentationDrawableMutation::ConditionsMarked { .. } => phases.push("conditions"),
            _ => panic!("ordinary sync must not recreate/retire the receiving binding"),
        })
        .unwrap();
    assert_eq!(phases, ["transform", "transform", "conditions"]);
    phases.clear();
    client
        .sync_presentation_drawables_observed(
            Vec::<PresentationDrawableSync>::new(),
            1000,
            |event| match event {
                PresentationDrawableMutation::Destroyed {
                    object_id,
                    host_epoch,
                } => {
                    assert_eq!((host_epoch, object_id), (19, 42));
                    phases.push("destroy");
                }
                _ => panic!("empty roster only retires existing bindings"),
            },
        )
        .unwrap();
    assert_eq!(phases, ["destroy"]);
}
