use crate::gui::source_catalog::GuiPortRecord;

const FRAME_FUDGE_ADD: f32 = 30.0;
const TELETYPE_UPDATE_FREQ: i32 = 2;
const STATE_BEGIN: i32 = 250;
const STATE_SHOW_LOCATION: i32 = 251;
const STATE_BEGIN_BRIEFING: i32 = 255;
const STATE_SHOW_CAMEO_1: i32 = 434;
const STATE_BEGIN_ANIMATING_TEXT: i32 = 356;
const STATE_HIDE_CAMEO_1: i32 = 459;
const STATE_SHOW_CAMEO_2: i32 = 464;
const STATE_HIDE_CAMEO_2: i32 = 492;
const STATE_SHOW_CAMEO_3: i32 = 497;
const STATE_HIDE_CAMEO_3: i32 = 524;
const STATE_END_ANIMATING_TEXT: i32 = 730;
const STATE_END: i32 = 730;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "LoadScreen.cpp",
    "crate::gui::load_screen",
    "Load Screen",
    "Owns loading-screen layout composition, progress display, and staging transitions.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadScreenKindPort {
    SinglePlayer,
    Challenge,
    ShellGame,
    Multiplayer,
    GameSpy,
    MapTransfer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerLoadProgressPort {
    pub slot: usize,
    pub name: String,
    pub side: String,
    pub progress: f32,
    pub hidden: bool,
}

#[derive(Clone, Debug)]
pub struct LoadScreenPort {
    pub kind: LoadScreenKindPort,
    pub title: String,
    pub objective: String,
    pub progress: f32,
    pub status_text: String,
    pub percent_text: String,
    pub progress_range: (i32, i32),
    pub current_frame: i32,
    pub objective_lines: Vec<String>,
    pub rendered_objective_lines: Vec<String>,
    pub current_objective_line: usize,
    pub current_objective_line_character: usize,
    pub objective_text_finished: bool,
    pub objective_visible: bool,
    pub location_label: String,
    pub location_visible: bool,
    pub unit_descriptions: Vec<String>,
    pub cameo_visible: [bool; 3],
    pub briefing_voice_started: bool,
    pub ambient_loop_active: bool,
    pub player_progress: Vec<PlayerLoadProgressPort>,
}

impl Default for LoadScreenPort {
    fn default() -> Self {
        let objective_lines = vec![
            "Capture the forward supply line before the GLA reinforce the plateau.".to_string(),
            "Secure the command uplink and defend it until the extraction window opens."
                .to_string(),
            "Keep Colonel Burton alive during the infiltration stage.".to_string(),
        ];
        Self {
            kind: LoadScreenKindPort::SinglePlayer,
            title: "Loading Tournament Desert".to_string(),
            objective: "Initialize terrain, players, scripts, and shell transitions.".to_string(),
            progress: 0.42,
            status_text: "Preparing world state".to_string(),
            percent_text: "42%".to_string(),
            progress_range: (0, 100),
            current_frame: 0,
            rendered_objective_lines: vec![String::new(); objective_lines.len()],
            objective_lines,
            current_objective_line: 0,
            current_objective_line_character: 0,
            objective_text_finished: false,
            objective_visible: false,
            location_label: "Taklamakan Forward Operating Base".to_string(),
            location_visible: false,
            unit_descriptions: vec![
                "Ranger strike team".to_string(),
                "Pathfinder overwatch".to_string(),
                "Comanche escort".to_string(),
            ],
            cameo_visible: [false; 3],
            briefing_voice_started: false,
            ambient_loop_active: false,
            player_progress: vec![
                PlayerLoadProgressPort {
                    slot: 0,
                    name: "Commander".to_string(),
                    side: "USA".to_string(),
                    progress: 0.42,
                    hidden: false,
                },
                PlayerLoadProgressPort {
                    slot: 1,
                    name: "Opponent".to_string(),
                    side: "GLA".to_string(),
                    progress: 0.37,
                    hidden: false,
                },
            ],
        }
    }
}

impl LoadScreenPort {
    pub fn advance_progress(&mut self, status_text: impl Into<String>, progress: f32) {
        self.status_text = status_text.into();
        self.progress = progress.clamp(0.0, 1.0);
        self.percent_text = format!("{}%", (self.progress * 100.0).round() as i32);
    }

    pub fn set_progress_range(&mut self, min: i32, max: i32) {
        self.progress_range = if min <= max { (min, max) } else { (max, min) };
    }

    pub fn update_percent(&mut self, percent: i32) {
        let (min, max) = self.progress_range;
        let span = (max - min).max(1) as f32;
        let normalized = ((percent - min) as f32 / span) * 100.0;
        let display_percent = match self.kind {
            LoadScreenKindPort::SinglePlayer | LoadScreenKindPort::Challenge => {
                (normalized + FRAME_FUDGE_ADD) / 1.3
            }
            LoadScreenKindPort::ShellGame
            | LoadScreenKindPort::Multiplayer
            | LoadScreenKindPort::GameSpy
            | LoadScreenKindPort::MapTransfer => normalized,
        }
        .clamp(0.0, 100.0);
        self.progress = display_percent / 100.0;
        self.percent_text = format!("{}%", display_percent.round() as i32);
    }

