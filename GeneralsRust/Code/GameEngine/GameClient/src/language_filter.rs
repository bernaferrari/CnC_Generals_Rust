//! Language filter for chat/input text (ported from `LanguageFilter.cpp`).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use crate::system::SubsystemInterface;

const LANGUAGE_XOR_KEY: u16 = 0x5555;
const BAD_WORD_FILE_NAME: &str = "langdata.dat";

const SEPARATORS: &[char] = &[
    ' ', ';', ',', '.', '!', '?', ':', '=', '\\', '/', '>', '<', '`', '~', '(', ')', '&', '^', '%',
    '#', '\n', '\t',
];

const IGNORED_CHARS: &[char] = &['-', '_', '*', '\'', '"'];

pub struct LanguageFilter {
    word_list: HashSet<String>,
}

impl Default for LanguageFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageFilter {
    pub fn new() -> Self {
        Self {
            word_list: HashSet::new(),
        }
    }

    pub fn init(&mut self) {
        self.word_list.clear();
        let Some(path) = locate_bad_word_file() else {
            return;
        };
        let Ok(bytes) = fs::read(path) else {
            return;
        };

        let mut idx = 0;
        while let Some((word, next)) = read_word(&bytes, idx) {
            idx = next;
            if word.is_empty() {
                continue;
            }
            let decoded = decode_word(&word);
            let cleaned = un_haxor(&decoded);
            if !cleaned.is_empty() {
                self.word_list.insert(cleaned.to_lowercase());
            }
        }
    }

    pub fn reset(&mut self) {
        self.init();
    }

    pub fn update(&mut self) {}

    pub fn filter_line(&self, line: &mut String) {
        if line.is_empty() {
            return;
        }

        let mut buffer: Vec<char> = line.chars().collect();
        let tokens = tokenize(line);

        for token in tokens {
            if token.is_empty() {
                continue;
            }
            let cleaned = un_haxor(&token);
            if self.word_list.contains(&cleaned.to_lowercase()) {
                if let Some(pos) = find_subslice(&buffer, &token.chars().collect::<Vec<_>>()) {
                    for i in 0..token.chars().count() {
                        if let Some(ch) = buffer.get_mut(pos + i) {
                            *ch = '*';
                        }
                    }
                }
            }
        }

        *line = buffer.into_iter().collect();
    }
}

impl SubsystemInterface for LanguageFilter {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.init();
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update();
        Ok(())
    }
}

static LANGUAGE_FILTER: OnceLock<RwLock<LanguageFilter>> = OnceLock::new();

pub fn get_language_filter() -> std::sync::RwLockWriteGuard<'static, LanguageFilter> {
    LANGUAGE_FILTER
        .get_or_init(|| RwLock::new(LanguageFilter::new()))
        .write()
        .unwrap_or_else(|e| e.into_inner())
}

fn locate_bad_word_file() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(BAD_WORD_FILE_NAME),
        PathBuf::from("Data").join(BAD_WORD_FILE_NAME),
        PathBuf::from("windows_game")
            .join("Command & Conquer Generals Zero Hour")
            .join(BAD_WORD_FILE_NAME),
    ];
    for path in candidates {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn read_word(bytes: &[u8], start: usize) -> Option<(Vec<u16>, usize)> {
    let mut idx = start;
    let mut out = Vec::new();
    while idx + 1 < bytes.len() {
        let value = u16::from_le_bytes([bytes[idx], bytes[idx + 1]]);
        idx += 2;
        if value == 0x20 {
            break;
        }
        out.push(value);
    }
    if out.is_empty() && idx >= bytes.len() {
        return None;
    }
    Some((out, idx))
}

fn decode_word(word: &[u16]) -> String {
    let decoded: Vec<u16> = word.iter().map(|c| c ^ LANGUAGE_XOR_KEY).collect();
    String::from_utf16_lossy(&decoded)
}

fn un_haxor(word: &str) -> String {
    let mut result = String::new();
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        let lower = c.to_ascii_lowercase();
        let mapped = match lower {
            'p' => {
                if let Some('h') = chars.peek().map(|c| c.to_ascii_lowercase()) {
                    chars.next();
                    'f'
                } else {
                    c
                }
            }
            '1' => 'l',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '6' => 'b',
            '7' => 't',
            '0' => 'o',
            '@' => 'a',
            '$' => 's',
            '+' => 't',
            _ => {
                if IGNORED_CHARS.contains(&c) {
                    continue;
                }
                c
            }
        };
        result.push(mapped);
    }
    result
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        if SEPARATORS.contains(&ch) {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn find_subslice(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
