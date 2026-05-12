//! INI parser for ShellMenuScheme
//!
//! Reference: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Shell/ShellMenuScheme.cpp
//! Parses [ShellMenuScheme] blocks from INI files for shell menu visual schemes.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::ini::{INIError, INILoadType, INIResult, INI};
use log::warn;

/// Integer coordinate 2D
#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Color type (RGBA packed as u32)
pub type Color = u32;

/// Make a color from RGBA components
pub fn game_make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Shell menu scheme line definition
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeLine {
    pub start_pos: ICoord2D,
    pub end_pos: ICoord2D,
    pub width: i32,
    pub color: Color,
}

impl Default for ShellMenuSchemeLine {
    fn default() -> Self {
        Self {
            start_pos: ICoord2D::default(),
            end_pos: ICoord2D::default(),
            width: 1,
            color: game_make_color(255, 255, 255, 255),
        }
    }
}

/// Shell menu scheme image definition
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeImage {
    pub name: String,
    pub position: ICoord2D,
    pub size: ICoord2D,
    /// Image pointer - would be resolved from MappedImageCollection
    pub image_name: String,
}

impl Default for ShellMenuSchemeImage {
    fn default() -> Self {
        Self {
            name: String::new(),
            position: ICoord2D::default(),
            size: ICoord2D::default(),
            image_name: String::new(),
        }
    }
}

/// A shell menu scheme containing images and lines
#[derive(Debug, Clone)]
pub struct ShellMenuScheme {
    pub name: String,
    pub image_list: Vec<ShellMenuSchemeImage>,
    pub line_list: Vec<ShellMenuSchemeLine>,
}

impl ShellMenuScheme {
    pub fn new(name: String) -> Self {
        Self {
            name: name.trim().to_lowercase(),
            image_list: Vec::new(),
            line_list: Vec::new(),
        }
    }

    /// Add a line to the scheme
    pub fn add_line(&mut self, line: ShellMenuSchemeLine) {
        self.line_list.push(line);
    }

    /// Add an image to the scheme
    pub fn add_image(&mut self, image: ShellMenuSchemeImage) {
        self.image_list.push(image);
    }
}

/// Shell menu scheme manager
#[derive(Debug, Default)]
pub struct ShellMenuSchemeManager {
    schemes: HashMap<String, ShellMenuScheme>,
    scheme_order: Vec<String>,
    current_scheme: Option<String>,
}

impl ShellMenuSchemeManager {
    pub fn new() -> Self {
        Self {
            schemes: HashMap::new(),
            scheme_order: Vec::new(),
            current_scheme: None,
        }
    }

    /// Create or get a new shell menu scheme by name
    /// If the scheme already exists, it's replaced
    pub fn new_shell_menu_scheme(&mut self, name: String) -> &mut ShellMenuScheme {
        let normalized_name = name.trim().to_lowercase();
        self.schemes.remove(&normalized_name);
        self.scheme_order
            .retain(|existing| existing != &normalized_name);

        let scheme = ShellMenuScheme::new(normalized_name.clone());
        self.schemes.insert(normalized_name.clone(), scheme);
        self.scheme_order.push(normalized_name.clone());

        self.schemes.get_mut(&normalized_name).unwrap()
    }

    /// Set the current scheme by name
    pub fn set_shell_menu_scheme(&mut self, name: &str) {
        let normalized_name = name.trim().to_lowercase();
        if normalized_name.is_empty() {
            self.current_scheme = None;
            return;
        }

        if self.schemes.contains_key(&normalized_name) {
            self.current_scheme = Some(normalized_name);
        }
    }

    /// Get the current scheme
    pub fn get_current_scheme(&self) -> Option<&ShellMenuScheme> {
        self.current_scheme
            .as_ref()
            .and_then(|name| self.schemes.get(name))
    }

    /// Get a scheme by name
    pub fn get_scheme(&self, name: &str) -> Option<&ShellMenuScheme> {
        self.schemes.get(&name.trim().to_lowercase())
    }

    /// Clear all schemes.
    pub fn clear(&mut self) {
        self.schemes.clear();
        self.scheme_order.clear();
        self.current_scheme = None;
    }
}

