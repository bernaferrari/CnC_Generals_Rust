//! Shared fullscreen movie lifecycle for the native and standalone renderers.

use super::display_fx;
use crate::game_text::GameText;
use crate::gui::display_string::{DisplayStringHandle, get_display_string_manager};
use crate::gui::font::{FontDesc, get_font_library};
use crate::video_buffer::{SoftwareVideoBuffer, VideoBuffer, VideoBufferType};
use crate::video_stream::VideoStreamInterface;
use std::time::{Duration, Instant};

/// Codec-neutral fullscreen playback shared by standalone Display and the native receiver.
pub(crate) struct FullscreenMovie {
    playing: Option<(Box<dyn VideoStreamInterface>, SoftwareVideoBuffer)>,
    movie_hold_time_ms: i32,
    copyright_hold_time_ms: i32,
    movie_start_time: Option<Instant>,
    copyright_start_time: Option<Instant>,
    copyright_display_string: Option<DisplayStringHandle>,
}
impl Default for FullscreenMovie {
    fn default() -> Self {
        Self {
            playing: None,
            movie_hold_time_ms: -1,
            copyright_hold_time_ms: -1,
            movie_start_time: None,
            copyright_start_time: None,
            copyright_display_string: None,
        }
    }
}

impl FullscreenMovie {
    pub(crate) fn start(
        &mut self,
        stream: Box<dyn VideoStreamInterface>,
        movie_hold_time_ms: i32,
        copyright_hold_time_ms: i32,
    ) -> bool {
        let (Ok(width), Ok(height)) = (
            u32::try_from(stream.width()),
            u32::try_from(stream.height()),
        ) else {
            stream.close();
            return false;
        };
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        if !buffer.allocate(width, height) {
            stream.close();
            return false;
        }
        self.movie_hold_time_ms = movie_hold_time_ms;
        self.copyright_hold_time_ms = copyright_hold_time_ms;
        self.movie_start_time = (movie_hold_time_ms >= 0).then(Instant::now);
        self.playing = Some((stream, buffer));
        true
    }

    pub(crate) fn stop(&mut self, fx: &mut display_fx::DisplayFxState) {
        if let Some((stream, _)) = self.playing.take() {
            stream.close();
        }
        if let Some(display_string) = self.copyright_display_string.take() {
            get_display_string_manager().free_display_string(display_string);
        }
        fx.clear_movie_frame();
        fx.set_copyright_overlay(None);
        self.movie_hold_time_ms = -1;
        self.copyright_hold_time_ms = -1;
        self.movie_start_time = None;
        self.copyright_start_time = None;
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    fn ensure_copyright_display_string(&mut self, fx: &mut display_fx::DisplayFxState) {
        let mut manager = get_display_string_manager();
        let display_string = manager.new_display_string();
        let (font_name, font_size, font_bold) = {
            let language = crate::global_language::get_global_language_data();
            let lang = language.read().ok();
            if let Some(lang) = lang.as_ref() {
                if !lang.copyright_font.name.is_empty() {
                    (
                        lang.copyright_font.name.clone(),
                        lang.adjust_font_size(lang.copyright_font.size),
                        lang.copyright_font.bold,
                    )
                } else {
                    ("Courier".to_string(), lang.adjust_font_size(12), true)
                }
            } else {
                ("Courier".to_string(), 12, true)
            }
        };
        let (text_w, text_h) = {
            let mut display_string_mut = display_string.borrow_mut();
            display_string_mut.set_text(GameText::fetch("GUI:EACopyright"));
            let font_desc = FontDesc::new(&font_name, font_size, font_bold);
            let font_library_handle = get_font_library();
            let mut font_library = font_library_handle
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Ok(font) = font_library.get_font(&font_desc) {
                display_string_mut.set_font(font);
            }
            display_string_mut.get_size()
        };
        fx.set_copyright_overlay(Some(display_fx::CopyrightOverlay {
            text: GameText::fetch("GUI:EACopyright"),
            width: text_w as f32,
            height: text_h.max(font_size) as f32,
            font_size: font_size.max(1) as f32,
        }));
        self.copyright_display_string = Some(display_string);
        self.copyright_start_time = Some(Instant::now());
    }

    pub(crate) fn advance(&mut self, fx: &mut display_fx::DisplayFxState) {
        let should_stop = 'frame: {
            let Some((stream, buffer)) = self.playing.as_mut() else {
                return;
            };

            if stream.has_decode_failed() {
                break 'frame true;
            }
            if !stream.is_frame_ready() {
                return;
            }

            stream.frame_decompress();
            if stream.has_decode_failed() {
                break 'frame true;
            }
            stream.frame_render(buffer);
            if let Some((w, h, rgba)) = display_fx::video_buffer_rgba_mut(buffer) {
                fx.store_movie_frame(w, h, rgba);
            }

            if stream.frame_index() != stream.frame_count() - 1 {
                stream.frame_next();
                break 'frame stream.has_decode_failed();
            }

            if self.copyright_hold_time_ms >= 0 || self.movie_hold_time_ms >= 0 {
                if self.copyright_start_time.is_none() && self.copyright_hold_time_ms >= 0 {
                    self.ensure_copyright_display_string(fx);
                }

                let now = Instant::now();
                let movie_elapsed = self
                    .movie_start_time
                    .map(|start| now.duration_since(start))
                    .unwrap_or_default();
                let copyright_elapsed = self
                    .copyright_start_time
                    .map(|start| now.duration_since(start))
                    .unwrap_or_default();

                if (self.movie_hold_time_ms >= 0
                    && movie_elapsed
                        >= Duration::from_millis(self.movie_hold_time_ms.max(0) as u64))
                    && (self.copyright_hold_time_ms >= 0
                        && copyright_elapsed
                            >= Duration::from_millis(self.copyright_hold_time_ms.max(0) as u64))
                {
                    self.movie_hold_time_ms = -1;
                    self.copyright_hold_time_ms = -1;
                    self.movie_start_time = None;
                    self.copyright_start_time = None;
                }
                false
            } else {
                true
            }
        };

        if should_stop {
            if let Some((stream, _)) = self.playing.take() {
                if stream.has_decode_failed() {
                    stream.close();
                } else {
                    stream.finish();
                }
            }
            self.stop(fx);
        }
    }
}