    pub fn start_ambient_loop(&mut self) {
        self.ambient_loop_active = true;
    }

    pub fn stop_ambient_loop(&mut self) {
        self.ambient_loop_active = false;
    }

    pub fn set_player_progress(&mut self, slot: usize, percent: f32) {
        if let Some(entry) = self
            .player_progress
            .iter_mut()
            .find(|entry| entry.slot == slot)
        {
            entry.progress = percent.clamp(0.0, 1.0);
        }
    }

    pub fn tick_frame(&mut self, frame: i32) {
        self.current_frame = frame;
        if !(STATE_BEGIN..=STATE_END).contains(&frame) {
            return;
        }

        if frame == STATE_BEGIN_BRIEFING {
            self.briefing_voice_started = true;
        }

        if frame == STATE_BEGIN_ANIMATING_TEXT {
            self.objective_visible = true;
        }

        if frame > STATE_BEGIN_ANIMATING_TEXT
            && frame <= STATE_END_ANIMATING_TEXT
            && !self.objective_text_finished
            && (frame - STATE_BEGIN_ANIMATING_TEXT) % TELETYPE_UPDATE_FREQ == 0
        {
            self.advance_objective_teletype();
        }

        match frame {
            STATE_SHOW_LOCATION => self.location_visible = true,
            STATE_SHOW_CAMEO_1 => self.cameo_visible[0] = true,
            STATE_HIDE_CAMEO_1 => self.cameo_visible[0] = false,
            STATE_SHOW_CAMEO_2 => self.cameo_visible[1] = true,
            STATE_HIDE_CAMEO_2 => self.cameo_visible[1] = false,
            STATE_SHOW_CAMEO_3 => self.cameo_visible[2] = true,
            STATE_HIDE_CAMEO_3 => self.cameo_visible[2] = false,
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.progress = 0.0;
        self.percent_text = "0%".to_string();
        self.current_frame = 0;
        self.current_objective_line = 0;
        self.current_objective_line_character = 0;
        self.objective_text_finished = false;
        self.objective_visible = false;
        self.location_visible = false;
        self.cameo_visible = [false; 3];
        self.briefing_voice_started = false;
        self.ambient_loop_active = false;
        self.rendered_objective_lines = vec![String::new(); self.objective_lines.len()];
        for player in &mut self.player_progress {
            player.progress = 0.0;
        }
    }

    pub fn visible_cameo_count(&self) -> usize {
        self.cameo_visible
            .into_iter()
            .filter(|visible| *visible)
            .count()
    }

    pub fn rendered_objective_text(&self) -> String {
        self.rendered_objective_lines
            .iter()
            .filter(|line| !line.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn advance_objective_teletype(&mut self) {
        while self.current_objective_line < self.objective_lines.len() {
            let line = &self.objective_lines[self.current_objective_line];
            let line_length = line.chars().count();
            if self.current_objective_line_character >= line_length {
                self.current_objective_line += 1;
                self.current_objective_line_character = 0;
                continue;
            }

            if let Some(next_char) = line.chars().nth(self.current_objective_line_character) {
                self.rendered_objective_lines[self.current_objective_line].push(next_char);
                self.current_objective_line_character += 1;
                return;
            }
        }

        self.objective_text_finished = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_player_progress_applies_fudge_factor() {
        let mut load = LoadScreenPort::default();
        load.update_percent(35);

        assert_eq!(load.percent_text, "50%");
        assert!((load.progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn tick_frame_reveals_location_and_cameos() {
        let mut load = LoadScreenPort::default();

        load.tick_frame(STATE_SHOW_LOCATION);
        load.tick_frame(STATE_SHOW_CAMEO_1);
        load.tick_frame(STATE_SHOW_CAMEO_2);
        load.tick_frame(STATE_HIDE_CAMEO_1);

        assert!(load.location_visible);
        assert_eq!(load.visible_cameo_count(), 1);
        assert!(!load.cameo_visible[0]);
        assert!(load.cameo_visible[1]);
    }

    #[test]
    fn teletype_advances_across_objective_lines() {
        let mut load = LoadScreenPort {
            objective_lines: vec!["AB".to_string(), "CD".to_string()],
            rendered_objective_lines: vec![String::new(), String::new()],
            ..LoadScreenPort::default()
        };

        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT);
        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT + 2);
        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT + 4);
        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT + 6);
        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT + 8);
        load.tick_frame(STATE_BEGIN_ANIMATING_TEXT + 10);

        assert!(load.objective_visible);
        assert_eq!(load.rendered_objective_lines[0], "AB");
        assert_eq!(load.rendered_objective_lines[1], "CD");
        assert!(load.objective_text_finished);
    }
}
