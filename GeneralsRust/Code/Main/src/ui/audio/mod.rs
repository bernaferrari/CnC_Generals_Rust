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
        // C++ authored radar event: SoundEffects.ini `RadarEvent` has an empty
        // Sounds list (authentic silent chirp); audible cues are the
        // RadarNotify*UnderAttack family dispatched by the radar/EVA lane.
        "Radar_Event" => "RadarEvent",
        "Radar_Attack" => "RadarEvent",
        "Radar_Ally" => "RadarEvent",
        "Radar_BaseAttacked" => "RadarEvent",
        "Radar_EnemyDetected" => "RadarEvent",
        "Radar_UnitCreated" => "RadarEvent",
        "Radar_UnitDestroyed" => "RadarEvent",
        "Radar_Event_Beacon" => "RadarEvent",
        _ => event,
    }
}
