use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindowTransitions.cpp",
    "crate::gui::game_window_transitions",
    "Game Window Transitions",
    "Maps legacy window transition handlers onto GPUI presentation and timing hooks.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionInstancePort {
    pub style_name: String,
    pub current_frame: i32,
    pub frame_length: i32,
    pub is_finished: bool,
    pub reversed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionGroupPort {
    pub name: String,
    pub fire_once: bool,
    pub current_frame: i32,
    pub direction_multiplier: i32,
    pub windows: Vec<TransitionInstancePort>,
}

#[derive(Clone, Debug, Default)]
pub struct TransitionHandlerPort {
    pub groups: Vec<TransitionGroupPort>,
    pub current_group: Option<String>,
    pub pending_group: Option<String>,
    pub draw_group: Option<String>,
    pub secondary_draw_group: Option<String>,
}

impl TransitionHandlerPort {
    pub fn trigger(&mut self, style_name: impl Into<String>, frame_length: i32) {
        let style_name = style_name.into();
        self.groups.push(TransitionGroupPort {
            name: style_name.clone(),
            fire_once: true,
            current_frame: 0,
            direction_multiplier: 1,
            windows: vec![TransitionInstancePort {
                style_name,
                current_frame: 0,
                frame_length,
                is_finished: false,
                reversed: false,
            }],
        });
        self.current_group = self.groups.last().map(|group| group.name.clone());
    }

    pub fn get_new_group(&mut self, name: impl Into<String>) -> Option<&mut TransitionGroupPort> {
        let name = name.into();
        if self
            .groups
            .iter()
            .any(|group| group.name.eq_ignore_ascii_case(&name))
        {
            return None;
        }
        self.groups.push(TransitionGroupPort {
            name,
            fire_once: false,
            current_frame: 0,
            direction_multiplier: 1,
            windows: Vec::new(),
        });
        self.groups.last_mut()
    }

    pub fn add_window_to_group(
        &mut self,
        group_name: &str,
        style_name: impl Into<String>,
        frame_length: i32,
    ) {
        if let Some(group) = self.find_group_mut(group_name) {
            group.windows.push(TransitionInstancePort {
                style_name: style_name.into(),
                current_frame: 0,
                frame_length,
                is_finished: false,
                reversed: false,
            });
        }
    }

    pub fn reset(&mut self) {
        self.current_group = None;
        self.pending_group = None;
        self.draw_group = None;
        self.secondary_draw_group = None;
    }

    pub fn update(&mut self) {
        if self.draw_group != self.current_group {
            self.secondary_draw_group = self.draw_group.clone();
        } else {
            self.secondary_draw_group = None;
        }

        self.draw_group = self.current_group.clone();

        if let Some(current_name) = self.current_group.clone() {
            if let Some(group) = self.find_group_mut(&current_name) {
                if !group_is_finished(group) {
                    update_group(group);
                }
            }
        }

        if let Some(current_name) = self.current_group.clone() {
            let current_finished = self
                .find_group(&current_name)
                .map(group_is_finished)
                .unwrap_or(true);
            let current_fire_once = self
                .find_group(&current_name)
                .map(|group| group.fire_once)
                .unwrap_or(false);
            let current_reversed = self
                .find_group(&current_name)
                .map(|group| group.direction_multiplier < 0)
                .unwrap_or(false);

            if current_finished && (current_fire_once || current_reversed) {
                self.current_group = None;
            }
        }

        if self.current_group.is_none() && self.pending_group.is_some() {
            self.current_group = self.pending_group.take();
        }
    }

    pub fn set_group(&mut self, group_name: &str, immediate: bool) {
        if group_name.is_empty() && immediate {
            self.current_group = None;
            return;
        }

        if immediate && self.current_group.is_some() {
            if let Some(current_name) = self.current_group.clone() {
                if let Some(group) = self.find_group_mut(&current_name) {
                    skip_group(group);
                }
            }
            self.current_group = self.find_group(group_name).map(|group| group.name.clone());
            if let Some(group) = self.find_group_mut(group_name) {
                init_group(group);
            }
            return;
        }

        if let Some(current_name) = self.current_group.clone() {
            if let Some(group) = self.find_group_mut(&current_name) {
                if !group.fire_once && !group_is_reversed(group) {
                    reverse_group(group);
                }
            }
            self.pending_group = self.find_group(group_name).map(|group| group.name.clone());
            if let Some(group) = self.find_group_mut(group_name) {
                init_group(group);
            }
            return;
        }

        self.current_group = self.find_group(group_name).map(|group| group.name.clone());
        if let Some(group) = self.find_group_mut(group_name) {
            init_group(group);
        }
    }

