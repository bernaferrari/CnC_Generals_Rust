use game_client::gui::GameWindow;
use game_client::gui::game_window::WindowInstanceData;
use game_client::gui::window_draw_context::WindowDrawContext;
use game_engine_device::w3_d_device::common::system::w3_d_function_lexicon::load_w3d_tables;
use game_engine_device::w3_d_device::common::w3_d_function_lexicon::{
    FunctionLexicon, FunctionPtr, TableIndex,
};

fn observe_receiving_draw(
    _window: &GameWindow,
    _data: &WindowInstanceData,
    context: &mut WindowDrawContext<'_>,
) {
    context.display.clip_region = Some([1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn draw_callbacks_keep_their_type_and_borrow_only_the_receiving_context() {
    let entry = FunctionPtr::from_window_draw(observe_receiving_draw);
    assert!(entry.as_unit_fn().is_none());
    let callback = entry.as_window_draw().unwrap();
    let mut first = WindowDrawContext::default();
    let peer = WindowDrawContext::default();
    callback(
        &GameWindow::default(),
        &WindowInstanceData::default(),
        &mut first,
    );
    assert_eq!(first.display.clip_region, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(peer.display.clip_region, None);

    let mut lexicon = FunctionLexicon::new();
    load_w3d_tables(&mut lexicon);
    let entry = lexicon
        .get_table(TableIndex::GameWinDeviceDraw)
        .unwrap()
        .iter()
        .find(|entry| entry.name == "W3DNoDraw")
        .unwrap();
    let key = entry.key;
    let callback = lexicon
        .find_function_by_name("W3DNoDraw", TableIndex::GameWinDeviceDraw)
        .unwrap()
        .as_window_draw()
        .expect("W3D draw table must retain its borrowed callback signature");
    assert!(
        lexicon
            .find_function(key, TableIndex::GameWinDeviceDraw)
            .is_some()
    );
    callback(
        &GameWindow::default(),
        &WindowInstanceData::default(),
        &mut first,
    );
    assert_eq!(first.display.clip_region, Some([1.0, 2.0, 3.0, 4.0]));
}

#[test]
fn initializing_a_peer_lexicon_does_not_reset_this_owners_name_keys() {
    let mut first = FunctionLexicon::new();
    let original = first.key_for_name("FirstOwnedName");
    let mut peer = FunctionLexicon::new();
    peer.init();
    peer.key_for_name("PeerName");
    assert_eq!(first.key_for_name("FirstOwnedName"), original);
    peer.reset();
    drop(peer);
    assert_eq!(first.key_for_name("FirstOwnedName"), original);
}
