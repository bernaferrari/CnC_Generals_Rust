// FILE: load_screen.rs
//-----------------------------------------------------------------------------
//
//                       Electronic Arts Pacific.
//
//                       Confidential Information
//                Copyright (C) 2002 - All Rights Reserved
//
//-----------------------------------------------------------------------------
//
//  created:    Mar 2002
//
//  Filename:   load_screen.rs
//
//  author:     Chris Huybregts (original C++), Rust port
//
//  purpose:    Contains each of the different derived LoadClasses for each of the
//              Different kind of games we can have.
//
//-----------------------------------------------------------------------------

use std::sync::{Arc, Mutex};
use std::ptr;
use std::time::{Duration, Instant};
use std::thread;

// Constants
const MAX_SLOTS: usize = 8; // MAX_PLAYER + 1 where MAX_PLAYER = 7
const MAX_OBJECTIVE_LINES: usize = 5;
const MAX_DISPLAYED_UNITS: usize = 3;

// Frame timing constants for animations
const FRAME_TITLES_START: i32 = 20;
const FRAME_TELETYPE_START: i32 = 24;
const FRAME_FUDGE_ADD: i32 = 30;
const FRAME_PORTRAITS_START: i32 = 35;
const FRAME_OUTER_CIRCLE_LINE_SHOW: i32 = 50;
const FRAME_INNER_CIRCLE_LINE_SHOW: i32 = 52;
const FRAME_OUTER_CIRCLE_ALPHA_SHOW: i32 = 63;
const FRAME_INNER_CIRCLE_ALPHA_SHOW: i32 = 74;
const FRAME_OUTER_CIRCLE_LINE_HIDE: i32 = 75;
const FRAME_INNER_BACKDROP_ALPHA_SHOW: i32 = 80;
const FRAME_INNER_CIRCLE_LINE_HIDE: i32 = 81;
const FRAME_VS_ANIM_START: i32 = 98;
const FRAME_RIGHT_VOICE: i32 = 140;

const TELETYPE_UPDATE_FREQ: i32 = 2; // how many frames between teletype updates

// Forward declarations / Type aliases
type GameWindow = Arc<Mutex<dyn GameWindowTrait>>;
type VideoBuffer = Arc<Mutex<dyn VideoBufferTrait>>;
type VideoStream = Arc<Mutex<dyn VideoStreamTrait>>;
type AudioHandle = Option<u32>;

// Trait definitions for external dependencies
pub trait GameWindowTrait: Send {
    fn win_hide(&mut self, hide: bool);
    fn win_bring_to_top(&mut self);
    fn win_enable(&mut self, enable: bool);
    fn win_set_position(&mut self, x: i32, y: i32);
    fn win_get_position(&self) -> (i32, i32);
    fn win_set_enabled_image(&mut self, index: usize, image: Option<Arc<dyn ImageTrait>>);
    fn win_set_disabled_image(&mut self, index: usize, image: Option<Arc<dyn ImageTrait>>);
    fn win_set_enabled_text_colors(&mut self, text_color: Color, border_color: Color);
    fn win_get_enabled_text_border_color(&self) -> Color;
    fn win_set_status(&mut self, status: u32);
    fn win_clear_status(&mut self, status: u32);
    fn win_set_user_data(&mut self, data: *const ());
    fn win_get_instance_data(&mut self) -> Arc<Mutex<dyn WindowInstanceDataTrait>>;
}

pub trait WindowInstanceDataTrait: Send {
    fn set_video_buffer(&mut self, buffer: Option<VideoBuffer>);
}

pub trait ImageTrait: Send + Sync {}

pub trait VideoBufferTrait: Send {
    fn allocate(&mut self, width: u32, height: u32) -> bool;
}

pub trait VideoStreamTrait: Send {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn frame_count(&self) -> i32;
    fn frame_index(&self) -> i32;
    fn is_frame_ready(&self) -> bool;
    fn frame_decompress(&mut self);
    fn frame_render(&mut self, buffer: &mut dyn VideoBufferTrait);
    fn frame_next(&mut self);
    fn frame_goto(&mut self, frame: i32);
    fn close(&mut self);
}

pub trait GameInfoTrait: Send {
    fn get_slot(&self, index: usize) -> Option<Arc<Mutex<dyn GameSlotTrait>>>;
    fn get_local_slot_num(&self) -> usize;
    fn get_map(&self) -> String;
}

pub trait GameSlotTrait: Send {
    fn is_occupied(&self) -> bool;
    fn is_human(&self) -> bool;
    fn is_ai(&self) -> bool;
    fn get_player_template(&self) -> i32;
    fn get_apparent_color(&self) -> usize;
    fn get_name(&self) -> String;
    fn get_apparent_player_template_display_name(&self) -> String;
    fn get_team_number(&self) -> i32;
    fn has_map(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct AudioEventRTS {
    event_name: String,
    should_fade: bool,
}

impl AudioEventRTS {
    pub fn new(name: &str) -> Self {
        Self {
            event_name: name.to_string(),
            should_fade: false,
        }
    }

    pub fn set_event_name(&mut self, name: &str) {
        self.event_name = name.to_string();
    }

    pub fn set_should_fade(&mut self, fade: bool) {
        self.should_fade = fade;
    }
}

//-----------------------------------------------------------------------------
// LoadScreen Base Class
//-----------------------------------------------------------------------------
pub trait LoadScreen: Send {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>);
    fn reset(&mut self);
    fn update_void(&mut self);
    fn update(&mut self, percent: i32);
    fn process_progress(&mut self, player_id: i32, percentage: i32);
    fn set_progress_range(&mut self, min: i32, max: i32);
    fn get_load_screen(&self) -> Option<GameWindow>;
    fn set_load_screen(&mut self, window: Option<GameWindow>);
}

pub struct BaseLoadScreen {
    m_load_screen: Option<GameWindow>,
}

impl BaseLoadScreen {
    pub fn new() -> Self {
        Self {
            m_load_screen: None,
        }
    }

