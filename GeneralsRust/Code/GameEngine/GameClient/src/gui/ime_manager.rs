//! IME manager (cross-platform state manager).

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use super::game_window::GameWindow;

#[derive(Debug, Clone)]
pub enum ImeMessage {
    StartComposition,
    EndComposition,
    UpdateComposition {
        text: String,
        cursor_pos: usize,
    },
    ResultString(String),
    CandidateList {
        candidates: Vec<String>,
        selected_index: usize,
        page_start: usize,
        page_size: usize,
        index_base: i32,
    },
    ClearCandidateList,
}

#[derive(Debug)]
pub struct ImeManager {
    window: Option<Weak<RefCell<GameWindow>>>,
    enabled: bool,
    composing: bool,
    composition_string: String,
    result_string: String,
    composition_cursor_pos: usize,
    index_base: i32,
    page_start: usize,
    page_size: usize,
    selected_index: usize,
    candidates: Vec<String>,
}

impl Default for ImeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImeManager {
    pub fn new() -> Self {
        Self {
            window: None,
            enabled: true,
            composing: false,
            composition_string: String::new(),
            result_string: String::new(),
            composition_cursor_pos: 0,
            index_base: 0,
            page_start: 0,
            page_size: 0,
            selected_index: 0,
            candidates: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        self.reset();
    }

    pub fn reset(&mut self) {
        self.window = None;
        self.composing = false;
        self.composition_string.clear();
        self.result_string.clear();
        self.candidates.clear();
        self.composition_cursor_pos = 0;
        self.index_base = 0;
        self.page_start = 0;
        self.page_size = 0;
        self.selected_index = 0;
    }

    pub fn update(&mut self) {}

    pub fn attach(&mut self, window: Rc<RefCell<GameWindow>>) {
        self.window = Some(Rc::downgrade(&window));
    }

    pub fn detach(&mut self) {
        self.window = None;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
        self.composing = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_attached_to(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        self.window
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|w| Rc::ptr_eq(&w, window))
            .unwrap_or(false)
    }

    pub fn get_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.window.as_ref().and_then(|weak| weak.upgrade())
    }

    pub fn is_composing(&self) -> bool {
        self.composing
    }

    pub fn composition_string(&self) -> &str {
        &self.composition_string
    }

    pub fn composition_cursor_position(&self) -> usize {
        self.composition_cursor_pos
    }

    pub fn index_base(&self) -> i32 {
        self.index_base
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn candidate(&self, index: usize) -> Option<&str> {
        self.candidates.get(index).map(|s| s.as_str())
    }

    pub fn selected_candidate_index(&self) -> usize {
        self.selected_index
    }

    pub fn candidate_page_size(&self) -> usize {
        self.page_size
    }

    pub fn candidate_page_start(&self) -> usize {
        self.page_start
    }

    pub fn result_string(&self) -> &str {
        &self.result_string
    }

    pub fn service_ime_message(&mut self, message: ImeMessage) -> bool {
        if !self.enabled {
            return false;
        }

        match message {
            ImeMessage::StartComposition => {
                self.composing = true;
                self.composition_string.clear();
                self.result_string.clear();
            }
            ImeMessage::EndComposition => {
                self.composing = false;
            }
            ImeMessage::UpdateComposition { text, cursor_pos } => {
                self.composition_string = text;
                self.composition_cursor_pos = cursor_pos;
                self.composing = true;
            }
            ImeMessage::ResultString(text) => {
                self.result_string = text;
                self.composing = false;
            }
            ImeMessage::CandidateList {
                candidates,
                selected_index,
                page_start,
                page_size,
                index_base,
            } => {
                self.candidates = candidates;
                self.selected_index = selected_index;
                self.page_start = page_start;
                self.page_size = page_size;
                self.index_base = index_base;
            }
            ImeMessage::ClearCandidateList => {
                self.candidates.clear();
                self.page_start = 0;
                self.page_size = 0;
                self.selected_index = 0;
            }
        }

        true
    }
}

thread_local! {
    static THE_IME_MANAGER: Arc<Mutex<ImeManager>> = Arc::new(Mutex::new(ImeManager::new()));
}

pub fn get_ime_manager() -> Arc<Mutex<ImeManager>> {
    THE_IME_MANAGER.with(|manager| manager.clone())
}
