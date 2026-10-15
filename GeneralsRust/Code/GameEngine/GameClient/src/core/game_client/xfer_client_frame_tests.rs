use super::*;

#[test]
fn actual_client_xfer_preserves_separate_receiving_clock_and_wire() {
    use game_engine::ClientFrameContext;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    let mut source = GameClient::new().unwrap();
    source.set_frame(37);
    let mut bytes = Vec::new();
    source
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 3))
        .unwrap();
    let before = bytes.clone();
    let mut receiving = GameClient::new().unwrap();
    receiving.set_frame(900);
    let mut peer = GameClient::new().unwrap();
    peer.set_frame(1234);
    let mut load = XferLoad::new(Cursor::new(bytes), 3);
    assert_eq!(
        load.client_frame_context(),
        Err(CommonXferStatus::InvalidData)
    );
    receiving.xfer(&mut load).unwrap();
    assert_eq!(receiving.get_frame(), 37);
    assert_eq!(
        load.client_frame_context(),
        Ok(ClientFrameContext::Frame(37))
    );
    assert_eq!(peer.get_frame(), 1234);
    let mut after = Vec::new();
    receiving
        .xfer(&mut XferSave::new(Cursor::new(&mut after), 3))
        .unwrap();
    assert_eq!(
        after, before,
        "operation metadata never changes native wire bytes"
    );
}

#[test]
fn actual_runtime_common_adapter_forwards_explicit_clock_capability() {
    use game_engine::ClientFrameContext;
    let mut runtime = game_engine::XferLoad::new();
    let mut peer = game_engine::XferLoad::new();
    peer.set_client_frame_context(ClientFrameContext::Frame(1234))
        .unwrap();
    {
        let mut adapter = RuntimeCommonXferAdapter::new(&mut runtime);
        assert_eq!(
            adapter.client_frame_context(),
            Err(CommonXferStatus::InvalidData)
        );
        adapter
            .set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        assert_eq!(
            adapter.client_frame_context(),
            Ok(ClientFrameContext::NoPresentation)
        );
        adapter
            .set_client_frame_context(ClientFrameContext::Frame(0))
            .unwrap();
        assert_eq!(
            adapter.client_frame_context(),
            Ok(ClientFrameContext::Frame(0))
        );
    }
    assert_eq!(
        runtime.client_frame_context(),
        Ok(ClientFrameContext::Frame(0))
    );
    assert_eq!(
        peer.client_frame_context(),
        Ok(ClientFrameContext::Frame(1234))
    );
}