    pub fn base_update(&mut self, _percent: i32) {
        // TheGameEngine->serviceWindowsOS();
        // if (TheGameEngine->getQuitting())
        //     return; // don't bother with any of this if the player is exiting game.

        // TheWindowManager->update();
        // TheDisplay->update();
        // redraw all views, update the GUI
        // TheDisplay->draw();

        // setFPMode();
    }
}

//-----------------------------------------------------------------------------
// SinglePlayerLoadScreen Class
//-----------------------------------------------------------------------------
pub struct SinglePlayerLoadScreen {
    base: BaseLoadScreen,
    m_progress_bar: Option<GameWindow>,
    m_percent: Option<GameWindow>,
    m_video_stream: Option<VideoStream>,
    m_video_buffer: Option<VideoBuffer>,
    m_objective_win: Option<GameWindow>,
    m_objective_lines: [Option<GameWindow>; MAX_OBJECTIVE_LINES],
    m_unit_desc: [Option<GameWindow>; MAX_DISPLAYED_UNITS],
    m_location: Option<GameWindow>,
    m_current_objective_line: usize,
    m_current_objective_line_character: usize,
    m_current_objective_width_offset: i32,
    m_finished_objective_text: bool,
    m_unicode_objective_lines: [String; MAX_OBJECTIVE_LINES],
    m_ambient_loop: AudioEventRTS,
    m_ambient_loop_handle: AudioHandle,
}

impl SinglePlayerLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bar: None,
            m_percent: None,
            m_video_stream: None,
            m_video_buffer: None,
            m_objective_win: None,
            m_objective_lines: Default::default(),
            m_unit_desc: Default::default(),
            m_location: None,
            m_current_objective_line: 0,
            m_current_objective_line_character: 0,
            m_current_objective_width_offset: 0,
            m_finished_objective_text: false,
            m_unicode_objective_lines: Default::default(),
            m_ambient_loop: AudioEventRTS::new("LoadScreenAmbient"),
            m_ambient_loop_handle: None,
        }
    }

    fn move_windows(&mut self, frame: i32) {
        const STATE_BEGIN: i32 = 250;
        const STATE_SHOW_LOCATION: i32 = 251;
        const STATE_BEGIN_BREIFING: i32 = 255;
        const STATE_SHOW_CAMEO_1: i32 = 434;
        const STATE_BEGIN_ANIMATING_TEXT: i32 = 356;
        const STATE_HIDE_CAMEO_1: i32 = 459;
        const STATE_SHOW_CAMEO_2: i32 = 464;
        const STATE_HIDE_CAMEO_2: i32 = 492;
        const STATE_SHOW_CAMEO_3: i32 = 497;
        const STATE_HIDE_CAMEO_3: i32 = 524;
        const STATE_END_ANIMATING_TEXT: i32 = 730;
        const STATE_END: i32 = 730;

        if frame < STATE_BEGIN || frame > STATE_END {
            return;
        }

        if frame == STATE_BEGIN_BREIFING {
            // add sound support here
            // TheAudio->friend_forcePlayAudioEventRTS(&TheCampaignManager->getCurrentMission()->m_briefingVoice);
        }

        if frame == STATE_BEGIN_ANIMATING_TEXT {
            if let Some(ref win) = self.m_objective_win {
                win.lock().unwrap().win_hide(false);
            }
            // animate the text and stuff
        }

        if frame > STATE_BEGIN_ANIMATING_TEXT && frame <= STATE_END_ANIMATING_TEXT && !self.m_finished_objective_text {
            if self.m_current_objective_line_character >= self.m_unicode_objective_lines[self.m_current_objective_line].len() {
                self.m_current_objective_line += 1;
                self.m_current_objective_line_character = 0;
            }
            if self.m_current_objective_line >= MAX_OBJECTIVE_LINES
                || self.m_unicode_objective_lines[self.m_current_objective_line].is_empty() {
                self.m_finished_objective_text = true;
            } else {
                let w_char = self.m_unicode_objective_lines[self.m_current_objective_line]
                    .chars()
                    .nth(self.m_current_objective_line_character);

                if let (Some(c), Some(ref win)) = (w_char, &self.m_objective_lines[self.m_current_objective_line]) {
                    // Get current text
                    // let mut text = GadgetStaticTextGetText(win);
                    // text.push(c);
                    // GadgetStaticTextSetText(win, text);
                }
            }
            self.m_current_objective_line_character += 1;
        }

        match frame {
            STATE_SHOW_LOCATION => {
                if let Some(ref win) = self.m_location {
                    win.lock().unwrap().win_hide(false);
                }
            }
            STATE_SHOW_CAMEO_1 => {
                if let Some(ref win) = self.m_unit_desc[0] {
                    win.lock().unwrap().win_hide(false);
                }
            }
            STATE_HIDE_CAMEO_1 => {
                if let Some(ref win) = self.m_unit_desc[0] {
                    win.lock().unwrap().win_hide(true);
                }
            }
            STATE_SHOW_CAMEO_2 => {
                if let Some(ref win) = self.m_unit_desc[1] {
                    win.lock().unwrap().win_hide(false);
                }
            }
            STATE_HIDE_CAMEO_2 => {
                if let Some(ref win) = self.m_unit_desc[1] {
                    win.lock().unwrap().win_hide(true);
                }
            }
            STATE_SHOW_CAMEO_3 => {
                if let Some(ref win) = self.m_unit_desc[2] {
                    win.lock().unwrap().win_hide(false);
                }
            }
            STATE_HIDE_CAMEO_3 => {
                if let Some(ref win) = self.m_unit_desc[2] {
                    win.lock().unwrap().win_hide(true);
                }
            }
            _ => {}
        }
    }
}

impl LoadScreen for SinglePlayerLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        // No music in SinglePlayerLoadScreen

        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/SinglePlayerLoadScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the single player loadscreen"));

        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // Store the pointer to the progress bar on the loadscreen
        // m_progressBar = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("SinglePlayerLoadScreen.wnd:ProgressLoad")));
        // DEBUG_ASSERTCRASH(m_progressBar, ("Can't initialize the progressbar for the single player loadscreen"));
        // GadgetProgressBarSetProgress(m_progressBar, 0);

        // m_percent = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("SinglePlayerLoadScreen.wnd:Percent")));
        // DEBUG_ASSERTCRASH(m_percent, ("Can't initialize the m_percent for the single player loadscreen"));
        // GadgetStaticTextSetText(m_percent, UnicodeString(L"0%"));
        if let Some(ref win) = self.m_percent {
            win.lock().unwrap().win_hide(true);
        }

        // m_objectiveWin = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("SinglePlayerLoadScreen.wnd:ObjectivesWin")));
        // DEBUG_ASSERTCRASH(m_objectiveWin, ("Can't initialize the m_objectiveWin for the single player loadscreen"));
        if let Some(ref win) = self.m_objective_win {
            win.lock().unwrap().win_hide(true);
        }

        // Mission *mission = TheCampaignManager->getCurrentMission();
        // Load objective lines
        for i in 0..MAX_OBJECTIVE_LINES {
            // let line_name = format!("SinglePlayerLoadScreen.wnd:StaticTextLine{}", i);
            // m_objectiveLines[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(lineName));
            // DEBUG_ASSERTCRASH(m_objectiveLines[i], ("Can't initialize the m_objectiveLines[%d] for the single player loadscreen", i));
            // GadgetStaticTextSetText(m_objectiveLines[i], UnicodeString::TheEmptyString);

            // translate the objective lines
            // if (mission->m_missionObjectivesLabel[i].isNotEmpty())
            //     m_unicodeObjectiveLines[i] = TheGameText->fetch(mission->m_missionObjectivesLabel[i]);
        }

        // Load unit descriptions
        for i in 0..MAX_DISPLAYED_UNITS {
            // let line_name = format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{}", i);
            // m_unitDesc[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(lineName));
            // DEBUG_ASSERTCRASH(m_unitDesc[i], ("Can't initialize the m_objectiveLines[%d] for the single player loadscreen", i));
            // GadgetStaticTextSetText(m_unitDesc[i], TheGameText->fetch(mission->m_unitNames[i]));
            if let Some(ref win) = self.m_unit_desc[i] {
                win.lock().unwrap().win_hide(true);
            }
        }

        // m_location = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("SinglePlayerLoadScreen.wnd:StaticTextCameoText3")));
        // DEBUG_ASSERTCRASH(m_location, ("Can't initialize the m_objectiveWin for the single player loadscreen"));
        if let Some(ref win) = self.m_location {
            win.lock().unwrap().win_hide(true);
        }
        // GadgetStaticTextSetText(m_location, TheGameText->fetch(mission->m_locationNameLabel));

        self.m_current_objective_line = 0;
        self.m_current_objective_width_offset = 0;
        self.m_current_objective_line_character = 0;
        self.m_finished_objective_text = false;

        self.m_ambient_loop.set_event_name("LoadScreenAmbient");

        // create the new stream
        // m_videoStream = TheVideoPlayer->open(TheCampaignManager->getCurrentMission()->m_movieLabel);
        // if (m_videoStream == NULL) {
        //     m_percent->winHide(TRUE);
        //     return;
        // }

        // Create the new buffer
        // m_videoBuffer = TheDisplay->createVideoBuffer();
        // if (m_videoBuffer == NULL || !m_videoBuffer->allocate(m_videoStream->width(), m_videoStream->height())) {
        //     delete m_videoBuffer;
        //     m_videoBuffer = NULL;
        //     if (m_videoStream)
        //         m_videoStream->close();
        //     m_videoStream = NULL;
        //     return;
        // }

        // format the progress bar: USA to blue, GLA to green, China to red
        // and set the background image
        // AsciiString campaignName = TheCampaignManager->getCurrentCampaign()->m_name;
        // GameWindow *backgroundWin = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen")));
        // if (campaignName.compareNoCase("USA") == 0) {
        //     backgroundWin->winSetEnabledImage(0, TheMappedImageCollection->findImageByName("MissionLoad_USA"));
        //     m_progressBar->winSetEnabledImage(6, TheMappedImageCollection->findImageByName("LoadingBar_ProgressCenter2"));
        // } else if (campaignName.compareNoCase("GLA") == 0) {
        //     backgroundWin->winSetEnabledImage(0, TheMappedImageCollection->findImageByName("MissionLoad_GLA"));
        //     m_progressBar->winSetEnabledImage(6, TheMappedImageCollection->findImageByName("LoadingBar_ProgressCenter3"));
        // } else if (campaignName.compareNoCase("China") == 0) {
        //     backgroundWin->winSetEnabledImage(0, TheMappedImageCollection->findImageByName("MissionLoad_China"));
        //     m_progressBar->winSetEnabledImage(6, TheMappedImageCollection->findImageByName("LoadingBar_ProgressCenter1"));
        // }

        // Video playback loop
        // if (TheGameLODManager && TheGameLODManager->didMemPass()) {
        //     let progress_update_count = m_videoStream->frameCount() / FRAME_FUDGE_ADD;
        //     let mut shifted_percent = -FRAME_FUDGE_ADD + 1;
        //
        //     while (m_videoStream->frameIndex() < m_videoStream->frameCount() - 1) {
        //         TheGameEngine->serviceWindowsOS();
        //
        //         if (!m_videoStream->isFrameReady()) {
        //             Sleep(1);
        //             continue;
        //         }
        //
        //         if (!TheGameEngine->isActive()) {
        //             // Changing for MissionDisk, just skip to end.
        //             break;
        //         }
        //
        //         m_videoStream->frameDecompress();
        //         m_videoStream->frameRender(m_videoBuffer);
        //         m_videoStream->frameNext();
        //
        //         if (m_videoBuffer)
        //             m_loadScreen->winGetInstanceData()->setVideoBuffer(m_videoBuffer);
        //
        //         if (m_videoStream->frameIndex() % progress_update_count == 0) {
        //             shifted_percent++;
        //             if (shifted_percent > 0)
        //                 shifted_percent = 0;
        //             let percent = (shifted_percent + FRAME_FUDGE_ADD) / 1.3;
        //             let per = format!("{}%", percent);
        //             TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);
        //             GadgetProgressBarSetProgress(m_progressBar, percent);
        //             GadgetStaticTextSetText(m_percent, per);
        //         }
        //         TheWindowManager->update();
        //         TheDisplay->draw();
        //     }
        //
        //     // let the background image show through
        //     m_videoStream->close();
        //     m_videoStream = NULL;
        //     m_loadScreen->winGetInstanceData()->setVideoBuffer(NULL);
        //     TheDisplay->draw();
        // } else {
        //     // if we're min spec'ed don't play a movie
        //     let delay = mission->m_voiceLength * 1000;
        //     let begin = timeGetTime();
        //     let mut curr_time = begin;
        //     let mut fudge_factor = 0;
        //
        //     while (begin + delay > curr_time) {
        //         fudge_factor = 30 * ((curr_time - begin) as f32 / delay as f32) as i32;
        //         GadgetProgressBarSetProgress(m_progressBar, fudge_factor);
        //
        //         TheWindowManager->update();
        //         TheDisplay->draw();
        //         Sleep(100);
        //         curr_time = timeGetTime();
        //     }
        //
        //     TheWindowManager->update();
        //     TheDisplay->draw();
        // }

        // setFPMode();
        if let Some(ref win) = self.m_percent {
            win.lock().unwrap().win_hide(true);
        }
        // m_ambientLoopHandle = TheAudio->addAudioEvent(&m_ambientLoop);
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        self.m_progress_bar = None;
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        let percent = (percent + FRAME_FUDGE_ADD) / 1;
        let per = format!("{}%", percent);
        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);
        // GadgetProgressBarSetProgress(m_progressBar, percent);
        // GadgetStaticTextSetText(m_percent, per);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, _player_id: i32, _percentage: i32) {
        panic!("We Got to a single player load screen throw the Network...");
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for SinglePlayerLoadScreen {
    fn drop(&mut self) {
        self.m_progress_bar = None;
        self.m_percent = None;
        self.m_objective_win = None;
        for i in 0..MAX_OBJECTIVE_LINES {
            self.m_objective_lines[i] = None;
        }

        self.m_video_buffer = None;

        if let Some(ref mut stream) = self.m_video_stream {
            stream.lock().unwrap().close();
        }
        self.m_video_stream = None;

        // TheAudio->removeAudioEvent(m_ambientLoopHandle);
        self.m_ambient_loop_handle = None;
    }
}

