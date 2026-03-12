//! Script editing system for World Builder

use crate::map::{Map, Trigger};
use anyhow::Result;
use std::collections::HashMap;

/// Script editor for map logic and triggers
pub struct ScriptEditor {
    open_scripts: HashMap<String, ScriptBuffer>,
    active_script: Option<String>,
    syntax_highlighter: SyntaxHighlighter,
    script_validator: ScriptValidator,
    dirty: bool,
}

impl ScriptEditor {
    pub fn new() -> Self {
        Self {
            open_scripts: HashMap::new(),
            active_script: None,
            syntax_highlighter: SyntaxHighlighter::new(),
            script_validator: ScriptValidator::new(),
            dirty: false,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.syntax_highlighter.load_syntax_definitions()?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        // Collect scripts that need validation
        let scripts_to_validate: Vec<String> = self
            .open_scripts
            .iter()
            .filter(|(_, buffer)| buffer.needs_validation)
            .map(|(name, _)| name.clone())
            .collect();

        // Validate each script
        for script_name in scripts_to_validate {
            if let Some(buffer) = self.open_scripts.get_mut(&script_name) {
                buffer.errors.clear();
                let errors = self.script_validator.validate(&buffer.content)?;
                buffer.errors = errors;
                buffer.needs_validation = false;
            }
        }

        Ok(())
    }

    /// Open a script for editing
    pub fn open_script(&mut self, name: String, content: String) {
        let buffer = ScriptBuffer {
            name: name.clone(),
            content,
            cursor_position: 0,
            selection: None,
            errors: Vec::new(),
            needs_validation: true,
            modified: false,
        };

        self.open_scripts.insert(name.clone(), buffer);
        self.active_script = Some(name);
        self.dirty = true;
    }

    /// Close a script
    pub fn close_script(&mut self, name: &str) -> bool {
        if let Some(buffer) = self.open_scripts.get(name) {
            if buffer.modified {
                // Should prompt user to save
                return false;
            }
        }

        self.open_scripts.remove(name);

        if self.active_script.as_ref() == Some(&name.to_string()) {
            self.active_script = self.open_scripts.keys().next().cloned();
        }

        true
    }

    /// Create a new script
    pub fn new_script(&mut self, name: String) {
        let template = r#"-- New Script
-- This script will be executed when triggered

function OnTrigger()
    -- Add your code here
    print("Script triggered: " .. GetScriptName())
end
"#;

        self.open_script(name, template.to_string());
    }

    /// Save script changes to map
    pub fn save_to_map(&self, map: &mut Map) -> Result<()> {
        for (name, buffer) in &self.open_scripts {
            map.set_script(name.clone(), buffer.content.clone());
        }
        Ok(())
    }

    /// Load scripts from map
    pub fn load_scripts(&mut self, map: &Map) -> Result<()> {
        self.open_scripts.clear();

        for script_name in map.script_names() {
            if let Some(content) = map.get_script(script_name) {
                let buffer = ScriptBuffer {
                    name: script_name.clone(),
                    content: content.to_string(),
                    cursor_position: 0,
                    selection: None,
                    errors: Vec::new(),
                    needs_validation: true,
                    modified: false,
                };

                self.open_scripts.insert(script_name.clone(), buffer);
            }
        }

        self.active_script = self.open_scripts.keys().next().cloned();
        self.dirty = false;
        Ok(())
    }

    /// Clear all scripts
    pub fn clear(&mut self) {
        self.open_scripts.clear();
        self.active_script = None;
        self.dirty = false;
    }

    /// Get the active script buffer
    pub fn active_buffer(&self) -> Option<&ScriptBuffer> {
        self.active_script
            .as_ref()
            .and_then(|name| self.open_scripts.get(name))
    }

    /// Get mutable active script buffer
    pub fn active_buffer_mut(&mut self) -> Option<&mut ScriptBuffer> {
        self.active_script
            .clone()
            .and_then(|name| self.open_scripts.get_mut(&name))
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty || self.open_scripts.values().any(|buffer| buffer.modified)
    }

    /// Get list of open scripts
    pub fn open_script_names(&self) -> Vec<&String> {
        self.open_scripts.keys().collect()
    }

    /// Set active script
    pub fn set_active_script(&mut self, name: &str) {
        if self.open_scripts.contains_key(name) {
            self.active_script = Some(name.to_string());
        }
    }
}

/// Script buffer for editing
#[derive(Debug, Clone)]
pub struct ScriptBuffer {
    pub name: String,
    pub content: String,
    pub cursor_position: usize,
    pub selection: Option<(usize, usize)>,
    pub errors: Vec<ScriptError>,
    pub needs_validation: bool,
    pub modified: bool,
}

impl ScriptBuffer {
    /// Insert text at cursor position
    pub fn insert_text(&mut self, text: &str) {
        self.content.insert_str(self.cursor_position, text);
        self.cursor_position += text.len();
        self.modified = true;
        self.needs_validation = true;
    }

