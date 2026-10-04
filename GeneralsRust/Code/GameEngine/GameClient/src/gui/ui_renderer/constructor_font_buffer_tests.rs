//! The real UIRenderer constructor's CPU initialization boundary.
//! No GPU instance, fallback font, or direct test database registration.
use super::*;

const INTER: &[u8] = include_bytes!("../tests/fixtures/fonts/Inter-Regular.ttf");

fn empty_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db("en-US".to_string(), cosmic_text::fontdb::Database::new())
}

fn draw_registered(runtime: &mut FontRuntime, weight: Weight) -> RasterizedText {
    let attrs = Attrs::new()
        .family(Family::Name("Inter"))
        .weight(weight)
        .stretch(Stretch::Normal)
        .style(Style::Normal);
    runtime.raster_text(
        "iiiiiiii",
        &attrs,
        Metrics::new(16.0, 19.2),
        (512, 64),
        Wrap::None,
        TextColor::rgba(255, 255, 255, 255),
    )
}

#[test]
fn actual_constructor_empty_database_accepts_memory_face_before_first_draw() {
    let font_system = empty_font_system();
    assert_eq!(font_system.db().faces().count(), 0);
    // This exact helper is called by UIRenderer::new. OLD is intentionally
    // expected to panic here; the empty font database is the constructor bug.
    let text_buffer = initialize_font_text_buffer();
    assert_eq!(text_buffer.metrics(), Metrics::new(14.0, 16.0));
    assert_eq!(text_buffer.layout_runs().count(), 0);
    assert_eq!(font_system.db().faces().count(), 0);

    let mut runtime = FontRuntime::new(font_system, text_buffer);
    runtime.load_font("Inter", INTER).unwrap();
    let desc = super::super::font::FontDesc::new("Inter", 12, false);
    let font = runtime.registered_font(&desc).unwrap();
    assert_eq!(font.desc, desc);
    assert!(font.measure_text("iiiiiiii") > 0);
    let image = draw_registered(&mut runtime, Weight::NORMAL);
    assert!(
        !image.pixels.is_empty(),
        "actual supported registered bytes must rasterize after initialization"
    );
    assert!(runtime.text_buffer.layout_runs().next().is_some());
}

#[test]
fn actual_constructor_buffer_preserves_preloaded_face_identity_and_bold_request() {
    // Preload through the real registration boundary, not a manual DB insert.
    // The constructor helper accepts the same preloaded DB shape native
    // discovery supplies; this test does not claim native filesystem coverage.
    let mut runtime = FontRuntime::new(
        empty_font_system(),
        TextBuffer::new_empty(Metrics::new(14.0, 16.0)),
    );
    runtime.load_font("Inter", INTER).unwrap();
    let before_faces: Vec<_> = runtime
        .font_system
        .db()
        .faces()
        .map(|face| (face.id, face.index, face.weight, face.families.clone()))
        .collect();
    let buffer = initialize_font_text_buffer();
    assert_eq!(buffer.metrics(), Metrics::new(14.0, 16.0));
    assert_eq!(
        runtime
            .font_system
            .db()
            .faces()
            .map(|face| (face.id, face.index, face.weight, face.families.clone()))
            .collect::<Vec<_>>(),
        before_faces
    );
    runtime.text_buffer = buffer;
    let image = draw_registered(&mut runtime, Weight::BOLD);
    assert!(!image.pixels.is_empty());
    let mut glyphs = 0;
    for run in runtime.text_buffer.layout_runs() {
        for glyph in run.glyphs {
            glyphs += 1;
            assert_eq!(glyph.font_weight, Weight::BOLD);
            assert_eq!(glyph.font_size, 16.0);
            assert!(before_faces.iter().any(|face| face.0 == glyph.font_id));
        }
    }
    assert_eq!(glyphs, 8);
}
