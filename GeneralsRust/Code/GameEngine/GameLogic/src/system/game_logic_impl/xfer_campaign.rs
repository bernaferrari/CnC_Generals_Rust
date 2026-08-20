/// C++ `GameLogic::xfer` → `xferSnapshot(TheCampaignManager)` (CampaignManager::xfer v5).
fn xfer_campaign_manager_snapshot(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 5;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut state = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        game_engine::System::capture_campaign_manager_runtime()
    } else {
        game_engine::System::CampaignManagerXferState::default()
    };

    xfer.xfer_ascii_string(&mut state.campaign)?;
    xfer.xfer_ascii_string(&mut state.mission)?;
    if version >= 2 {
        xfer.xfer_int(&mut state.rank_points)?;
    }
    if version >= 3 {
        xfer.xfer_int(&mut state.difficulty)?;
    }
    if version >= 4 {
        xfer.xfer_bool(&mut state.is_challenge)?;
        if state.is_challenge {
            xfer.xfer_ascii_string(&mut state.challenge_map)?;
            xfer.xfer_int(&mut state.challenge_template)?;
        }
    }
    if version >= 5 {
        xfer.xfer_int(&mut state.generals_template)?;
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        game_engine::System::apply_campaign_manager_runtime(state);
    }
    Ok(())
}