//-----------------------------------------------------------------------------
// ChallengeLoadScreen Class
//-----------------------------------------------------------------------------
pub struct ChallengeLoadScreen {
    base: BaseLoadScreen,
    m_progress_bar: Option<GameWindow>,
    m_video_stream: Option<VideoStream>,
    m_video_buffer: Option<VideoBuffer>,
    m_wnd_video_manager: Option<Arc<Mutex<WindowVideoManager>>>,
    m_ambient_loop: AudioEventRTS,
    m_ambient_loop_handle: AudioHandle,

    // Bio fields - left side
    m_bio_name_left: Option<GameWindow>,
    m_bio_age_left: Option<GameWindow>,
    m_bio_birthplace_left: Option<GameWindow>,
    m_bio_strategy_left: Option<GameWindow>,
    m_bio_big_name_entry_left: Option<GameWindow>,
    m_bio_name_entry_left: Option<GameWindow>,
    m_bio_age_entry_left: Option<GameWindow>,
    m_bio_birthplace_entry_left: Option<GameWindow>,
    m_bio_strategy_entry_left: Option<GameWindow>,

    // Bio fields - right side
    m_bio_big_name_entry_right: Option<GameWindow>,
    m_bio_name_right: Option<GameWindow>,
    m_bio_age_right: Option<GameWindow>,
    m_bio_birthplace_right: Option<GameWindow>,
    m_bio_strategy_right: Option<GameWindow>,
    m_bio_name_entry_right: Option<GameWindow>,
    m_bio_age_entry_right: Option<GameWindow>,
    m_bio_birthplace_entry_right: Option<GameWindow>,
    m_bio_strategy_entry_right: Option<GameWindow>,

    // Portrait windows
    m_portrait_left: Option<GameWindow>,
    m_portrait_right: Option<GameWindow>,
    m_portrait_movie_left: Option<GameWindow>,
    m_portrait_movie_right: Option<GameWindow>,

    // Overlay windows
    m_overlay_reticle_circle_alpha_outer: Option<GameWindow>,
    m_overlay_reticle_circle_alpha_inner: Option<GameWindow>,
    m_overlay_vs_backdrop: Option<GameWindow>,
    m_overlay_vs: Option<GameWindow>,
}

pub struct WindowVideoManager {
    // Window video manager implementation
}

impl WindowVideoManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn init(&mut self) {
        // Initialize video manager
    }

    pub fn update(&mut self) {
        // Update video manager
    }

    pub fn play_movie(&mut self, _window: Option<GameWindow>, _movie_name: &str, _flags: u32) {
        // Play movie in window
    }
}

const WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME: u32 = 1;

