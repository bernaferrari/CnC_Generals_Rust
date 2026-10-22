//! Actual Client icon dispatch, independent of the standalone Core flag.
//! Explicit CPU overlay component fixture, not Object/native factory evidence.
use super::*;
use crate::drawable::BasicDrawable;
use crate::drawable::drawable::{ICoord2D, IRegion2D};
fn actor(client: &mut GameClient) -> DrawableId {
    let mut drawable = BasicDrawable::new(DrawableId::INVALID);
    drawable.overlay_data.health_region =
        Some(IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32)));
    drawable.set_caption_text("ReceivingCaption");
    drawable.overlay_data.visible = true;
    client.register_drawable(Box::new(drawable)).unwrap()
}
fn visible(client: &mut GameClient, id: DrawableId) -> bool {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .overlay_data
        .visible
}
#[test]
fn actual_client_icon_dispatch_uses_only_its_admitted_policy() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "actual_client_icon_dispatch_uses_only_its_admitted_policy",
    ) {
        return;
    }
    let mut a = GameClient::new().unwrap();
    let mut b = GameClient::new().unwrap();
    let aid = actor(&mut a);
    let bid = actor(&mut b);
    assert_eq!(aid, bid);
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(false);
    a.admit_draw_icon_ui(true);
    b.admit_draw_icon_ui(false);
    a.draw_drawable_icon_ui();
    b.draw_drawable_icon_ui();
    assert!(
        visible(&mut a, aid),
        "native admitted true ignores foreign Core false"
    );
    assert!(!visible(&mut b, bid));
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(true);
    b.draw_drawable_icon_ui();
    assert!(
        !visible(&mut b, bid),
        "foreign Core true cannot re-enable receiving false"
    );
    a.admit_draw_icon_ui(false);
    a.draw_drawable_icon_ui();
    assert!(!visible(&mut a, aid));
    assert!(!b.receiving_draw_icon_ui);
}
