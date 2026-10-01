/// C++ `PlayerList::crc` (PlayerList.cpp:411): playerCount then `xferSnapshot` each Player.
/// `XferCRC::xferSnapshot` calls `Player::crc`, not `Player::xfer`.
fn xfer_player_list_crc(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    use game_engine::common::system::snapshot::Snapshotable;

    let players = player_list();
    let list_guard = players.read().map_err(|_| XferStatus::InvalidData)?;
    let mut player_count = list_guard.get_player_count() as i32;
    xfer.xfer_int(&mut player_count)?;
    for idx in 0..player_count.max(0) {
        let player = list_guard
            .get_player(idx)
            .ok_or(XferStatus::InvalidData)?;
        let mut bridge = CommonXferBridge { inner: xfer };
        Snapshotable::crc(player, &mut bridge).map_err(|_| XferStatus::InvalidData)?;
    }
    Ok(())
}

/// C++ `PlayerList::xfer` version 1 (PlayerList.cpp:424): count + each `Player::xfer` v8.
fn xfer_player_list_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    use game_engine::common::system::snapshot::Snapshotable;

    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let indices = {
        let players = player_list();
        let list_guard = players.read().map_err(|_| XferStatus::InvalidData)?;
        let mut player_count = list_guard.get_player_count() as i32;
        xfer.xfer_int(&mut player_count)?;

        if player_count != list_guard.get_player_count() as i32 {
            return Err(XferStatus::InvalidData);
        }

        (0..player_count.max(0)).collect::<Vec<_>>()
    };

    for idx in indices {
        let result = crate::player::with_player_mut(idx, |player| {
            let mut bridge = CommonXferBridge { inner: xfer };
            Snapshotable::xfer(player, &mut bridge)
        })
        .ok_or(XferStatus::InvalidData)?;
        result.map_err(|_| XferStatus::InvalidData)?;
    }

    Ok(())
}