impl ChallengeLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bar: None,
            m_video_stream: None,
            m_video_buffer: None,
            m_wnd_video_manager: None,
            m_ambient_loop: AudioEventRTS::new("LoadScreenAmbient"),
            m_ambient_loop_handle: None,
            m_bio_name_left: None,
            m_bio_age_left: None,
            m_bio_birthplace_left: None,
            m_bio_strategy_left: None,
            m_bio_big_name_entry_left: None,
            m_bio_name_entry_left: None,
            m_bio_age_entry_left: None,
            m_bio_birthplace_entry_left: None,
            m_bio_strategy_entry_left: None,
            m_bio_big_name_entry_right: None,
            m_bio_name_right: None,
            m_bio_age_right: None,
            m_bio_birthplace_right: None,
            m_bio_strategy_right: None,
            m_bio_name_entry_right: None,
            m_bio_age_entry_right: None,
            m_bio_birthplace_entry_right: None,
            m_bio_strategy_entry_right: None,
            m_portrait_left: None,
            m_portrait_right: None,
            m_portrait_movie_left: None,
            m_portrait_movie_right: None,
            m_overlay_reticle_circle_alpha_outer: None,
            m_overlay_reticle_circle_alpha_inner: None,
            m_overlay_vs_backdrop: None,
            m_overlay_vs: None,
        }
    }

    fn activate_pieces(&mut self, frame: i32, _general_player: &GeneralPersona, _general_opponent: &GeneralPersona) {
        // Static variables for teletype text positions
        static mut TEXT_POS_BIG_NAME_RIGHT: i32 = 0;
        static mut TEXT_POS_NAME_RIGHT: i32 = 0;
        static mut TEXT_POS_AGE_RIGHT: i32 = 0;
        static mut TEXT_POS_BIRTHPLACE_RIGHT: i32 = 0;
        static mut TEXT_POS_STRATEGY_RIGHT: i32 = 0;
        static mut TEXT_POS_BIG_NAME_LEFT: i32 = 0;
        static mut TEXT_POS_NAME_LEFT: i32 = 0;
        static mut TEXT_POS_AGE_LEFT: i32 = 0;
        static mut TEXT_POS_BIRTHPLACE_LEFT: i32 = 0;
        static mut TEXT_POS_STRATEGY_LEFT: i32 = 0;

        // AudioEventRTS eventLeftGeneral(generalPlayer->getNameSound());
        // AudioEventRTS eventVS("Taunts_GCAnnouncer12");
        // AudioEventRTS eventRightGeneral(generalOpponent->getNameSound());

        match frame {
            FRAME_TITLES_START => {
                if let Some(ref win) = self.m_bio_name_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_birthplace_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_strategy_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_name_right {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_birthplace_right {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_strategy_right {
                    win.lock().unwrap().win_hide(false);
                }
            }
            FRAME_TELETYPE_START => {
                // reinit the statics for each new load screen
                unsafe {
                    TEXT_POS_BIG_NAME_RIGHT = 0;
                    TEXT_POS_NAME_RIGHT = 0;
                    TEXT_POS_AGE_RIGHT = 0;
                    TEXT_POS_BIRTHPLACE_RIGHT = 0;
                    TEXT_POS_STRATEGY_RIGHT = 0;
                    TEXT_POS_BIG_NAME_LEFT = 0;
                    TEXT_POS_NAME_LEFT = 0;
                    TEXT_POS_AGE_LEFT = 0;
                    TEXT_POS_BIRTHPLACE_LEFT = 0;
                    TEXT_POS_STRATEGY_LEFT = 0;
                }

                if let Some(ref win) = self.m_bio_big_name_entry_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_name_entry_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_birthplace_entry_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_strategy_entry_left {
                    win.lock().unwrap().win_hide(false);
                }
                // GadgetStaticTextSetText(m_bioBigNameEntryLeft, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioNameEntryLeft, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioBirthplaceEntryLeft, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioStrategyEntryLeft, UnicodeString::TheEmptyString);

                if let Some(ref win) = self.m_bio_big_name_entry_right {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_name_entry_right {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_birthplace_entry_right {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_bio_strategy_entry_right {
                    win.lock().unwrap().win_hide(false);
                }
                // GadgetStaticTextSetText(m_bioBigNameEntryRight, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioNameEntryRight, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioBirthplaceEntryRight, UnicodeString::TheEmptyString);
                // GadgetStaticTextSetText(m_bioStrategyEntryRight, UnicodeString::TheEmptyString);
            }
            FRAME_PORTRAITS_START => {
                // m_wndVideoManager->playMovie(m_portraitMovieLeft, generalPlayer->getPortraitMovieLeftName(), WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME);
                // m_wndVideoManager->playMovie(m_portraitMovieRight, generalOpponent->getPortraitMovieRightName(), WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME);
                if let Some(ref win) = self.m_portrait_movie_left {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_portrait_movie_right {
                    win.lock().unwrap().win_hide(false);
                }
                // TheAudio->addAudioEvent(&eventLeftGeneral);
            }
            FRAME_OUTER_CIRCLE_ALPHA_SHOW => {
                if let Some(ref win) = self.m_overlay_reticle_circle_alpha_outer {
                    win.lock().unwrap().win_hide(false);
                }
            }
            FRAME_INNER_CIRCLE_ALPHA_SHOW => {
                if let Some(ref win) = self.m_overlay_reticle_circle_alpha_inner {
                    win.lock().unwrap().win_hide(false);
                }
            }
            FRAME_INNER_BACKDROP_ALPHA_SHOW => {
                if let Some(ref win) = self.m_overlay_vs_backdrop {
                    win.lock().unwrap().win_hide(false);
                }
            }
            FRAME_VS_ANIM_START => {
                // it's time to start the overlay movie
                if let Some(ref win) = self.m_overlay_vs_backdrop {
                    win.lock().unwrap().win_hide(false);
                }
                if let Some(ref win) = self.m_overlay_vs {
                    win.lock().unwrap().win_hide(false);
                }
                // m_wndVideoManager->playMovie(m_overlayVs, AsciiString("VSSmall"), WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME);
                // "Verses"
                // TheAudio->addAudioEvent(&eventVS);
            }
            FRAME_RIGHT_VOICE => {
                // TheAudio->addAudioEvent(&eventRightGeneral);
            }
            _ => {}
        }

        // update the teletype readout
        if frame > FRAME_TELETYPE_START && (frame % TELETYPE_UPDATE_FREQ) == 0 {
            // textPosNameLeft = updateTeletypeText(1, m_bioNameEntryLeft, TheGameText->fetch(generalPlayer->getBioName()), textPosNameLeft);
            // textPosBigNameLeft = updateTeletypeText(1, m_bioBigNameEntryLeft, TheGameText->fetch(generalPlayer->getBioName()), textPosBigNameLeft);
            // textPosBirthplaceLeft = updateTeletypeText(1, m_bioBirthplaceEntryLeft, TheGameText->fetch(generalPlayer->getBioRank()), textPosBirthplaceLeft);
            // textPosStrategyLeft = updateTeletypeText(1, m_bioStrategyEntryLeft, TheGameText->fetch(generalPlayer->getBioStrategy()), textPosStrategyLeft);

            // textPosNameRight = updateTeletypeText(1, m_bioNameEntryRight, TheGameText->fetch(generalOpponent->getBioName()), textPosNameRight);
            // textPosBigNameRight = updateTeletypeText(1, m_bioBigNameEntryRight, TheGameText->fetch(generalOpponent->getBioName()), textPosBigNameRight);
            // textPosBirthplaceRight = updateTeletypeText(1, m_bioBirthplaceEntryRight, TheGameText->fetch(generalOpponent->getBioRank()), textPosBirthplaceRight);
            // textPosStrategyRight = updateTeletypeText(1, m_bioStrategyEntryRight, TheGameText->fetch(generalOpponent->getBioStrategy()), textPosStrategyRight);
        }
    }

    fn activate_pieces_min_spec(&mut self, _general_player: &GeneralPersona, _general_opponent: &GeneralPersona) {
        if let Some(ref win) = self.m_bio_name_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_birthplace_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_strategy_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_name_right {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_birthplace_right {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_strategy_right {
            win.lock().unwrap().win_hide(false);
        }

        if let Some(ref win) = self.m_bio_big_name_entry_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_name_entry_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_birthplace_entry_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_strategy_entry_left {
            win.lock().unwrap().win_hide(false);
        }

        // GadgetStaticTextSetText(m_bioBigNameEntryLeft, TheGameText->fetch(generalPlayer->getBioName()));
        // GadgetStaticTextSetText(m_bioNameEntryLeft, TheGameText->fetch(generalPlayer->getBioName()));
        // GadgetStaticTextSetText(m_bioBirthplaceEntryLeft, TheGameText->fetch(generalPlayer->getBioRank()));
        // GadgetStaticTextSetText(m_bioStrategyEntryLeft, TheGameText->fetch(generalPlayer->getBioStrategy()));

        if let Some(ref win) = self.m_bio_big_name_entry_right {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_name_entry_right {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_birthplace_entry_right {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_bio_strategy_entry_right {
            win.lock().unwrap().win_hide(false);
        }

        // GadgetStaticTextSetText(m_bioBigNameEntryRight, TheGameText->fetch(generalOpponent->getBioName()));
        // GadgetStaticTextSetText(m_bioNameEntryRight, TheGameText->fetch(generalOpponent->getBioName()));
        // GadgetStaticTextSetText(m_bioBirthplaceEntryRight, TheGameText->fetch(generalOpponent->getBioRank()));
        // GadgetStaticTextSetText(m_bioStrategyEntryRight, TheGameText->fetch(generalOpponent->getBioStrategy()));

        // m_portraitLeft->winSetEnabledImage(0, generalPlayer->getBioPortraitLarge());
        // m_portraitRight->winSetEnabledImage(0, generalOpponent->getBioPortraitLarge());
        if let Some(ref win) = self.m_portrait_left {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_portrait_right {
            win.lock().unwrap().win_hide(false);
        }

        if let Some(ref win) = self.m_overlay_reticle_circle_alpha_outer {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_overlay_reticle_circle_alpha_inner {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_overlay_vs_backdrop {
            win.lock().unwrap().win_hide(false);
        }
        if let Some(ref win) = self.m_overlay_vs {
            win.lock().unwrap().win_hide(false);
        }

        // m_wndVideoManager->playMovie(m_overlayVs, AsciiString("VSSmall"), WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME);
    }
}

pub struct GeneralPersona {
    // General persona data
}

impl LoadScreen for ChallengeLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        // const Campaign *campaign = TheCampaignManager->getCurrentCampaign();
        // const Mission *mission = TheCampaignManager->getCurrentMission();

        // the player general is tied to the campaign
        // const GeneralPersona* generalPlayer = TheChallengeGenerals->getPlayerGeneralByCampaignName(campaign->m_name);

        // the opponent general is tied to the mission
        // DEBUG_ASSERTCRASH(mission->m_generalName.isNotEmpty(), ("No GeneralName associated with this mission, check Campaign.ini"));
        // const GeneralPersona* generalOpponent = TheChallengeGenerals->getGeneralByGeneralName(mission->m_generalName);

        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/ChallengeLoadScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the single player loadscreen"));
        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // Store the pointer to the progress bar on the loadscreen
        // m_progressBar = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("ChallengeLoadScreen.wnd:ProgressLoad")));
        // DEBUG_ASSERTCRASH(m_progressBar, ("Can't initialize the progressbar for the single player loadscreen"));
        // GadgetProgressBarSetProgress(m_progressBar, 0);

        self.m_ambient_loop.set_event_name("LoadScreenAmbient");

        // create the new background video stream
        // m_videoStream = TheVideoPlayer->open(TheCampaignManager->getCurrentMission()->m_movieLabel);

        // Create the new buffer
        // m_videoBuffer = TheDisplay->createVideoBuffer();
        // if (m_videoBuffer == NULL || !m_videoBuffer->allocate(m_videoStream->width(), m_videoStream->height())) {
        //     delete m_videoBuffer;
        //     m_videoBuffer = NULL;
        //     if (m_videoStream)
        //         m_videoStream->close();
        //     m_videoStream = NULL;
        //     return;
        // }

        // init overlays
        // namekey = TheNameKeyGenerator->nameToKey(AsciiString("ChallengeLoadScreen.wnd:PortraitLeft"));
        // m_portraitLeft = TheWindowManager->winGetWindowFromId(m_loadScreen, namekey);
        // ... (similar initialization for all other windows)

        // make sure reticle stuff starts out hidden
        if let Some(ref win) = self.m_overlay_reticle_circle_alpha_outer {
            win.lock().unwrap().win_hide(true);
        }
        if let Some(ref win) = self.m_overlay_reticle_circle_alpha_inner {
            win.lock().unwrap().win_hide(true);
        }
        if let Some(ref win) = self.m_overlay_vs_backdrop {
            win.lock().unwrap().win_hide(true);
        }
        if let Some(ref win) = self.m_overlay_vs {
            win.lock().unwrap().win_hide(true);
        }

        // m_wndVideoManager = NEW WindowVideoManager;
        // m_wndVideoManager->init();

        // if (TheGameLODManager && TheGameLODManager->didMemPass()) {
        //     Int progressUpdateCount = m_videoStream->frameCount() / FRAME_FUDGE_ADD;
        //     Int shiftedPercent = -FRAME_FUDGE_ADD + 1;
        //     while (m_videoStream->frameIndex() < m_videoStream->frameCount() - 1) {
        //         TheGameEngine->serviceWindowsOS();
        //
        //         if (!m_videoStream->isFrameReady()) {
        //             Sleep(1);
        //             continue;
        //         }
        //
        //         if (!TheGameEngine->isActive()) {
        //             m_videoStream->frameNext();
        //             m_videoStream->frameDecompress();
        //             continue;
        //         }
        //
        //         m_videoStream->frameDecompress();
        //         m_videoStream->frameRender(m_videoBuffer);
        //         m_videoStream->frameNext();
        //
        //         if (m_videoBuffer)
        //             m_loadScreen->winGetInstanceData()->setVideoBuffer(m_videoBuffer);
        //
        //         Int frame = m_videoStream->frameIndex();
        //         if (frame % progressUpdateCount == 0) {
        //             shiftedPercent++;
        //             if (shiftedPercent > 0)
        //                 shiftedPercent = 0;
        //             Int percent = (shiftedPercent + FRAME_FUDGE_ADD) / 1.3;
        //             UnicodeString per;
        //             per.format(L"%d%%", percent);
        //             TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);
        //             GadgetProgressBarSetProgress(m_progressBar, percent);
        //         }
        //         TheWindowManager->update();
        //
        //         activatePieces(frame, generalPlayer, generalOpponent);
        //         m_wndVideoManager->update();
        //
        //         TheDisplay->draw();
        //         TheAudio->update();
        //     }
        // } else {
        //     // if we're min speced
        //     m_videoStream->frameGoto(m_videoStream->frameCount());
        //     while (!m_videoStream->isFrameReady())
        //         Sleep(1);
        //     m_videoStream->frameDecompress();
        //     m_videoStream->frameRender(m_videoBuffer);
        //     if (m_videoBuffer)
        //         m_loadScreen->winGetInstanceData()->setVideoBuffer(m_videoBuffer);
        //
        //     activatePiecesMinSpec(generalPlayer, generalOpponent);
        //
        //     Int delay = mission->m_voiceLength * 1000;
        //     Int begin = timeGetTime();
        //     Int currTime = begin;
        //     Int fudgeFactor = 0;
        //     while (begin + delay > currTime) {
        //         fudgeFactor = 30 * ((currTime - begin) / INT_TO_REAL(delay));
        //         GadgetProgressBarSetProgress(m_progressBar, fudgeFactor);
        //
        //         TheWindowManager->update();
        //         TheDisplay->draw();
        //         Sleep(100);
        //         currTime = timeGetTime();
        //     }
        //
        //     m_wndVideoManager->update();
        //     TheWindowManager->update();
        //     TheDisplay->draw();
        // }
        // setFPMode();

        // AudioEventRTS event(generalOpponent->getRandomTauntSound());
        // TheAudio->addAudioEvent(&event);

        // m_ambientLoopHandle = TheAudio->addAudioEvent(&m_ambientLoop);
        // TheAudio->update();
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        self.m_progress_bar = None;
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        let percent = (percent + FRAME_FUDGE_ADD) / 1;
        let per = format!("{}%", percent);
        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);
        // GadgetProgressBarSetProgress(m_progressBar, percent);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, _player_id: i32, _percentage: i32) {
        panic!("We Got to a single player load screen throw the Network...");
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for ChallengeLoadScreen {
    fn drop(&mut self) {
        self.m_progress_bar = None;

        self.m_video_buffer = None;

        if let Some(ref mut stream) = self.m_video_stream {
            stream.lock().unwrap().close();
        }
        self.m_video_stream = None;

        self.m_bio_name_left = None;
        self.m_bio_age_left = None;
        self.m_bio_birthplace_left = None;
        self.m_bio_strategy_left = None;
        self.m_bio_big_name_entry_left = None;
        self.m_bio_name_entry_left = None;
        self.m_bio_age_entry_left = None;
        self.m_bio_birthplace_entry_left = None;
        self.m_bio_strategy_entry_left = None;
        self.m_bio_big_name_entry_right = None;
        self.m_bio_name_right = None;
        self.m_bio_age_right = None;
        self.m_bio_birthplace_right = None;
        self.m_bio_strategy_right = None;
        self.m_bio_name_entry_right = None;
        self.m_bio_age_entry_right = None;
        self.m_bio_birthplace_entry_right = None;
        self.m_bio_strategy_entry_right = None;

        self.m_portrait_left = None;
        self.m_portrait_right = None;
        self.m_portrait_movie_left = None;
        self.m_portrait_movie_right = None;

        self.m_overlay_reticle_circle_alpha_outer = None;
        self.m_overlay_reticle_circle_alpha_inner = None;
        self.m_overlay_vs_backdrop = None;
        self.m_overlay_vs = None;

        self.m_wnd_video_manager = None;

        // TheAudio->removeAudioEvent(m_ambientLoopHandle);
        self.m_ambient_loop_handle = None;
    }
}

//-----------------------------------------------------------------------------
// ShellGameLoadScreen Class
//-----------------------------------------------------------------------------
pub struct ShellGameLoadScreen {
    base: BaseLoadScreen,
    m_progress_bar: Option<GameWindow>,
}

impl ShellGameLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bar: None,
        }
    }
}

impl LoadScreen for ShellGameLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        static mut FIRST_LOAD: bool = true;

        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/ShellGameLoadScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the ShellGame loadscreen"));
        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // Store the pointer to the progress bar on the loadscreen
        // m_progressBar = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("ShellGameLoadScreen.wnd:ProgressLoad")));
        // DEBUG_ASSERTCRASH(m_progressBar, ("Can't initialize the progressbar for the single player loadscreen"));
        // GadgetProgressBarSetProgress(m_progressBar, 0);
        if let Some(ref win) = self.m_progress_bar {
            win.lock().unwrap().win_hide(true);
        }

        unsafe {
            if FIRST_LOAD {
                // if (m_loadScreen && TheGameLODManager && TheGameLODManager->didMemPass()) {
                //     m_loadScreen->winSetEnabledImage(0, TheMappedImageCollection->findImageByName("TitleScreen"));
                //     TheWritableGlobalData->m_breakTheMovie = FALSE;
                //
                //     GameWindow *win = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(AsciiString("ShellGameLoadScreen.wnd:StaticTextLegal")));
                //     if (win)
                //         win->winHide(FALSE);
                //     FIRST_LOAD = FALSE;
                //
                //     UnsignedInt showTime = timeGetTime();
                //     while (showTime + 3000 > timeGetTime()) {
                //         LoadScreen::update(0);
                //         Sleep(100);
                //     }
                // }

                FIRST_LOAD = false;
            }
        }

        if let Some(ref win) = self.m_progress_bar {
            win.lock().unwrap().win_hide(false);
        }
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        self.m_progress_bar = None;
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);
        // GadgetProgressBarSetProgress(m_progressBar, percent);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, _player_id: i32, _percentage: i32) {
        panic!("We Got to a single player load screen throw the Network...");
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for ShellGameLoadScreen {
    fn drop(&mut self) {
        self.m_progress_bar = None;
    }
}

//-----------------------------------------------------------------------------
// MultiPlayerLoadScreen Class
//-----------------------------------------------------------------------------
pub struct MultiPlayerLoadScreen {
    base: BaseLoadScreen,
    m_progress_bars: [Option<GameWindow>; MAX_SLOTS],
    m_player_names: [Option<GameWindow>; MAX_SLOTS],
    m_player_side: [Option<GameWindow>; MAX_SLOTS],
    m_player_lookup: [i32; MAX_SLOTS],
    m_map_preview: Option<GameWindow>,
    m_button_map_start_position: [Option<GameWindow>; MAX_SLOTS],
    m_portrait_local_general: Option<GameWindow>,
    m_features_local_general: Option<GameWindow>,
    m_name_local_general: Option<GameWindow>,
}

impl MultiPlayerLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bars: Default::default(),
            m_player_names: Default::default(),
            m_player_side: Default::default(),
            m_player_lookup: [-1; MAX_SLOTS],
            m_map_preview: None,
            m_button_map_start_position: Default::default(),
            m_portrait_local_general: None,
            m_features_local_general: None,
            m_name_local_general: None,
        }
    }

    pub fn process_progress_multi(&mut self, player_id: i32, percentage: i32) {
        if percentage < 0 || percentage > 100 || player_id >= MAX_SLOTS as i32 || player_id < 0
            || self.m_player_lookup[player_id as usize] == -1 {
            // DEBUG_ASSERTCRASH(FALSE, ("Percentage %d was passed in for Player %d\n", percentage, playerId));
            return;
        }

        let lookup_idx = self.m_player_lookup[player_id as usize] as usize;
        if let Some(ref win) = self.m_progress_bars[lookup_idx] {
            // GadgetProgressBarSetProgress(m_progressBars[lookup_idx], percentage);
        }
    }
}

impl LoadScreen for MultiPlayerLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/MultiplayerLoadScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the Multiplayer loadscreen"));
        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // m_mapPreview = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey("MultiplayerLoadScreen.wnd:WinMapPreview"));

        // Get local slot and player template
        // GameSlot *lSlot = game->getSlot(game->getLocalSlotNum());
        // const PlayerTemplate* pt;
        // if (lSlot->getPlayerTemplate() >= 0)
        //     pt = ThePlayerTemplateStore->getNthPlayerTemplate(lSlot->getPlayerTemplate());
        // else
        //     pt = ThePlayerTemplateStore->findPlayerTemplate(TheNameKeyGenerator->nameToKey("FactionObserver"));

        // add portrait, features, and name for the local player's general
        // const GeneralPersona *localGeneral = TheChallengeGenerals->getGeneralByTemplateName(pt->getName());
        // ... (portrait and name setup)

        // m_portraitLocalGeneral = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey("MultiplayerLoadScreen.wnd:LocalGeneralPortrait"));
        // m_portraitLocalGeneral->winSetEnabledImage(0, portrait);
        // m_featuresLocalGeneral = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey("MultiplayerLoadScreen.wnd:LocalGeneralFeatures"));
        // GadgetStaticTextSetText(m_featuresLocalGeneral, TheGameText->fetch(features.isEmpty() ? AsciiString("GUI:PlayerObserver") : pt->getGeneralFeatures()));
        // m_nameLocalGeneral = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey("MultiplayerLoadScreen.wnd:LocalGeneralName"));
        // GadgetStaticTextSetText(m_nameLocalGeneral, localName);

        // AsciiString musicName = pt->getLoadScreenMusic();
        // if (!musicName.isEmpty()) {
        //     TheAudio->removeAudioEvent(AHSV_StopTheMusicFade);
        //     AudioEventRTS event(musicName);
        //     event.setShouldFade(TRUE);
        //     TheAudio->addAudioEvent(&event);
        //     TheAudio->update();
        // }

        let mut net_slot = 0;
        // Loop through and make the loadscreen look all good.
        for i in 0..MAX_SLOTS {
            // Load the Progress Bar
            // let win_name = format!("MultiplayerLoadScreen.wnd:ProgressLoad{}", i);
            // m_progressBars[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_progressBars[i], ("Can't initialize the progressbars for the Multiplayer loadscreen"));
            // GadgetProgressBarSetProgress(m_progressBars[i], 0);

            // Load MapStart Positions
            // let win_name = format!("MultiplayerLoadScreen.wnd:ButtonMapStartPosition{}", i);
            // m_buttonMapStartPosition[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_buttonMapStartPosition[i], ("Can't initialize the MapStart Positions for the MultiplayerLoadScreen loadscreen"));

            // Load the Player's name
            // let win_name = format!("MultiplayerLoadScreen.wnd:StaticTextPlayer{}", i);
            // m_playerNames[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_playerNames[i], ("Can't initialize the Names for the Multiplayer loadscreen"));

            // Load the Player's Side
            // let win_name = format!("MultiplayerLoadScreen.wnd:StaticTextSide{}", i);
            // m_playerSide[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_playerSide[i], ("Can't initialize the Sides for the Multiplayer loadscreen"));

            // get the slot man!
            // GameSlot *slot = game->getSlot(i);
            // if (!slot || !slot->isOccupied())
            //     continue;

            // Color houseColor = TheMultiplayerSettings->getColor(slot->getApparentColor())->getColor();

            // format the progress bar to house colors
            // AsciiString imageName;
            // imageName.format("LoadingBar_ProgressCenter%d", slot->getApparentColor());
            // const Image *houseImage = TheMappedImageCollection->findImageByName(imageName);
            // if (!houseImage)
            //     houseImage = TheMappedImageCollection->findImageByName("LoadingBar_Progress");
            // m_progressBars[netSlot]->winSetEnabledImage(6, houseImage);

            // UnicodeString name = slot->getName();
            // GadgetStaticTextSetText(m_playerNames[netSlot], name);
            // m_playerNames[netSlot]->winSetEnabledTextColors(houseColor, m_playerNames[netSlot]->winGetEnabledTextBorderColor());

            // GadgetStaticTextSetText(m_playerSide[netSlot], slot->getApparentPlayerTemplateDisplayName());
            // m_playerSide[netSlot]->winSetEnabledTextColors(houseColor, m_playerSide[netSlot]->winGetEnabledTextBorderColor());

            // if (slot->isAI() && m_progressBars[netSlot])
            //     m_progressBars[netSlot]->winHide(TRUE);

            // if (teamWin[netSlot]) {
            //     AsciiString teamStr;
            //     teamStr.format("Team:%d", slot->getTeamNumber() + 1);
            //     GadgetStaticTextSetText(teamWin[netSlot], TheGameText->fetch(teamStr));
            //     teamWin[netSlot]->winSetEnabledTextColors(houseColor, m_playerNames[netSlot]->winGetEnabledTextBorderColor());
            // }

            // m_playerLookup[i] = netSlot;
            // netSlot++;
        }

        for i in net_slot..MAX_SLOTS {
            if let Some(ref win) = self.m_progress_bars[i] {
                win.lock().unwrap().win_hide(true);
            }
            if let Some(ref win) = self.m_player_names[i] {
                win.lock().unwrap().win_hide(true);
            }
            if let Some(ref win) = self.m_player_side[i] {
                win.lock().unwrap().win_hide(true);
            }
        }

        // if (m_mapPreview) {
        //     const MapMetaData *mmd = TheMapCache->findMap(game->getMap());
        //     Image *image = getMapPreviewImage(game->getMap());
        //     m_mapPreview->winSetUserData((void *)mmd);
        //
        //     positionStartSpots(game, m_buttonMapStartPosition, m_mapPreview);
        //     updateMapStartSpots(game, m_buttonMapStartPosition, TRUE);
        //
        //     if (image) {
        //         m_mapPreview->winSetStatus(WIN_STATUS_IMAGE);
        //         m_mapPreview->winSetEnabledImage(0, image);
        //     } else {
        //         m_mapPreview->winClearStatus(WIN_STATUS_IMAGE);
        //     }
        // }

        // TheGameLogic->initTimeOutValues();
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_player_side[i] = None;
        }
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        // if (TheNetwork) {
        //     if (percent <= 100)
        //         TheNetwork->updateLoadProgress(percent);
        //     TheNetwork->liteupdate();
        // } else {
        //     if (percent <= 100)
        //         TheGameLogic->processProgress(TheGameInfo->getLocalSlotNum(), percent);
        // }

        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, player_id: i32, percentage: i32) {
        self.process_progress_multi(player_id, percentage);
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for MultiPlayerLoadScreen {
    fn drop(&mut self) {
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_player_side[i] = None;
            self.m_player_lookup[i] = -1;
        }

        self.m_portrait_local_general = None;
        self.m_features_local_general = None;
        self.m_name_local_general = None;

        // TheAudio->removeAudioEvent(AHSV_StopTheMusicFade);
    }
}

//-----------------------------------------------------------------------------
// GameSpyLoadScreen Class
//-----------------------------------------------------------------------------
pub struct GameSpyLoadScreen {
    base: BaseLoadScreen,
    m_progress_bars: [Option<GameWindow>; MAX_SLOTS],
    m_player_names: [Option<GameWindow>; MAX_SLOTS],
    m_player_side: [Option<GameWindow>; MAX_SLOTS],
    m_player_favorite_factions: [Option<GameWindow>; MAX_SLOTS],
    m_player_total_disconnects: [Option<GameWindow>; MAX_SLOTS],
    m_player_win: [Option<GameWindow>; MAX_SLOTS],
    m_player_win_losses: [Option<GameWindow>; MAX_SLOTS],
    m_player_rank: [Option<GameWindow>; MAX_SLOTS],
    m_player_officer_medal: [Option<GameWindow>; MAX_SLOTS],
    m_map_preview: Option<GameWindow>,
    m_button_map_start_position: [Option<GameWindow>; MAX_SLOTS],
    m_player_lookup: [i32; MAX_SLOTS],
    m_portrait_local_general: Option<GameWindow>,
    m_features_local_general: Option<GameWindow>,
    m_name_local_general: Option<GameWindow>,
}

impl GameSpyLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bars: Default::default(),
            m_player_names: Default::default(),
            m_player_side: Default::default(),
            m_player_favorite_factions: Default::default(),
            m_player_total_disconnects: Default::default(),
            m_player_win: Default::default(),
            m_player_win_losses: Default::default(),
            m_player_rank: Default::default(),
            m_player_officer_medal: Default::default(),
            m_map_preview: None,
            m_button_map_start_position: Default::default(),
            m_player_lookup: [-1; MAX_SLOTS],
            m_portrait_local_general: None,
            m_features_local_general: None,
            m_name_local_general: None,
        }
    }

    pub fn process_progress_gamespy(&mut self, player_id: i32, percentage: i32) {
        if percentage < 0 || percentage > 100 || player_id >= MAX_SLOTS as i32 || player_id < 0
            || self.m_player_lookup[player_id as usize] == -1 {
            // DEBUG_ASSERTCRASH(FALSE, ("Percentage %d was passed in for Player %d\n", percentage, playerId));
            return;
        }

        let lookup_idx = self.m_player_lookup[player_id as usize] as usize;
        if let Some(ref win) = self.m_progress_bars[lookup_idx] {
            // GadgetProgressBarSetProgress(m_progressBars[lookup_idx], percentage);
        }
    }
}

