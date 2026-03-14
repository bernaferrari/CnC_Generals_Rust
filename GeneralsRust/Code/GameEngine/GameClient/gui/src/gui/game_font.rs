use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameFont.cpp",
    "crate::gui::game_font",
    "Game Font",
    "Bridges legacy font naming and text intent onto GPUI text styles.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameFontPort {
    pub name: String,
    pub point_size: i32,
    pub bold: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FontLibraryPort {
    pub fonts: Vec<GameFontPort>,
}

impl FontLibraryPort {
    pub fn init(&mut self) {}

    pub fn reset(&mut self) {
        self.delete_all_fonts();
    }

    pub fn link_font(&mut self, font: GameFontPort) {
        self.fonts.insert(0, font);
    }

    pub fn unlink_font(&mut self, name: &str) -> Option<GameFontPort> {
        let index = self.fonts.iter().position(|font| font.name == name)?;
        Some(self.fonts.remove(index))
    }

    pub fn delete_all_fonts(&mut self) {
        self.fonts.clear();
    }

    pub fn defaults() -> Self {
        Self {
            fonts: vec![
                GameFontPort {
                    name: "CommandBar".to_string(),
                    point_size: 14,
                    bold: true,
                },
                GameFontPort {
                    name: "MenuBody".to_string(),
                    point_size: 16,
                    bold: false,
                },
            ],
        }
    }

    pub fn get_font(&mut self, name: &str, point_size: i32, bold: bool) -> Option<&GameFontPort> {
        if let Some(index) = self.fonts.iter().position(|font| {
            font.name == name && font.point_size == point_size && font.bold == bold
        }) {
            return self.fonts.get(index);
        }

        self.link_font(GameFontPort {
            name: name.to_string(),
            point_size,
            bold,
        });
        self.fonts.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlink_font_removes_matching_font() {
        let mut library = FontLibraryPort::defaults();
        let removed = library.unlink_font("CommandBar");

        assert!(removed.is_some());
        assert!(library.fonts.iter().all(|font| font.name != "CommandBar"));
    }

    #[test]
    fn get_font_loads_missing_font() {
        let mut library = FontLibraryPort::default();
        let font = library
            .get_font("MenuBody", 16, false)
            .expect("font should be created");

        assert_eq!(font.name, "MenuBody");
        assert_eq!(library.fonts.len(), 1);
    }
}
