//! UI delivery acknowledgement is transient receiving-view output, not save state.
use super::SessionView;

#[test]
fn synchronous_visibility_ack_is_consumed_once_with_ordered_pending_output() {
    let mut view = SessionView::default();
    let mut peer = SessionView::default();
    peer.pending_letterbox_visibility.push(true);
    view.letterbox_visibility_delivered_since_present = true;
    view.pending_letterbox_visibility
        .extend([false, true, false]);
    let (pending, delivered) = view.take_letterbox_visibility_output();
    assert_eq!(pending, [false, true, false]);
    assert!(
        delivered,
        "native projection must suppress a duplicate transition delivery"
    );
    assert_eq!(view.take_letterbox_visibility_output(), (Vec::new(), false));
    assert_eq!(peer.take_letterbox_visibility_output(), (vec![true], false));
}

#[test]
fn admitted_view_clears_delivery_ack_without_changing_saved_camera_contract() {
    let mut view = SessionView::default();
    let state = view.capture();
    view.pending_letterbox_visibility.push(true);
    view.letterbox_visibility_delivered_since_present = true;
    assert_eq!(
        view.capture(),
        state,
        "delivery output is not another saved camera state"
    );
    view.admit(state).unwrap();
    assert_eq!(view.take_letterbox_visibility_output(), (Vec::new(), false));
}

#[test]
fn acknowledged_visibility_consumption_preserves_later_manual_zoom_policy() {
    let mut view = SessionView::default();
    view.camera.native_camera.set_zoom_limited(false);
    view.letterbox_visibility_delivered_since_present = true;
    // A later manual command wins over the earlier authored letterbox command.
    view.camera.native_camera.set_zoom_limited(true);
    let (_, delivered) = view.take_letterbox_visibility_output();
    assert!(delivered);
    assert!(view.camera.native_camera.zoom_limited());
}
