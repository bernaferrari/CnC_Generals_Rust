//! INI Animation parsing module (Anim2D)
//! Author: Colin Day, July 2002
//! Rust port: 2025

use super::ini::{FieldParse, INIError, INIResult, INI};
use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini_mapped_image::get_mapped_image_collection;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

/// Animation mode (matches Anim2DMode in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anim2DMode {
    Invalid = 0,
    Once,
    OnceBackwards,
    Loop,
    LoopBackwards,
    PingPong,
    PingPongBackwards,
}

const ANIM_2D_MODE_NAMES: [&str; 7] = [
    "NONE",
    "ONCE",
    "ONCE_BACKWARDS",
    "LOOP",
    "LOOP_BACKWARDS",
    "PING_PONG",
    "PING_PONG_BACKWARDS",
];

impl Anim2DMode {
    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Invalid),
            1 => Some(Self::Once),
            2 => Some(Self::OnceBackwards),
            3 => Some(Self::Loop),
            4 => Some(Self::LoopBackwards),
            5 => Some(Self::PingPong),
            6 => Some(Self::PingPongBackwards),
            _ => None,
        }
    }
}

/// 2D animation template definition (mirrors GameClient/Anim2DTemplate).
#[derive(Debug)]
pub struct Anim2DTemplate {
    next_template: Option<Arc<RwLock<Anim2DTemplate>>>,
    name: AsciiString,
    images: Vec<Option<String>>,
    num_frames: u16,
    frames_between_updates: u16,
    anim_mode: Anim2DMode,
    randomize_start_frame: bool,
}

impl Anim2DTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            next_template: None,
            name,
            images: Vec::new(),
            num_frames: 0,
            frames_between_updates: 0,
            anim_mode: Anim2DMode::Loop,
            randomize_start_frame: false,
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn get_num_frames(&self) -> u16 {
        self.num_frames
    }

    pub fn get_num_frames_between_updates(&self) -> u16 {
        self.frames_between_updates
    }

    pub fn get_anim_mode(&self) -> Anim2DMode {
        self.anim_mode
    }

    pub fn is_randomized_start_frame(&self) -> bool {
        self.randomize_start_frame
    }

    pub fn get_next_template(&self) -> Option<Arc<RwLock<Anim2DTemplate>>> {
        self.next_template.clone()
    }

    pub fn set_next_template(&mut self, next: Option<Arc<RwLock<Anim2DTemplate>>>) {
        self.next_template = next;
    }

    pub fn get_frame_name(&self, frame_number: u16) -> Option<&str> {
        self.images
            .get(frame_number as usize)
            .and_then(|entry| entry.as_deref())
    }

    pub fn allocate_images(&mut self, num_frames: u16) {
        self.num_frames = num_frames;
        self.images = vec![None; num_frames as usize];
    }

    pub fn store_image_name(&mut self, name: Option<String>) -> INIResult<()> {
        let Some(name) = name else {
            return Ok(());
        };

        for entry in &mut self.images {
            if entry.is_none() {
                *entry = Some(name);
                return Ok(());
            }
        }

        Err(INIError::InvalidData)
    }

    fn parse_num_images(
        ini: &mut INI,
        template: &mut Anim2DTemplate,
        tokens: &[&str],
    ) -> INIResult<()> {
        let token = tokens.first().ok_or(INIError::InvalidData)?;
        let num_frames = INI::parse_unsigned_int(token)?;
        if num_frames < 1 {
            return Err(INIError::InvalidData);
        }
        template.allocate_images(num_frames.min(u16::MAX as u32) as u16);
        Ok(())
    }

    fn parse_image(ini: &mut INI, template: &mut Anim2DTemplate, tokens: &[&str]) -> INIResult<()> {
        let token = tokens.first().ok_or(INIError::InvalidData)?;
        let image_name = INI::parse_ascii_string(token)?;

        let Some(collection) = get_mapped_image_collection() else {
            return Ok(());
        };
        let collection = collection.read();
        let found = collection.find_image_by_name(&image_name);

        if found.is_none() {
            // Missing images are tolerated (builder/editor scenario).
            return Ok(());
        }

        template.store_image_name(Some(image_name))
    }

    fn parse_image_sequence(
        ini: &mut INI,
        template: &mut Anim2DTemplate,
        tokens: &[&str],
    ) -> INIResult<()> {
        if template.num_frames == 0 {
            return Err(INIError::InvalidData);
        }

        let token = tokens.first().ok_or(INIError::InvalidData)?;
        let base_name = INI::parse_ascii_string(token)?;

        let Some(collection) = get_mapped_image_collection() else {
            return Ok(());
        };
        let collection = collection.read();

        for index in 0..template.num_frames {
            let image_name = format!("{}{:03}", base_name, index);
            if collection.find_image_by_name(&image_name).is_none() {
                return Err(INIError::InvalidData);
            }
            template.store_image_name(Some(image_name))?;
        }

        Ok(())
    }

    fn parse_anim_mode(
        _ini: &mut INI,
        template: &mut Anim2DTemplate,
        tokens: &[&str],
    ) -> INIResult<()> {
        let token = tokens.first().ok_or(INIError::InvalidData)?;
        let index = INI::parse_index_list(token, &ANIM_2D_MODE_NAMES)?;
        template.anim_mode = Anim2DMode::from_index(index).ok_or(INIError::InvalidData)?;
        Ok(())
    }

    fn parse_animation_delay(
        _ini: &mut INI,
        template: &mut Anim2DTemplate,
        tokens: &[&str],
    ) -> INIResult<()> {
        let token = tokens.first().ok_or(INIError::InvalidData)?;
        let frames = INI::parse_duration_unsigned_int(token)?;
        template.frames_between_updates = frames.min(u16::MAX as u32) as u16;
        Ok(())
    }

    fn parse_randomize_start_frame(
        _ini: &mut INI,
        template: &mut Anim2DTemplate,
        tokens: &[&str],
    ) -> INIResult<()> {
        let token = tokens.first().ok_or(INIError::InvalidData)?;
        template.randomize_start_frame = INI::parse_bool(token)?;
        Ok(())
    }

    pub fn get_field_parse() -> &'static [FieldParse<Anim2DTemplate>] {
        &ANIM_2D_FIELD_PARSE_TABLE
    }
}

