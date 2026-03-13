use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "HeaderTemplate.cpp",
    "crate::gui::header_template",
    "Header Template",
    "Carries reusable shell header presentation data for multiple legacy layouts.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeaderTemplatePort {
    pub name: String,
    pub font_name: String,
    pub point: i32,
    pub bold: bool,
}

#[derive(Clone, Debug, Default)]
pub struct HeaderTemplateManagerPort {
    pub templates: Vec<HeaderTemplatePort>,
    pub loaded_file: Option<String>,
}

impl HeaderTemplateManagerPort {
    pub fn init_defaults() -> Self {
        Self {
            loaded_file: Some("Data/English/HeaderTemplate.ini".to_string()),
            templates: vec![
                HeaderTemplatePort {
                    name: "MainMenuHeader".to_string(),
                    font_name: "MenuBody".to_string(),
                    point: 18,
                    bold: true,
                },
                HeaderTemplatePort {
                    name: "PopupHeader".to_string(),
                    font_name: "CommandBar".to_string(),
                    point: 14,
                    bold: true,
                },
            ],
        }
    }

    pub fn find_header_template(&self, name: &str) -> Option<&HeaderTemplatePort> {
        self.templates.iter().find(|template| template.name == name)
    }

    pub fn new_header_template(&mut self, name: impl Into<String>) -> &HeaderTemplatePort {
        self.templates.insert(
            0,
            HeaderTemplatePort {
                name: name.into(),
                font_name: String::new(),
                point: 0,
                bold: false,
            },
        );
        &self.templates[0]
    }

    pub fn populate_game_fonts(
        &self,
        font_library: &mut crate::gui::game_font::FontLibraryPort,
    ) -> Vec<Option<crate::gui::game_font::GameFontPort>> {
        self.templates
            .iter()
            .map(|template| {
                font_library
                    .get_font(&template.font_name, template.point, template.bold)
                    .cloned()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::game_font::FontLibraryPort;

    #[test]
    fn populate_game_fonts_resolves_each_template() {
        let headers = HeaderTemplateManagerPort::init_defaults();
        let mut fonts = FontLibraryPort::default();
        let resolved = headers.populate_game_fonts(&mut fonts);

        assert_eq!(resolved.len(), headers.templates.len());
        assert!(resolved.iter().all(|font| font.is_some()));
    }
}
