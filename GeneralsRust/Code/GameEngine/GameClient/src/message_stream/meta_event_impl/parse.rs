// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn parse_block_field(ini: &mut INI) -> INIResult<Option<(String, Vec<String>)>> {
    ini.read_line()?;
    if ini.is_eof() {
        return Err(INIError::EndOfFile);
    }
    let tokens = ini.get_line_tokens();
    let Some(key) = tokens.first() else {
        return Ok(None);
    };
    if key.eq_ignore_ascii_case("End") {
        return Ok(Some((String::from("End"), Vec::new())));
    }
    let values: Vec<String> = tokens
        .iter()
        .skip(1)
        .filter(|token| **token != "=")
        .map(|token| (*token).to_string())
        .collect();
    Ok(Some((key.to_string(), values)))
}

fn parse_transition(value: &str) -> Transition {
    match value.to_ascii_uppercase().as_str() {
        "UP" => Transition::Up,
        "DOUBLEDOWN" => Transition::DoubleDown,
        _ => Transition::Down,
    }
}

fn parse_mod_state(value: &str) -> u32 {
    match value.to_ascii_uppercase().as_str() {
        "CTRL" => MOD_CTRL,
        "ALT" => MOD_ALT,
        "SHIFT" => MOD_SHIFT,
        "CTRL_ALT" => MOD_CTRL | MOD_ALT,
        "SHIFT_CTRL" => MOD_SHIFT | MOD_CTRL,
        "SHIFT_ALT" => MOD_SHIFT | MOD_ALT,
        "SHIFT_ALT_CTRL" => MOD_SHIFT | MOD_ALT | MOD_CTRL,
        _ => 0,
    }
}

fn parse_usable_in(values: &[String]) -> u32 {
    let mut flags = 0;
    for value in values {
        match value.to_ascii_uppercase().as_str() {
            "SHELL" => flags |= COMMANDUSABLE_SHELL,
            "GAME" => flags |= COMMANDUSABLE_GAME,
            _ => {}
        }
    }
    flags
}

