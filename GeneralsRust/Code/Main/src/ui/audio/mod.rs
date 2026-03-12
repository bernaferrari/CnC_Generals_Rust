/// Map high-level UI/gameplay cues to concrete EVA/GUI audio event names.
pub fn translate_audio_event(event: &str) -> &str {
    match event {
        "Mission_Victory" => "GUI_Victory",
        "Mission_Defeat" => "GUI_Defeat",
        "Mission_Warning" => "GUI_Warning",
        "Mission_Failure" => "GUI_Defeat",
        "Mission_Success" => "GUI_Victory",
        "Mission_Message" => "GUIMessageReceived",
        "Beacon_Placed" => "UI_BeaconPlaced",
        "Beacon_Removed" => "UI_BeaconRemoved",
        "Radar_Event" => "UI_RadarEvent",
        "Radar_Attack" => "UI_RadarAttack",
        "Radar_Ally" => "UI_RadarAllyRequest",
        "Radar_BaseAttacked" => "UI_RadarAttack",
        "Radar_EnemyDetected" => "UI_RadarEvent",
        "Radar_UnitCreated" => "UI_RadarEvent",
        "Radar_UnitDestroyed" => "UI_RadarEvent",
        "Radar_Event_Beacon" => "UI_RadarEvent",
        _ => event,
    }
}