const ANIM_2D_FIELD_PARSE_TABLE: &[FieldParse<Anim2DTemplate>] = &[
    FieldParse {
        token: "NumberImages",
        parse: Anim2DTemplate::parse_num_images,
    },
    FieldParse {
        token: "Image",
        parse: Anim2DTemplate::parse_image,
    },
    FieldParse {
        token: "ImageSequence",
        parse: Anim2DTemplate::parse_image_sequence,
    },
    FieldParse {
        token: "AnimationMode",
        parse: Anim2DTemplate::parse_anim_mode,
    },
    FieldParse {
        token: "AnimationDelay",
        parse: Anim2DTemplate::parse_animation_delay,
    },
    FieldParse {
        token: "RandomizeStartFrame",
        parse: Anim2DTemplate::parse_randomize_start_frame,
    },
];

/// Animation template collection (mirrors Anim2DCollection template list).
#[derive(Debug, Default)]
pub struct Anim2DCollection {
    template_head: Option<Arc<RwLock<Anim2DTemplate>>>,
}

impl Anim2DCollection {
    pub fn new() -> Self {
        Self {
            template_head: None,
        }
    }

    pub fn get_template_head(&self) -> Option<Arc<RwLock<Anim2DTemplate>>> {
        self.template_head.clone()
    }

    pub fn get_next_template(
        &self,
        template: &Arc<RwLock<Anim2DTemplate>>,
    ) -> Option<Arc<RwLock<Anim2DTemplate>>> {
        template.read().get_next_template()
    }

    pub fn find_template(&self, name: &AsciiString) -> Option<Arc<RwLock<Anim2DTemplate>>> {
        let mut current = self.template_head.clone();
        while let Some(node) = current {
            if node.read().get_name() == name {
                return Some(node);
            }
            current = node.read().get_next_template();
        }
        None
    }

    pub fn new_template(&mut self, name: AsciiString) -> Arc<RwLock<Anim2DTemplate>> {
        let template = Arc::new(RwLock::new(Anim2DTemplate::new(name)));
        template
            .write()
            .set_next_template(self.template_head.clone());
        self.template_head = Some(template.clone());
        template
    }

    pub fn clear(&mut self) {
        self.template_head = None;
    }
}

static ANIM2D_COLLECTION: OnceCell<Arc<RwLock<Anim2DCollection>>> = OnceCell::new();

pub fn ensure_anim2d_collection() -> Arc<RwLock<Anim2DCollection>> {
    ANIM2D_COLLECTION
        .get_or_init(|| Arc::new(RwLock::new(Anim2DCollection::new())))
        .clone()
}

pub fn initialize_anim2d_collection() {
    let collection = ensure_anim2d_collection();
    collection.write().clear();
}

pub fn get_anim2d_collection() -> Option<Arc<RwLock<Anim2DCollection>>> {
    ANIM2D_COLLECTION.get().cloned()
}

pub fn parse_anim2d_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name_token = tokens.get(1).ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*name_token);

    let collection = ensure_anim2d_collection();
    let mut collection = collection.write();

    if collection.find_template(&name).is_some() {
        return Err(INIError::InvalidData);
    }

    let template = collection.new_template(name);
    let mut template_guard = template.write();
    ini.init_from_ini_with_fields(&mut *template_guard, Anim2DTemplate::get_field_parse())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anim2d_template_creation() {
        let template = Anim2DTemplate::new(AsciiString::from("TestAnimation"));
        assert_eq!(template.get_name().as_str(), "TestAnimation");
        assert_eq!(template.get_num_frames(), 0);
    }

    #[test]
    fn test_collection_linking() {
        let mut collection = Anim2DCollection::new();
        let first = collection.new_template(AsciiString::from("First"));
        let second = collection.new_template(AsciiString::from("Second"));

        let head = collection.get_template_head().unwrap();
        assert_eq!(head.read().get_name().as_str(), "Second");
        let next = collection.get_next_template(&head).unwrap();
        assert_eq!(next.read().get_name().as_str(), "First");

        let found = collection
            .find_template(&AsciiString::from("First"))
            .unwrap();
        assert_eq!(found.read().get_name().as_str(), "First");
    }
}
