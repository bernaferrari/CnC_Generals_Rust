/// C++ `PlayerList::crc` (PlayerList.cpp:411): playerCount then `xferSnapshot` each Player.
/// `XferCRC::xferSnapshot` calls `Player::crc`, not `Player::xfer`.
fn xfer_player_list_crc(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    use game_engine::common::system::snapshot::Snapshotable;

    let player_arcs = {
        let players = player_list();
        let list_guard = players.read().map_err(|_| XferStatus::InvalidData)?;
        let mut player_count = list_guard.get_player_count() as i32;
        xfer.xfer_int(&mut player_count)?;
        (0..player_count.max(0))
            .map(|idx| {
                list_guard
                    .get_player(idx)
                    .cloned()
                    .ok_or(XferStatus::InvalidData)
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    for player_arc in player_arcs {
        let player = player_arc.read().map_err(|_| XferStatus::InvalidData)?;
        let mut bridge = CommonXferBridge { inner: xfer };
        Snapshotable::crc(&*player, &mut bridge).map_err(|_| XferStatus::InvalidData)?;
    }
    Ok(())
}

/// C++ `PlayerList::xfer` version 1 (PlayerList.cpp:424): count + each `Player::xfer` v8.
fn xfer_player_list_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    use game_engine::common::system::snapshot::Snapshotable;

    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // Drop the list lock before Player::xfer — rank/local-player queries re-enter the list.
    let player_arcs = {
        let players = player_list();
        let list_guard = players.read().map_err(|_| XferStatus::InvalidData)?;
        let mut player_count = list_guard.get_player_count() as i32;
        xfer.xfer_int(&mut player_count)?;

        if player_count != list_guard.get_player_count() as i32 {
            return Err(XferStatus::InvalidData);
        }

        (0..player_count.max(0))
            .map(|idx| {
                list_guard
                    .get_player(idx)
                    .cloned()
                    .ok_or(XferStatus::InvalidData)
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    for player_arc in player_arcs {
        let mut player = player_arc.write().map_err(|_| XferStatus::InvalidData)?;
        let mut bridge = CommonXferBridge { inner: xfer };
        Snapshotable::xfer(&mut *player, &mut bridge).map_err(|_| XferStatus::InvalidData)?;
    }

    Ok(())
}
