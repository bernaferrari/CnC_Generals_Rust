/// C++ `GameLogic::xfer` → `xferSnapshot(TheCampaignManager)` (CampaignManager::xfer v5).
fn xfer_campaign_manager_snapshot(
    xfer: &mut dyn Xfer,
    state: &mut game_engine::System::CampaignManagerXferState,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 5;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer.xfer_ascii_string(&mut state.campaign)?;
    xfer.xfer_ascii_string(&mut state.mission)?;
    if version >= 2 {
        xfer.xfer_int(&mut state.rank_points)?;
    }
    if version >= 3 {
        // C++ `xferUser(&m_difficulty, sizeof(m_difficulty))` — 4-byte MSVC enum.
        xfer.xfer_int(&mut state.difficulty)?;
    }
    if version >= 4 {
        xfer.xfer_bool(&mut state.is_challenge)?;
        if state.is_challenge {
            if state.challenge_info.is_none() {
                state.challenge_info = Some(game_engine::System::ChallengeGameInfoXfer::default());
            }
            if let Some(info) = state.challenge_info.as_mut() {
                info.xfer(xfer)?;
            }
        } else if xfer.get_xfer_mode() == XferMode::Load {
            state.challenge_info = None;
        }
    }
    if version >= 5 {
        xfer.xfer_int(&mut state.generals_template)?;
    }

    Ok(())
}
