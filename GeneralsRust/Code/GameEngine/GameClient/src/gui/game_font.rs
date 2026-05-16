// Port of GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameFont.cpp
// and GeneralsMD/Code/GameEngine/Include/GameClient/GameFont.h
//
// C++ had:
//   class GameFont  — lightweight font record (name, pointSize, height, bold, fontData, next)
//   class FontLibrary : SubsystemInterface — linked-list manager with getFont/firstFont/nextFont/getCount
//   TheFontLibrary global singleton
//
// Rust already has the full implementation in gui/font.rs (FontLibrary, GameFont, FontDesc,
// FontMetrics, FontData trait, DefaultFontData). This file provides C++-style compat wrappers.

use std::sync::Arc;

use super::font::{get_font_library, FontDesc, FontMetrics, GameFont};

// ---------------------------------------------------------------------------
// FontLibrary global singleton accessor  (C++: TheFontLibrary)
// ---------------------------------------------------------------------------

/// C++ parity: `TheFontLibrary` singleton accessor.
/// In C++ this was `extern FontLibrary *TheFontLibrary`.
/// The Rust implementation is already provided by `get_font_library()` in font.rs.
pub use super::font::get_font_library as the_font_library;

// ---------------------------------------------------------------------------
// FontLibrary lifecycle — mirrors C++ FontLibrary::init / reset / update
// ---------------------------------------------------------------------------

/// C++ parity: `FontLibrary::init()`.
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    get_font_library().init_mut()
}

/// C++ parity: `FontLibrary::reset()` — deletes all cached fonts.
pub fn reset() -> Result<(), Box<dyn std::error::Error>> {
    get_font_library().reset_mut()
}

/// C++ parity: `FontLibrary::update()` — periodic maintenance.
/// In C++ this was a no-op. In Rust we clean up dead weak references.
pub fn update() -> Result<(), Box<dyn std::error::Error>> {
    get_font_library().update_mut()
}

// ---------------------------------------------------------------------------
// FontLibrary linked-list traversal — mirrors C++ FontLibrary traversal
// ---------------------------------------------------------------------------
// C++ stored fonts in an intrusive linked list (GameFont::next).
// Rust uses an insertion-ordered Vec + HashMap cache, but the traversal API
// is preserved for parity.

/// C++ parity: `FontLibrary::getFont(name, pointSize, bold)`.
/// Searches the library for an existing match, loading a new font if needed.
pub fn get_font(name: &str, point_size: i32, bold: bool) -> Option<Arc<GameFont>> {
    get_font_library()
        .get_font_by_name(name, point_size, bold)
        .ok()
}

/// C++ parity: `FontLibrary::firstFont()` — returns the first loaded font.
pub fn first_font() -> Option<Arc<GameFont>> {
    get_font_library().first_font()
}

/// C++ parity: `FontLibrary::nextFont(font)` — returns the font after `desc`.
/// In C++ this followed the `next` pointer on the linked list.
/// In Rust we look up the next entry in insertion order.
pub fn next_font(desc: &FontDesc) -> Option<Arc<GameFont>> {
    get_font_library().next_font(desc)
}

/// C++ parity: `FontLibrary::getCount()` — number of unique fonts loaded.
pub fn get_count() -> usize {
    get_font_library().get_count()
}

// ---------------------------------------------------------------------------
// FontLibrary management — mirrors C++ linkFont / unlinkFont / deleteAllFonts
// ---------------------------------------------------------------------------

/// C++ parity: `FontLibrary::deleteAllFonts()`.
/// Removes all loaded fonts from the library cache.
pub fn delete_all_fonts() {
    get_font_library().clear_cache();
}

/// C++ parity: `FontLibrary::linkFont(font)`.
/// In C++ this inserted at the head of a singly-linked list and incremented m_count.
/// In Rust the cache manages insertion automatically via `get_font()`.
/// This is exposed for parity but is a no-op (fonts are linked on creation).
pub fn link_font(_font: &Arc<GameFont>) {
    // Rust font library manages linking automatically during get_font().
    // The C++ code did: font->next = m_fontList; m_fontList = font; m_count++;
    // In Rust this is handled by FontLibrary::get_font() inserting into the cache.
}

