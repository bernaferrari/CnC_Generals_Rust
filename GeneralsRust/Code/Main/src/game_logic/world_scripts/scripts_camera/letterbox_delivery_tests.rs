//! CPP ScriptActions.cpp:3749-3760 delivers Show/Hide for every authored action.
use super::*;

#[test]
fn repeated_letterbox_actions_keep_ordered_receiving_visibility_deliveries() {
    let (mut active, _, _) = world(120.);
    let (mut peer, _, _) = world(320.);
    let mut list = ScriptList::new();
    list.append_script(script(
        "RepeatedVisibility",
        vec![
            Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxEnd)),
            Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxEnd)),
            Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxBegin)),
            Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxEnd)),
        ],
        true,
    ));
    let hooks = active.mission_scripts.clone();
    let _ = run(&mut active, engine(hooks, list));
    assert_eq!(
        active.session_view.pending_letterbox_visibility,
        [false, false, true, false]
    );
    assert!(!active.cinematic_letterbox);
    assert!(peer.session_view.pending_letterbox_visibility.is_empty());
    peer.reset();
    assert_eq!(
        active.session_view.pending_letterbox_visibility,
        [false, false, true, false]
    );
    let delivered = std::mem::take(&mut active.session_view.pending_letterbox_visibility);
    assert_eq!(delivered, [false, false, true, false]);
    assert!(active.session_view.pending_letterbox_visibility.is_empty());
}

#[test]
fn reset_discards_undelivered_visibility_without_changing_peer() {
    let (mut active, _, _) = world(120.);
    let (mut peer, _, _) = world(320.);
    active
        .session_view
        .pending_letterbox_visibility
        .extend([true, false]);
    peer.session_view.pending_letterbox_visibility.push(true);
    active.reset();
    assert!(active.session_view.pending_letterbox_visibility.is_empty());
    assert_eq!(peer.session_view.pending_letterbox_visibility, [true]);
}