/// Global shell menu scheme manager singleton
static SHELL_MENU_SCHEME_MANAGER: OnceLock<RwLock<ShellMenuSchemeManager>> = OnceLock::new();

/// Get the shell menu scheme manager
pub fn get_shell_menu_scheme_manager() -> &'static RwLock<ShellMenuSchemeManager> {
    SHELL_MENU_SCHEME_MANAGER.get_or_init(|| RwLock::new(ShellMenuSchemeManager::new()))
}

/// Initialize the shell menu scheme manager
pub fn init_shell_menu_scheme_manager() {
    let _ = SHELL_MENU_SCHEME_MANAGER.get_or_init(|| RwLock::new(ShellMenuSchemeManager::new()));
    {
        let manager = get_shell_menu_scheme_manager();
        manager
            .write()
            .expect("shell menu scheme manager poisoned")
            .clear();
    }
    load_shell_menu_scheme_files();
}

fn load_shell_menu_scheme_files() {
    let mut ini = INI::new();
    for path in [
        "Data/INI/Default/ShellMenuScheme.ini",
        "Data/INI/ShellMenuScheme.ini",
    ] {
        if let Err(err) = ini.load(path, INILoadType::Overwrite) {
            warn!("Failed to load shell menu scheme INI '{}': {}", path, err);
        }
    }
}

/// Parse a [ShellMenuScheme] block from an INI file
///
/// This matches the C++ INI::parseShellMenuSchemeDefinition function
pub fn parse_shell_menu_scheme_definition(ini: &mut INI) -> INIResult<()> {
    // Read the scheme name
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    // Get or create the scheme in the manager
    {
        let mut manager = get_shell_menu_scheme_manager()
            .write()
            .map_err(|_| INIError::UnknownError)?;
        manager.new_shell_menu_scheme(name.clone());
    }

    // Parse the scheme contents
    parse_shell_menu_scheme_contents(ini, &name)
}

/// Parse the contents of a shell menu scheme block
fn parse_shell_menu_scheme_contents(ini: &mut INI, scheme_name: &str) -> INIResult<()> {
    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            break;
        }

        // Handle nested blocks
        if first.eq_ignore_ascii_case("ImagePart") {
            parse_image_part(ini, scheme_name)?;
        } else if first.eq_ignore_ascii_case("LinePart") {
            parse_line_part(ini, scheme_name)?;
        }
    }

    Ok(())
}

/// Parse an ImagePart sub-block
fn parse_image_part(ini: &mut INI, scheme_name: &str) -> INIResult<()> {
    let mut image = ShellMenuSchemeImage::default();

    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            break;
        }

        let value_idx = if tokens.len() > 2 && tokens[1] == "=" {
            2
        } else {
            1
        };

        if value_idx >= tokens.len() {
            continue;
        }

        match first.to_ascii_lowercase().as_str() {
            "position" => {
                let (x, y) = parse_icoord2d(&tokens[value_idx..])?;
                image.position = ICoord2D::new(x, y);
            }
            "size" => {
                let (x, y) = parse_icoord2d(&tokens[value_idx..])?;
                image.size = ICoord2D::new(x, y);
            }
            "imagename" => {
                image.image_name = tokens[value_idx].to_string();
            }
            _ => {}
        }
    }

    // Add the image to the scheme
    let mut manager = get_shell_menu_scheme_manager()
        .write()
        .map_err(|_| INIError::UnknownError)?;

    if let Some(scheme) = manager.schemes.get_mut(&scheme_name.to_lowercase()) {
        scheme.add_image(image);
    }

    Ok(())
}

/// Parse a LinePart sub-block
fn parse_line_part(ini: &mut INI, scheme_name: &str) -> INIResult<()> {
    let mut line = ShellMenuSchemeLine::default();

    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            break;
        }

        let value_idx = if tokens.len() > 2 && tokens[1] == "=" {
            2
        } else {
            1
        };

        if value_idx >= tokens.len() {
            continue;
        }

        match first.to_ascii_lowercase().as_str() {
            "startposition" => {
                let (x, y) = parse_icoord2d(&tokens[value_idx..])?;
                line.start_pos = ICoord2D::new(x, y);
            }
            "endposition" => {
                let (x, y) = parse_icoord2d(&tokens[value_idx..])?;
                line.end_pos = ICoord2D::new(x, y);
            }
            "color" => {
                line.color = parse_color_int(&tokens[value_idx..])?;
            }
            "width" => {
                line.width = INI::parse_int(tokens[value_idx])?;
            }
            _ => {}
        }
    }

    // Add the line to the scheme
    let mut manager = get_shell_menu_scheme_manager()
        .write()
        .map_err(|_| INIError::UnknownError)?;

    if let Some(scheme) = manager.schemes.get_mut(&scheme_name.to_lowercase()) {
        scheme.add_line(line);
    }

    Ok(())
}