fn lookup_key_code(name: &str) -> Option<u32> {
    match name.to_ascii_uppercase().as_str() {
        "KEY_ESC" => Some(0x1B),
        "KEY_BACKSPACE" => Some(0x08),
        "KEY_ENTER" => Some(0x0D),
        "KEY_SPACE" => Some(0x20),
        "KEY_TAB" => Some(0x09),
        "KEY_F1" => Some(0x70),
        "KEY_F2" => Some(0x71),
        "KEY_F3" => Some(0x72),
        "KEY_F4" => Some(0x73),
        "KEY_F5" => Some(0x74),
        "KEY_F6" => Some(0x75),
        "KEY_F7" => Some(0x76),
        "KEY_F8" => Some(0x77),
        "KEY_F9" => Some(0x78),
        "KEY_F10" => Some(0x79),
        "KEY_F11" => Some(0x7A),
        "KEY_F12" => Some(0x7B),
        "KEY_A" => Some(0x41),
        "KEY_B" => Some(0x42),
        "KEY_C" => Some(0x43),
        "KEY_D" => Some(0x44),
        "KEY_E" => Some(0x45),
        "KEY_F" => Some(0x46),
        "KEY_G" => Some(0x47),
        "KEY_H" => Some(0x48),
        "KEY_I" => Some(0x49),
        "KEY_J" => Some(0x4A),
        "KEY_K" => Some(0x4B),
        "KEY_L" => Some(0x4C),
        "KEY_M" => Some(0x4D),
        "KEY_N" => Some(0x4E),
        "KEY_O" => Some(0x4F),
        "KEY_P" => Some(0x50),
        "KEY_Q" => Some(0x51),
        "KEY_R" => Some(0x52),
        "KEY_S" => Some(0x53),
        "KEY_T" => Some(0x54),
        "KEY_U" => Some(0x55),
        "KEY_V" => Some(0x56),
        "KEY_W" => Some(0x57),
        "KEY_X" => Some(0x58),
        "KEY_Y" => Some(0x59),
        "KEY_Z" => Some(0x5A),
        "KEY_1" => Some(0x31),
        "KEY_2" => Some(0x32),
        "KEY_3" => Some(0x33),
        "KEY_4" => Some(0x34),
        "KEY_5" => Some(0x35),
        "KEY_6" => Some(0x36),
        "KEY_7" => Some(0x37),
        "KEY_8" => Some(0x38),
        "KEY_9" => Some(0x39),
        "KEY_0" => Some(0x30),
        "KEY_KP1" => Some(0x61),
        "KEY_KP2" => Some(0x62),
        "KEY_KP3" => Some(0x63),
        "KEY_KP4" => Some(0x64),
        "KEY_KP5" => Some(0x65),
        "KEY_KP6" => Some(0x66),
        "KEY_KP7" => Some(0x67),
        "KEY_KP8" => Some(0x68),
        "KEY_KP9" => Some(0x69),
        "KEY_KP0" => Some(0x60),
        "KEY_KPDEL" => Some(0x6E),
        "KEY_KPSTAR" => Some(0x6A),
        "KEY_KPMINUS" => Some(0x6D),
        "KEY_KPPLUS" => Some(0x6B),
        "KEY_UP" => Some(0x26),
        "KEY_DOWN" => Some(0x28),
        "KEY_LEFT" => Some(0x25),
        "KEY_RIGHT" => Some(0x27),
        "KEY_HOME" => Some(0x24),
        "KEY_END" => Some(0x23),
        "KEY_PGUP" => Some(0x21),
        "KEY_PGDN" => Some(0x22),
        "KEY_INS" => Some(0x2D),
        "KEY_DEL" => Some(0x2E),
        "KEY_MINUS" => Some(0xBD),
        "KEY_EQUAL" => Some(0xBB),
        "KEY_LBRACKET" => Some(0xDB),
        "KEY_RBRACKET" => Some(0xDD),
        "KEY_SEMICOLON" => Some(0xBA),
        "KEY_APOSTROPHE" => Some(0xDE),
        "KEY_TICK" => Some(0xC0),
        "KEY_BACKSLASH" => Some(0xDC),
        "KEY_COMMA" => Some(0xBC),
        "KEY_PERIOD" => Some(0xBE),
        "KEY_SLASH" => Some(0xBF),
        "KEY_KPENTER" => Some(0x0D),
        "KEY_KPSLASH" => Some(0x6F),
        "KEY_NONE" => Some(0),
        _ => None,
    }
}