impl LoadScreen for GameSpyLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/GameSpyLoadScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the Multiplayer loadscreen"));
        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // m_mapPreview = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey("GameSpyLoadScreen.wnd:WinMapPreview"));
        // DEBUG_ASSERTCRASH(TheNetwork, ("Where the Heck is the Network!!!!"));
        // DEBUG_LOG(("NumPlayers %d\n", TheNetwork->getNumPlayers()));

        // GameSlot *lSlot = game->getSlot(game->getLocalSlotNum());
        // const PlayerTemplate* pt;
        // if (lSlot->getPlayerTemplate() >= 0)
        //     pt = ThePlayerTemplateStore->getNthPlayerTemplate(lSlot->getPlayerTemplate());
        // else
        //     pt = ThePlayerTemplateStore->findPlayerTemplate(TheNameKeyGenerator->nameToKey("FactionObserver"));

        // add portrait, features, and name for the local player's general
        // ... (similar to MultiPlayerLoadScreen)

        let mut net_slot = 0;
        // Loop through and make the loadscreen look all good.
        for i in 0..MAX_SLOTS {
            // Load all the progress bars, player names, sides, stats, etc.
            // ... (extensive initialization similar to C++ version)

            // get the slot man!
            // GameSpyGameSlot *slot = (GameSpyGameSlot *)game->getSlot(i);
            // if (!slot || !slot->isOccupied())
            //     continue;

            // Color houseColor = TheMultiplayerSettings->getColor(slot->getApparentColor())->getColor();

            // format the progress bar to house colors
            // ... (similar to MultiPlayerLoadScreen)

            // Get the stats for the player
            // PSPlayerStats stats = TheGameSpyPSMessageQueue->findPlayerStatsByID(slot->getProfileID());
            // ... (populate stats, rank, medals, wins/losses, disconnects)

            // m_playerLookup[i] = netSlot;
            // netSlot++;
        }

        for i in net_slot..MAX_SLOTS {
            if let Some(ref win) = self.m_player_win[i] {
                win.lock().unwrap().win_hide(true);
            }
        }

        // if (m_mapPreview) {
        //     const MapMetaData *mmd = TheMapCache->findMap(game->getMap());
        //     Image *image = getMapPreviewImage(game->getMap());
        //     m_mapPreview->winSetUserData((void *)mmd);
        //
        //     positionStartSpots(game, m_buttonMapStartPosition, m_mapPreview);
        //     updateMapStartSpots(game, m_buttonMapStartPosition, TRUE);
        //
        //     if (image) {
        //         m_mapPreview->winSetStatus(WIN_STATUS_IMAGE);
        //         m_mapPreview->winSetEnabledImage(0, image);
        //     } else {
        //         m_mapPreview->winClearStatus(WIN_STATUS_IMAGE);
        //     }
        // }

        // TheGameLogic->initTimeOutValues();
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_player_side[i] = None;
        }
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        // if (percent <= 100)
        //     TheNetwork->updateLoadProgress(percent);
        // TheNetwork->liteupdate();

        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, player_id: i32, percentage: i32) {
        self.process_progress_gamespy(player_id, percentage);
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for GameSpyLoadScreen {
    fn drop(&mut self) {
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_player_side[i] = None;
            self.m_player_lookup[i] = -1;
            self.m_player_favorite_factions[i] = None;
            self.m_player_total_disconnects[i] = None;
            self.m_player_win[i] = None;
            self.m_player_win_losses[i] = None;
        }
    }
}