    pub fn reverse(&mut self, group_name: &str) {
        if self.current_group.as_deref() == Some(group_name) {
            if let Some(group) = self.find_group_mut(group_name) {
                reverse_group(group);
            }
            return;
        }
        if self.pending_group.as_deref() == Some(group_name) {
            self.pending_group = None;
            return;
        }

        if let Some(current_name) = self.current_group.clone() {
            if let Some(group) = self.find_group_mut(&current_name) {
                skip_group(group);
            }
        }
        if let Some(pending_name) = self.pending_group.clone() {
            if let Some(group) = self.find_group_mut(&pending_name) {
                skip_group(group);
            }
        }

        self.current_group = self.find_group(group_name).map(|group| group.name.clone());
        if let Some(group) = self.find_group_mut(group_name) {
            init_group(group);
            skip_group(group);
            reverse_group(group);
        }
        self.pending_group = None;
    }

    pub fn remove(&mut self, group_name: &str, skip_pending: bool) {
        if self.pending_group.as_deref() == Some(group_name) {
            if skip_pending {
                if let Some(group) = self.find_group_mut(group_name) {
                    skip_group(group);
                }
            }
            self.pending_group = None;
        }
        if self.current_group.as_deref() == Some(group_name) {
            if let Some(group) = self.find_group_mut(group_name) {
                skip_group(group);
            }
            self.current_group = None;
            if self.pending_group.is_some() {
                self.current_group = self.pending_group.take();
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.current_group
            .as_deref()
            .and_then(|name| self.find_group(name))
            .map(group_is_finished)
            .unwrap_or(true)
    }

    fn find_group(&self, name: &str) -> Option<&TransitionGroupPort> {
        self.groups
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(name))
    }

    fn find_group_mut(&mut self, name: &str) -> Option<&mut TransitionGroupPort> {
        self.groups
            .iter_mut()
            .find(|group| group.name.eq_ignore_ascii_case(name))
    }
}

fn init_group(group: &mut TransitionGroupPort) {
    group.current_frame = 0;
    group.direction_multiplier = 1;
    for window in &mut group.windows {
        window.current_frame = 0;
        window.is_finished = false;
        window.reversed = false;
    }
}

fn update_group(group: &mut TransitionGroupPort) {
    group.current_frame += group.direction_multiplier;
    for window in &mut group.windows {
        window.current_frame += group.direction_multiplier;
        if group.direction_multiplier >= 0 {
            if window.current_frame >= window.frame_length {
                window.is_finished = true;
            }
        } else if window.current_frame <= 0 {
            window.is_finished = true;
        }
    }
}

fn group_is_finished(group: &TransitionGroupPort) -> bool {
    group.windows.iter().all(|window| window.is_finished)
}

fn group_is_reversed(group: &TransitionGroupPort) -> bool {
    group.direction_multiplier < 0
}

fn reverse_group(group: &mut TransitionGroupPort) {
    let total_frames = group
        .windows
        .iter()
        .map(|window| window.frame_length)
        .max()
        .unwrap_or(0);
    group.direction_multiplier = -1;
    group.current_frame = total_frames;
    for window in &mut group.windows {
        window.reversed = true;
        window.is_finished = false;
        window.current_frame = window.frame_length;
    }
}

fn skip_group(group: &mut TransitionGroupPort) {
    for window in &mut group.windows {
        window.is_finished = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_transitions_are_removed_on_update() {
        let mut handler = TransitionHandlerPort::default();
        handler.trigger("FlashTransition", 1);
        handler.update();

        assert!(handler.is_finished());
    }

    #[test]
    fn pending_group_becomes_current_after_current_finishes() {
        let mut handler = TransitionHandlerPort::default();
        handler.get_new_group("A");
        handler.add_window_to_group("A", "Flash", 1);
        handler.get_new_group("B");
        handler.add_window_to_group("B", "Flash", 1);

        handler.set_group("A", false);
        handler.set_group("B", false);
        handler.update();

        assert_eq!(handler.current_group.as_deref(), Some("B"));
    }
}