fn lookup_meta_message_type(name: &str) -> Option<GameMessageType> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "SAVE_VIEW1" => Some(GameMessageType::MetaSaveView(1)),
        "SAVE_VIEW2" => Some(GameMessageType::MetaSaveView(2)),
        "SAVE_VIEW3" => Some(GameMessageType::MetaSaveView(3)),
        "SAVE_VIEW4" => Some(GameMessageType::MetaSaveView(4)),
        "SAVE_VIEW5" => Some(GameMessageType::MetaSaveView(5)),
        "SAVE_VIEW6" => Some(GameMessageType::MetaSaveView(6)),
        "SAVE_VIEW7" => Some(GameMessageType::MetaSaveView(7)),
        "SAVE_VIEW8" => Some(GameMessageType::MetaSaveView(8)),
        "VIEW_VIEW1" => Some(GameMessageType::MetaViewView(1)),
        "VIEW_VIEW2" => Some(GameMessageType::MetaViewView(2)),
        "VIEW_VIEW3" => Some(GameMessageType::MetaViewView(3)),
        "VIEW_VIEW4" => Some(GameMessageType::MetaViewView(4)),
        "VIEW_VIEW5" => Some(GameMessageType::MetaViewView(5)),
        "VIEW_VIEW6" => Some(GameMessageType::MetaViewView(6)),
        "VIEW_VIEW7" => Some(GameMessageType::MetaViewView(7)),
        "VIEW_VIEW8" => Some(GameMessageType::MetaViewView(8)),
        "CREATE_TEAM0" => Some(GameMessageType::MetaCreateTeam(0)),
        "CREATE_TEAM1" => Some(GameMessageType::MetaCreateTeam(1)),
        "CREATE_TEAM2" => Some(GameMessageType::MetaCreateTeam(2)),
        "CREATE_TEAM3" => Some(GameMessageType::MetaCreateTeam(3)),
        "CREATE_TEAM4" => Some(GameMessageType::MetaCreateTeam(4)),
        "CREATE_TEAM5" => Some(GameMessageType::MetaCreateTeam(5)),
        "CREATE_TEAM6" => Some(GameMessageType::MetaCreateTeam(6)),
        "CREATE_TEAM7" => Some(GameMessageType::MetaCreateTeam(7)),
        "CREATE_TEAM8" => Some(GameMessageType::MetaCreateTeam(8)),
        "CREATE_TEAM9" => Some(GameMessageType::MetaCreateTeam(9)),
        "SELECT_TEAM0" => Some(GameMessageType::MetaSelectTeam(0)),
        "SELECT_TEAM1" => Some(GameMessageType::MetaSelectTeam(1)),
        "SELECT_TEAM2" => Some(GameMessageType::MetaSelectTeam(2)),
        "SELECT_TEAM3" => Some(GameMessageType::MetaSelectTeam(3)),
        "SELECT_TEAM4" => Some(GameMessageType::MetaSelectTeam(4)),
        "SELECT_TEAM5" => Some(GameMessageType::MetaSelectTeam(5)),
        "SELECT_TEAM6" => Some(GameMessageType::MetaSelectTeam(6)),
        "SELECT_TEAM7" => Some(GameMessageType::MetaSelectTeam(7)),
        "SELECT_TEAM8" => Some(GameMessageType::MetaSelectTeam(8)),
        "SELECT_TEAM9" => Some(GameMessageType::MetaSelectTeam(9)),
        "ADD_TEAM0" => Some(GameMessageType::MetaAddTeam(0)),
        "ADD_TEAM1" => Some(GameMessageType::MetaAddTeam(1)),
        "ADD_TEAM2" => Some(GameMessageType::MetaAddTeam(2)),
        "ADD_TEAM3" => Some(GameMessageType::MetaAddTeam(3)),
        "ADD_TEAM4" => Some(GameMessageType::MetaAddTeam(4)),
        "ADD_TEAM5" => Some(GameMessageType::MetaAddTeam(5)),
        "ADD_TEAM6" => Some(GameMessageType::MetaAddTeam(6)),
        "ADD_TEAM7" => Some(GameMessageType::MetaAddTeam(7)),
        "ADD_TEAM8" => Some(GameMessageType::MetaAddTeam(8)),
        "ADD_TEAM9" => Some(GameMessageType::MetaAddTeam(9)),
        "VIEW_TEAM0" => Some(GameMessageType::MetaViewTeam(0)),
        "VIEW_TEAM1" => Some(GameMessageType::MetaViewTeam(1)),
        "VIEW_TEAM2" => Some(GameMessageType::MetaViewTeam(2)),
        "VIEW_TEAM3" => Some(GameMessageType::MetaViewTeam(3)),
        "VIEW_TEAM4" => Some(GameMessageType::MetaViewTeam(4)),
        "VIEW_TEAM5" => Some(GameMessageType::MetaViewTeam(5)),
        "VIEW_TEAM6" => Some(GameMessageType::MetaViewTeam(6)),
        "VIEW_TEAM7" => Some(GameMessageType::MetaViewTeam(7)),
        "VIEW_TEAM8" => Some(GameMessageType::MetaViewTeam(8)),
        "VIEW_TEAM9" => Some(GameMessageType::MetaViewTeam(9)),
        "SELECT_MATCHING_UNITS" => Some(GameMessageType::MetaSelectMatchingUnits),
        "SELECT_NEXT_UNIT" => Some(GameMessageType::MetaSelectNextUnit),
        "SELECT_PREV_UNIT" => Some(GameMessageType::MetaSelectPrevUnit),
        "SELECT_NEXT_WORKER" => Some(GameMessageType::MetaSelectNextWorker),
        "SELECT_PREV_WORKER" => Some(GameMessageType::MetaSelectPrevWorker),
        "SELECT_HERO" => Some(GameMessageType::MetaSelectHero),
        "SELECT_ALL" => Some(GameMessageType::MetaSelectAll),
        "SELECT_ALL_AIRCRAFT" => Some(GameMessageType::MetaSelectAllAircraft),
        "VIEW_COMMAND_CENTER" => Some(GameMessageType::MetaViewCommandCenter),
        "VIEW_LAST_RADAR_EVENT" => Some(GameMessageType::MetaViewLastRadarEvent),
        "SCATTER" => Some(GameMessageType::MetaScatter),
        "STOP" => Some(GameMessageType::MetaStop),
        "DEPLOY" => Some(GameMessageType::MetaDeploy),
        "CREATE_FORMATION" => Some(GameMessageType::MetaCreateFormation),
        "FOLLOW" => Some(GameMessageType::MetaFollow),
        "CHAT_PLAYERS" => Some(GameMessageType::MetaChatPlayers),
        "CHAT_ALLIES" => Some(GameMessageType::MetaChatAllies),
        "CHAT_EVERYONE" => Some(GameMessageType::MetaChatEveryone),
        "DIPLOMACY" => Some(GameMessageType::MetaDiplomacy),
        "OPTIONS" => Some(GameMessageType::MetaOptions),
        "TOGGLE_CONTROL_BAR" => Some(GameMessageType::MetaToggleControlBar),
        "BEGIN_PATH_BUILD" => Some(GameMessageType::MetaBeginPathBuild),
        "END_PATH_BUILD" => Some(GameMessageType::MetaEndPathBuild),
        "BEGIN_FORCEATTACK" => Some(GameMessageType::MetaBeginForceAttack),
        "END_FORCEATTACK" => Some(GameMessageType::MetaEndForceAttack),
        "BEGIN_FORCEMOVE" => Some(GameMessageType::MetaBeginForceMove),
        "END_FORCEMOVE" => Some(GameMessageType::MetaEndForceMove),
        "BEGIN_WAYPOINTS" => Some(GameMessageType::MetaBeginWaypoints),
        "END_WAYPOINTS" => Some(GameMessageType::MetaEndWaypoints),
        "BEGIN_PREFER_SELECTION" => Some(GameMessageType::MetaBeginPreferSelection),
        "END_PREFER_SELECTION" => Some(GameMessageType::MetaEndPreferSelection),
        "TAKE_SCREENSHOT" => Some(GameMessageType::MetaTakeScreenshot),
        "ALL_CHEER" => Some(GameMessageType::MetaAllCheer),
        "BEGIN_CAMERA_ROTATE_LEFT" => Some(GameMessageType::MetaBeginCameraRotateLeft),
        "END_CAMERA_ROTATE_LEFT" => Some(GameMessageType::MetaEndCameraRotateLeft),
        "BEGIN_CAMERA_ROTATE_RIGHT" => Some(GameMessageType::MetaBeginCameraRotateRight),
        "END_CAMERA_ROTATE_RIGHT" => Some(GameMessageType::MetaEndCameraRotateRight),
        "BEGIN_CAMERA_ZOOM_IN" => Some(GameMessageType::MetaBeginCameraZoomIn),
        "END_CAMERA_ZOOM_IN" => Some(GameMessageType::MetaEndCameraZoomIn),
        "BEGIN_CAMERA_ZOOM_OUT" => Some(GameMessageType::MetaBeginCameraZoomOut),
        "END_CAMERA_ZOOM_OUT" => Some(GameMessageType::MetaEndCameraZoomOut),
        "CAMERA_RESET" => Some(GameMessageType::MetaCameraReset),
        "TOGGLE_CAMERA_TRACKING_DRAWABLE" => Some(GameMessageType::MetaToggleCameraTracking),
        "TOGGLE_FAST_FORWARD_REPLAY" => Some(GameMessageType::MetaToggleFastForwardReplay),
        "DEMO_INSTANT_QUIT" => Some(GameMessageType::MetaDemoInstantQuit),
        _ => None,
    }
}
