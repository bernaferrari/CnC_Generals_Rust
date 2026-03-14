use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "IMEManager.cpp",
    "crate::gui::ime_manager",
    "IME Manager",
    "Tracks IME attachment and candidate interaction for text-entry focused layouts.",
);

#[derive(Clone, Debug, Default)]
pub struct ImeManagerPort {
    pub attached_window: Option<i32>,
    pub disabled_count: usize,
    pub composing: bool,
    pub composition_string: String,
    pub composition_cursor_position: usize,
    pub candidates: Vec<String>,
    pub selected_candidate_index: usize,
    pub page_size: usize,
    pub page_start: usize,
    pub index_base: usize,
    pub unicode_ime: bool,
    pub candidate_window_open: bool,
    pub candidate_window_lines: usize,
}

impl ImeManagerPort {
    pub fn attach(&mut self, window_id: i32) {
        if self.attached_window != Some(window_id) {
            self.detatch();
            if self.disabled_count == 0 {
                self.update_properties();
            }
            self.attached_window = Some(window_id);
        }
    }

    pub fn detatch(&mut self) {
        self.attached_window = None;
        self.composing = false;
        self.composition_string.clear();
        self.candidate_window_open = false;
    }

    pub fn enable(&mut self) {
        self.disabled_count = self.disabled_count.saturating_sub(1);
    }

    pub fn disable(&mut self) {
        self.disabled_count += 1;
    }

    pub fn is_enabled(&self) -> bool {
        self.disabled_count == 0
    }

    pub fn set_composition(&mut self, text: impl Into<String>, cursor: usize) {
        self.composing = true;
        self.composition_string = text.into();
        self.composition_cursor_position = cursor;
    }

    pub fn is_attached_to(&self, window_id: i32) -> bool {
        self.attached_window == Some(window_id)
    }

    pub fn update_properties(&mut self) {
        self.index_base = 0;
        self.unicode_ime = true;
    }

    pub fn open_candidate_list(&mut self) {
        self.update_properties();
        self.resize_candidate_window(self.page_size.max(1));
        self.candidate_window_open = true;
    }

    pub fn close_candidate_list(&mut self) {
        self.candidate_window_open = false;
        self.candidates.clear();
        self.page_size = 0;
        self.page_start = 0;
        self.selected_candidate_index = 0;
    }

    pub fn update_candidate_list(
        &mut self,
        candidates: Vec<String>,
        page_size: usize,
        page_start: usize,
        selected_index: usize,
    ) {
        self.candidates = candidates;
        self.page_size = page_size.max(1);
        self.page_start = page_start.min(self.candidates.len());
        self.selected_candidate_index = selected_index.min(self.candidates.len().saturating_sub(1));
    }

    pub fn resize_candidate_window(&mut self, page_size: usize) {
        self.candidate_window_lines = page_size;
    }

    pub fn get_candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn get_candidate(&self, index: usize) -> &str {
        self.candidates.get(index).map(String::as_str).unwrap_or("")
    }

    pub fn get_selected_candidate_index(&self) -> usize {
        self.selected_candidate_index
    }

    pub fn get_candidate_page_size(&self) -> usize {
        self.page_size
    }

    pub fn get_candidate_page_start(&self) -> usize {
        self.page_start
    }

    pub fn get_index_base(&self) -> usize {
        self.index_base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_candidate_list_preserves_page_size() {
        let mut ime = ImeManagerPort::default();
        ime.update_candidate_list(vec!["A".to_string(), "B".to_string()], 2, 0, 1);
        ime.open_candidate_list();

        assert!(ime.candidate_window_open);
        assert_eq!(ime.get_candidate_page_size(), 2);
        assert_eq!(ime.get_selected_candidate_index(), 1);
    }
}