/// Parse an integer coordinate pair (X:value Y:value format)
fn parse_icoord2d(tokens: &[&str]) -> INIResult<(i32, i32)> {
    let mut x = 0i32;
    let mut y = 0i32;

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];

        // Handle X:value or X: value format
        if let Some(rest) = token.strip_prefix("X:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            x = INI::parse_int(value_str)?;
        } else if let Some(rest) = token.strip_prefix("Y:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            y = INI::parse_int(value_str)?;
        }

        i += 1;
    }

    Ok((x, y))
}

/// Parse a color in R:val G:val B:val [A:val] format
fn parse_color_int(tokens: &[&str]) -> INIResult<Color> {
    let mut r = 255u8;
    let mut g = 255u8;
    let mut b = 255u8;
    let mut a = 255u8;

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];

        if let Some(rest) = token.strip_prefix("R:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            r = INI::parse_unsigned_int(value_str)? as u8;
        } else if let Some(rest) = token.strip_prefix("G:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            g = INI::parse_unsigned_int(value_str)? as u8;
        } else if let Some(rest) = token.strip_prefix("B:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            b = INI::parse_unsigned_int(value_str)? as u8;
        } else if let Some(rest) = token.strip_prefix("A:") {
            let value_str = if rest.is_empty() && i + 1 < tokens.len() {
                i += 1;
                tokens[i]
            } else {
                rest
            };
            a = INI::parse_unsigned_int(value_str)? as u8;
        }

        i += 1;
    }

    Ok(game_make_color(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_menu_scheme_creation() {
        let mut manager = ShellMenuSchemeManager::new();
        let scheme = manager.new_shell_menu_scheme("TestScheme".to_string());
        assert_eq!(scheme.name, "testscheme");
        assert!(scheme.image_list.is_empty());
        assert!(scheme.line_list.is_empty());
    }

    #[test]
    fn test_shell_menu_scheme_replacement_moves_to_cpp_list_tail() {
        let mut manager = ShellMenuSchemeManager::new();
        manager.new_shell_menu_scheme("First".to_string());
        manager.new_shell_menu_scheme("Second".to_string());
        manager
            .new_shell_menu_scheme("FIRST".to_string())
            .add_line(ShellMenuSchemeLine {
                start_pos: ICoord2D::new(0, 0),
                end_pos: ICoord2D::new(1, 1),
                width: 2,
                color: game_make_color(255, 255, 255, 255),
            });

        assert_eq!(manager.scheme_order, vec!["second", "first"]);
        assert_eq!(manager.schemes["first"].line_list.len(), 1);
        assert!(manager.schemes["first"].image_list.is_empty());
    }

    #[test]
    fn test_shell_menu_scheme_line() {
        let mut scheme = ShellMenuScheme::new("Test".to_string());
        let line = ShellMenuSchemeLine {
            start_pos: ICoord2D::new(0, 0),
            end_pos: ICoord2D::new(100, 100),
            width: 2,
            color: game_make_color(255, 0, 0, 255),
        };
        scheme.add_line(line);
        assert_eq!(scheme.line_list.len(), 1);
    }

    #[test]
    fn test_parse_icoord2d() {
        let tokens = vec!["X:100", "Y:200"];
        let (x, y) = parse_icoord2d(&tokens).unwrap();
        assert_eq!(x, 100);
        assert_eq!(y, 200);
    }

    #[test]
    fn test_parse_color_int() {
        let tokens = vec!["R:255", "G:128", "B:64", "A:255"];
        let color = parse_color_int(&tokens).unwrap();
        assert_eq!(color, game_make_color(255, 128, 64, 255));
    }
}