//-----------------------------------------------------------------------------
// MapTransferLoadScreen Class
//-----------------------------------------------------------------------------
pub struct MapTransferLoadScreen {
    base: BaseLoadScreen,
    m_progress_bars: [Option<GameWindow>; MAX_SLOTS],
    m_player_names: [Option<GameWindow>; MAX_SLOTS],
    m_progress_text: [Option<GameWindow>; MAX_SLOTS],
    m_player_lookup: [i32; MAX_SLOTS],
    m_old_progress: [i32; MAX_SLOTS],
    m_file_name_text: Option<GameWindow>,
    m_timeout_text: Option<GameWindow>,
    m_old_timeout: i32,
}

impl MapTransferLoadScreen {
    pub fn new() -> Self {
        Self {
            base: BaseLoadScreen::new(),
            m_progress_bars: Default::default(),
            m_player_names: Default::default(),
            m_progress_text: Default::default(),
            m_player_lookup: [-1; MAX_SLOTS],
            m_old_progress: [-1; MAX_SLOTS],
            m_file_name_text: None,
            m_timeout_text: None,
            m_old_timeout: 0,
        }
    }

    pub fn process_progress_with_state(&mut self, player_id: i32, percentage: i32, state_str: String) {
        if percentage < 0 || percentage > 100 || player_id >= MAX_SLOTS as i32 || player_id < 0
            || self.m_player_lookup[player_id as usize] == -1 {
            // DEBUG_ASSERTCRASH(FALSE, ("Percentage %d was passed in for Player %d\n", percentage, playerId));
            return;
        }

        if self.m_old_progress[player_id as usize] == percentage {
            return;
        }
        self.m_old_progress[player_id as usize] = percentage;

        let translated_slot = self.m_player_lookup[player_id as usize] as usize;
        if let Some(ref win) = self.m_progress_bars[translated_slot] {
            // GadgetProgressBarSetProgress(m_progressBars[translatedSlot], percentage);
        }
        if let Some(ref win) = self.m_progress_text[translated_slot] {
            // GadgetStaticTextSetText(m_progressText[translatedSlot], TheGameText->fetch(stateStr));
        }
    }