/// C++ parity: `FontLibrary::unlinkFont(font)`.
/// In C++ this removed a font from the linked list and decremented m_count.
/// In Rust we use weak-reference cleanup — dead Arcs are pruned during cleanup_cache().
pub fn unlink_font(desc: &FontDesc) {
    // Rust doesn't expose individual removal. Mark for cleanup.
    // The C++ code walked the linked list to find and splice out the font.
    get_font_library().cleanup_cache();
}

// ---------------------------------------------------------------------------
// GameFont property accessors — mirrors C++ GameFont public fields
// ---------------------------------------------------------------------------
// C++ GameFont fields:
//   AsciiString nameString;
//   Int          pointSize;
//   Int          height;
//   void*        fontData;  (device-specific)
//   Bool         bold;
//   GameFont*    next;      (library list linkage)

/// C++ parity: `GameFont::nameString` accessor.
#[inline]
pub fn font_name(font: &GameFont) -> &str {
    &font.desc.name
}

/// C++ parity: `GameFont::pointSize` accessor.
#[inline]
pub fn font_point_size(font: &GameFont) -> i32 {
    font.desc.size
}

/// C++ parity: `GameFont::height` accessor — pixel height of the font.
#[inline]
pub fn font_height(font: &GameFont) -> i32 {
    font.height
}

/// C++ parity: `GameFont::bold` accessor.
#[inline]
pub fn font_is_bold(font: &GameFont) -> bool {
    font.desc.bold
}

/// C++ parity: `GameFont::fontData` accessor — returns font metrics.
/// In C++ fontData was a void* to device-specific data (e.g. Win32 HFONT).
/// In Rust we return the FontMetrics which contains the equivalent info
/// (height, ascent, descent, averageCharWidth, maxCharWidth).
#[inline]
pub fn font_metrics(font: &GameFont) -> FontMetrics {
    font.get_metrics()
}

// ---------------------------------------------------------------------------
// Text measurement — mirrors C++ Win32 text measurement functions
// ---------------------------------------------------------------------------
// C++ used GetTextExtentPoint32 / GDI metrics for character and text width.
// The Rust FontData trait provides measure_text() which is the equivalent.

/// C++ parity: character width — width of a single character in this font.
/// In C++ this used GetCharWidth32 or computed from TEXTMETRIC.
/// In Rust we measure a single-char string.
pub fn get_char_width(font: &GameFont, ch: char) -> i32 {
    let s = ch.to_string();
    font.measure_text(&s)
}

/// C++ parity: text width — total pixel width of a string rendered in this font.
/// In C++ this was GetTextExtentPoint32.
#[inline]
pub fn get_text_width(font: &GameFont, text: &str) -> i32 {
    font.measure_text(text)
}

/// C++ parity: line height — the vertical spacing between lines of text.
#[inline]
pub fn get_line_height(font: &GameFont) -> i32 {
    font.get_line_height()
}

/// C++ parity: average character width from font metrics.
#[inline]
pub fn get_average_char_width(font: &GameFont) -> i32 {
    font.get_metrics().average_width
}

/// C++ parity: maximum character width from font metrics.
#[inline]
pub fn get_max_char_width(font: &GameFont) -> i32 {
    font.get_metrics().max_width
}

/// C++ parity: font ascent — distance from baseline to top of tallest character.
#[inline]
pub fn get_ascent(font: &GameFont) -> i32 {
    font.get_metrics().ascent
}

/// C++ parity: font descent — distance from baseline to bottom of lowest descender.
#[inline]
pub fn get_descent(font: &GameFont) -> i32 {
    font.get_metrics().descent
}

/// C++ parity: check if the font supports a given character.
#[inline]
pub fn supports_char(font: &GameFont, ch: char) -> bool {
    font.supports_char(ch)
}

// ---------------------------------------------------------------------------
// FontDesc helpers — C++ parity for constructing font lookup keys
// ---------------------------------------------------------------------------

/// Create a FontDesc (C++ parity for constructing a font lookup).
#[inline]
pub fn make_font_desc(name: &str, point_size: i32, bold: bool) -> FontDesc {
    FontDesc::new(name, point_size, bold)
}

/// Get the FontDesc from a loaded GameFont.
#[inline]
pub fn font_desc(font: &GameFont) -> &FontDesc {
    &font.desc
}