    /// Delete selected text or character at cursor
    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection {
            self.content.drain(start..end);
            self.cursor_position = start;
            self.selection = None;
        } else if self.cursor_position < self.content.len() {
            self.content.remove(self.cursor_position);
        }

        self.modified = true;
        self.needs_validation = true;
    }

    /// Move cursor to position
    pub fn set_cursor(&mut self, position: usize) {
        self.cursor_position = position.min(self.content.len());
        self.selection = None;
    }

    /// Get line and column from position
    pub fn get_line_col(&self, position: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for (i, ch) in self.content.chars().enumerate() {
            if i >= position {
                break;
            }

            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    /// Get current line
    pub fn current_line(&self) -> String {
        let (line_num, _) = self.get_line_col(self.cursor_position);

        self.content
            .lines()
            .nth(line_num - 1)
            .unwrap_or("")
            .to_string()
    }
}

/// Syntax highlighter for scripts
pub struct SyntaxHighlighter {
    keywords: Vec<String>,
    functions: Vec<String>,
    operators: Vec<String>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            keywords: Vec::new(),
            functions: Vec::new(),
            operators: Vec::new(),
        }
    }

    pub fn load_syntax_definitions(&mut self) -> Result<()> {
        // Lua keywords
        self.keywords = vec![
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in",
            "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        // Built-in functions
        self.functions = vec![
            "print",
            "type",
            "tostring",
            "tonumber",
            "pairs",
            "ipairs",
            "next",
            "getmetatable",
            "setmetatable",
            "rawget",
            "rawset",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        // Operators
        self.operators = vec![
            "+", "-", "*", "/", "%", "^", "#", "==", "~=", "<=", ">=", "<", ">", "=", "(", ")",
            "{", "}", "[", "]", ";", ":", ",", ".", "..", "...",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Ok(())
    }

    /// Highlight syntax in text
    pub fn highlight(&self, text: &str) -> Vec<SyntaxToken> {
        let mut tokens = Vec::new();

        // Simple tokenization (would be more sophisticated in real implementation)
        for word in text.split_whitespace() {
            let token_type = if self.keywords.contains(&word.to_string()) {
                TokenType::Keyword
            } else if self.functions.contains(&word.to_string()) {
                TokenType::Function
            } else if word.starts_with('"') && word.ends_with('"') {
                TokenType::String
            } else if word.parse::<f64>().is_ok() {
                TokenType::Number
            } else if word.starts_with("--") {
                TokenType::Comment
            } else {
                TokenType::Identifier
            };

            tokens.push(SyntaxToken {
                text: word.to_string(),
                token_type,
            });
        }

        tokens
    }
}

/// Script validator for error checking
pub struct ScriptValidator;

impl ScriptValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate script for syntax errors
    pub fn validate(&self, content: &str) -> Result<Vec<ScriptError>> {
        let mut errors = Vec::new();

        // Simple validation (would use actual Lua parser in real implementation)
        let mut brace_count = 0;
        let mut paren_count = 0;

        for (line_num, line) in content.lines().enumerate() {
            // Check for unmatched braces and parentheses
            for ch in line.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    '(' => paren_count += 1,
                    ')' => paren_count -= 1,
                    _ => {}
                }

                if brace_count < 0 {
                    errors.push(ScriptError {
                        line: line_num + 1,
                        column: 1,
                        message: "Unmatched closing brace".to_string(),
                        error_type: ErrorType::Syntax,
                    });
                    brace_count = 0;
                }

                if paren_count < 0 {
                    errors.push(ScriptError {
                        line: line_num + 1,
                        column: 1,
                        message: "Unmatched closing parenthesis".to_string(),
                        error_type: ErrorType::Syntax,
                    });
                    paren_count = 0;
                }
            }
        }

        if brace_count > 0 {
            errors.push(ScriptError {
                line: content.lines().count(),
                column: 1,
                message: "Unmatched opening brace".to_string(),
                error_type: ErrorType::Syntax,
            });
        }

        if paren_count > 0 {
            errors.push(ScriptError {
                line: content.lines().count(),
                column: 1,
                message: "Unmatched opening parenthesis".to_string(),
                error_type: ErrorType::Syntax,
            });
        }

        Ok(errors)
    }
}

/// Syntax highlighting token
#[derive(Debug, Clone)]
pub struct SyntaxToken {
    pub text: String,
    pub token_type: TokenType,
}

/// Token types for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenType {
    Keyword,
    Function,
    String,
    Number,
    Comment,
    Identifier,
    Operator,
}

/// Script error
#[derive(Debug, Clone)]
pub struct ScriptError {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub error_type: ErrorType,
}

/// Error types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorType {
    Syntax,
    Runtime,
    Logic,
}