    pub fn process_timeout(&mut self, seconds_left: i32) {
        if self.m_old_timeout == seconds_left {
            return;
        }
        self.m_old_timeout = seconds_left;

        if let Some(ref win) = self.m_timeout_text {
            // UnicodeString txt;
            // txt.format(TheGameText->fetch("MapTransfer:Timeout"), (secondsLeft/60), (secondsLeft%60));
            // GadgetStaticTextSetText(m_timeoutText, txt);
        }
    }

    pub fn set_current_filename(&mut self, filename: String) {
        if let Some(ref win) = self.m_file_name_text {
            // UnicodeString txt;
            // txt.translate(TheGameState->getMapLeafName(filename));
            // txt.format(TheGameText->fetch("MapTransfer:CurrentFile"), txt.str());
            // GadgetStaticTextSetText(m_fileNameText, txt);
        }
    }
}

impl LoadScreen for MapTransferLoadScreen {
    fn init(&mut self, game: Arc<Mutex<dyn GameInfoTrait>>) {
        // create the layout of the load screen
        // m_loadScreen = TheWindowManager->winCreateFromScript(AsciiString("Menus/MapTransferScreen.wnd"));
        // DEBUG_ASSERTCRASH(m_loadScreen, ("Can't initialize the map transfer loadscreen"));
        // if (!m_loadScreen)
        //     return;

        if let Some(ref win) = self.base.m_load_screen {
            let mut w = win.lock().unwrap();
            w.win_hide(false);
            w.win_bring_to_top();
        }

        // DEBUG_ASSERTCRASH(TheNetwork, ("Where the Heck is the Network?!!!!"));
        // DEBUG_LOG(("NumPlayers %d\n", TheNetwork->getNumPlayers()));

        // Load the Filename Text
        // winName.format("MapTransferScreen.wnd:StaticTextCurrentFile");
        // m_fileNameText = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
        // DEBUG_ASSERTCRASH(m_fileNameText, ("Can't initialize the filename for the map transfer loadscreen"));

        // Load the Timeout Text
        // winName.format("MapTransferScreen.wnd:StaticTextTimeout");
        // m_timeoutText = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
        // DEBUG_ASSERTCRASH(m_timeoutText, ("Can't initialize the timeout for the map transfer loadscreen"));

        let mut net_slot = 0;
        // Loop through and make the loadscreen look all good.
        for i in 0..MAX_SLOTS {
            // Load the Progress Bar
            // let win_name = format!("MapTransferScreen.wnd:ProgressLoad{}", i);
            // m_progressBars[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_progressBars[i], ("Can't initialize the progressbars for the map transfer loadscreen"));
            // GadgetProgressBarSetProgress(m_progressBars[i], 0);

            // Load the Player's name
            // let win_name = format!("MapTransferScreen.wnd:StaticTextPlayer{}", i);
            // m_playerNames[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_playerNames[i], ("Can't initialize the Names for the map transfer loadscreen"));

            // Load the Progress Text
            // let win_name = format!("MapTransferScreen.wnd:StaticTextProgress{}", i);
            // m_progressText[i] = TheWindowManager->winGetWindowFromId(m_loadScreen, TheNameKeyGenerator->nameToKey(winName));
            // DEBUG_ASSERTCRASH(m_progressText[i], ("Can't initialize the progress text for the map transfer loadscreen"));

            // get the slot man!
            // GameSlot *slot = game->getSlot(i);
            // if (!slot || !slot->isHuman())
            //     continue;

            // Color houseColor = TheMultiplayerSettings->getColor(slot->getApparentColor())->getColor();
            // GadgetProgressBarSetEnabledBarColor(m_progressBars[netSlot], houseColor);

            // UnicodeString name = slot->getName();
            // GadgetStaticTextSetText(m_playerNames[netSlot], name);
            // m_playerNames[netSlot]->winSetEnabledTextColors(houseColor, m_playerNames[netSlot]->winGetEnabledTextBorderColor());

            // GadgetStaticTextSetText(m_progressText[netSlot], UnicodeString::TheEmptyString);
            // m_progressText[netSlot]->winSetEnabledTextColors(houseColor, m_progressText[netSlot]->winGetEnabledTextBorderColor());

            // if ((i == 0 || (TheGameInfo->getConstSlot(i)->isHuman() && TheGameInfo->getConstSlot(i)->hasMap())) && m_progressBars[netSlot])
            //     m_progressBars[netSlot]->winHide(TRUE);

            // m_playerLookup[i] = netSlot;
            // netSlot++;
        }

        for i in net_slot..MAX_SLOTS {
            if let Some(ref win) = self.m_progress_bars[i] {
                win.lock().unwrap().win_hide(true);
            }
            if let Some(ref win) = self.m_player_names[i] {
                win.lock().unwrap().win_hide(true);
            }
            if let Some(ref win) = self.m_progress_text[i] {
                win.lock().unwrap().win_hide(true);
            }
        }
    }

    fn reset(&mut self) {
        self.base.m_load_screen = None;
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_progress_text[i] = None;
            self.m_player_lookup[i] = -1;
            self.m_old_progress[i] = -1;
        }
        self.m_file_name_text = None;
        self.m_timeout_text = None;
    }

    fn update_void(&mut self) {
        panic!("Call update(i32) instead. This update isn't supported");
    }

    fn update(&mut self, percent: i32) {
        // if (TheNetwork) {
        //     TheNetwork->liteupdate();
        // }

        // TheMouse->setCursorTooltip(UnicodeString::TheEmptyString);

        // Do this last!
        self.base.base_update(percent);
    }

    fn process_progress(&mut self, _player_id: i32, _percentage: i32) {
        panic!("Call processProgress(i32, i32, String) instead.");
    }

    fn set_progress_range(&mut self, _min: i32, _max: i32) {
        // Empty implementation
    }

    fn get_load_screen(&self) -> Option<GameWindow> {
        self.base.m_load_screen.clone()
    }

    fn set_load_screen(&mut self, window: Option<GameWindow>) {
        self.base.m_load_screen = window;
    }
}

impl Drop for MapTransferLoadScreen {
    fn drop(&mut self) {
        for i in 0..MAX_SLOTS {
            self.m_progress_bars[i] = None;
            self.m_player_names[i] = None;
            self.m_progress_text[i] = None;
            self.m_player_lookup[i] = -1;
            self.m_old_progress[i] = -1;
        }
        self.m_file_name_text = None;
        self.m_timeout_text = None;
    }
}

//-----------------------------------------------------------------------------
// Helper Functions
//-----------------------------------------------------------------------------

// accepts the number of chars to advance, the window we're concerned with,
// the total text for final display, and the current position of the readout
// returns the updated position of the readout
fn update_teletype_text(
    num_chars: usize,
    window: Option<&GameWindow>,
    full_text: &str,
    current_text_pos: usize
) -> usize {
    if window.is_none() {
        // DEBUG_ASSERTCRASH(window, ("No window for teletype text update"));
        return current_text_pos;
    }

    // UnicodeString currentText = GadgetStaticTextGetText(window);
    let mut pos = current_text_pos;

    for _ in 0..num_chars {
        if pos < full_text.len() {
            // WideChar wChar = full_text.getCharAt(current_text_pos);
            // currentText.concat(wChar);
            pos += 1;
        }
    }

    // GadgetStaticTextSetText(window, currentText);
    pos
}

// External helper functions referenced in C++ code
pub fn position_start_spots(
    _game: Arc<Mutex<dyn GameInfoTrait>>,
    _button_map_start_positions: &[Option<GameWindow>; MAX_SLOTS],
    _map_window: Option<&GameWindow>
) {
    // Implementation for positioning start spots on map
}

pub fn update_map_start_spots(
    _game: Arc<Mutex<dyn GameInfoTrait>>,
    _button_map_start_positions: &[Option<GameWindow>; MAX_SLOTS],
    _on_load_screen: bool
) {
    // Implementation for updating map start spots
}

pub fn position_additional_images(
    _mmd: *const (),
    _map_window: Option<&GameWindow>,
    _force: bool
) {
    // Implementation for positioning additional images
}
