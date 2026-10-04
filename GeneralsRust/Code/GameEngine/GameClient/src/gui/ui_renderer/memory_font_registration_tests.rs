//! CPU regression through the same load_font/raster_text methods as UIRenderer.
//! The OFL fixtures are owned bytes; no system enumeration or test DB insertion.
use super::*;

const INTER: &[u8] = include_bytes!("../tests/fixtures/fonts/Inter-Regular.ttf");
const FIRA_MONO: &[u8] = include_bytes!("../tests/fixtures/fonts/FiraMono-Medium.ttf");
const TEXT: &str = "iiiiiiii";
const POINT_SIZE: i32 = 12;

fn isolated_runtime() -> FontRuntime {
    let font_system = FontSystem::new_with_locale_and_db(
        "en-US".to_string(),
        cosmic_text::fontdb::Database::new(),
    );
    let text_buffer = TextBuffer::new_empty(Metrics::new(16.0, 24.0));
    FontRuntime::new(font_system, text_buffer)
}

fn measured_width(runtime: &mut FontRuntime, name: &str) -> i32 {
    let desc = super::super::font::FontDesc::new(name, POINT_SIZE, false);
    runtime
        .registered_font(&desc)
        .expect("registered render bytes must supply this owner's FontLibrary face")
        .measure_text(TEXT)
}

fn raster(runtime: &mut FontRuntime, authored_family: &str) -> RasterizedText {
    let px = super::super::font::font_pixel_size(POINT_SIZE) as f32;
    let attrs = Attrs::new()
        .family(Family::Name(authored_family))
        .weight(Weight::NORMAL)
        .stretch(Stretch::Normal)
        .style(Style::Normal);
    runtime.raster_text(
        TEXT,
        &attrs,
        Metrics::new(px, 24.0),
        (512, 64),
        Wrap::None,
        TextColor::rgba(255, 255, 255, 255),
    )
}

fn assert_registered_family(runtime: &FontRuntime, expected: &str) {
    assert!(
        runtime
            .font_system
            .db()
            .faces()
            .any(|face| { face.families.iter().any(|(name, _)| name == expected) }),
        "actual UIRenderer load_font must make its owned bytes available to the raster database: {expected}"
    );
}

fn assert_shaped_face_and_size(runtime: &FontRuntime, family: &str) {
    let mut count = 0;
    for run in runtime.text_buffer.layout_runs() {
        for glyph in run.glyphs {
            count += 1;
            let face = runtime
                .font_system
                .db()
                .face(glyph.font_id)
                .expect("shaped glyph must reference this runtime's registered face");
            assert!(face.families.iter().any(|(name, _)| name == family));
            assert_eq!(
                glyph.font_size, 16.0,
                "12 point font uses C++ 96 dpi em size"
            );
            assert_eq!(glyph.font_weight, Weight::NORMAL);
        }
    }
    assert_eq!(
        count,
        TEXT.len(),
        "ASCII fixture has one glyph per character"
    );
}

#[test]
fn actual_memory_font_registration_shapes_measures_and_rasterizes() {
    let mut runtime = isolated_runtime();
    assert_eq!(runtime.font_system.db().faces().count(), 0);
    runtime
        .load_font("Inter", INTER)
        .expect("licensed real font is supported");
    let width = measured_width(&mut runtime, "Inter");
    assert!(width > 0);
    // Check the live registration first: shaping an empty cosmic-text database
    // panics before it can rasterize, which would obscure the precise OLD defect.
    assert_registered_family(&runtime, "Inter");
    let image = raster(&mut runtime, "Inter");
    assert!(
        !image.pixels.is_empty(),
        "registered face must produce actual CPU pixels"
    );
    assert!(
        (image.logical_size.0 - width).abs() <= 1,
        "same face at same em must agree with glyph-advance measurement"
    );
    assert!(image.ink_bounds[2] >= image.ink_bounds[0]);
    assert!(
        runtime
            .swash_cache
            .image_cache
            .values()
            .any(Option::is_some)
    );
    assert_shaped_face_and_size(&runtime, "Inter");
}

#[test]
fn actual_memory_font_replacement_invalidates_faces_and_raster_caches() {
    let mut runtime = isolated_runtime();
    runtime.load_font("AuthoredUiFace", INTER).unwrap();
    assert_registered_family(&runtime, "Inter");
    let first = raster(&mut runtime, "AuthoredUiFace");
    let first_width = measured_width(&mut runtime, "AuthoredUiFace");
    assert!(!first.pixels.is_empty());
    assert!(!runtime.swash_cache.image_cache.is_empty());

    runtime.load_font("AuthoredUiFace", FIRA_MONO).unwrap();
    let second_width = measured_width(&mut runtime, "AuthoredUiFace");
    assert_ne!(
        first_width, second_width,
        "real fixtures must distinguish replacement width"
    );
    assert!(
        runtime.swash_cache.image_cache.is_empty(),
        "successful registration clears stale face raster keys"
    );
    assert!(runtime.swash_cache.outline_command_cache.is_empty());
    assert_eq!(
        runtime.text_buffer.layout_runs().count(),
        0,
        "successful registration discards old shaped glyph IDs"
    );
    assert_registered_family(&runtime, "Fira Mono");
    let second = raster(&mut runtime, "AuthoredUiFace");
    assert!(!second.pixels.is_empty());
    assert_ne!(
        first.pixels, second.pixels,
        "authored alias must resolve to the replacement bytes"
    );
    assert!((second.logical_size.0 - second_width).abs() <= 1);
    assert_shaped_face_and_size(&runtime, "Fira Mono");
}

#[test]
fn actual_memory_font_invalid_bytes_are_atomic_and_do_not_reach_other_runtime() {
    let mut first = isolated_runtime();
    let second = isolated_runtime();
    first.load_font("Inter", INTER).unwrap();
    let before_width = measured_width(&mut first, "Inter");
    let before_faces: Vec<_> = first.font_system.db().faces().map(|face| face.id).collect();
    let before_images = first.swash_cache.image_cache.len();
    assert!(first.load_font("Inter", b"not an sfnt font").is_err());
    assert_eq!(measured_width(&mut first, "Inter"), before_width);
    assert_eq!(
        first
            .font_system
            .db()
            .faces()
            .map(|face| face.id)
            .collect::<Vec<_>>(),
        before_faces
    );
    assert_eq!(first.swash_cache.image_cache.len(), before_images);
    assert_eq!(
        second.font_system.db().faces().count(),
        0,
        "registration is renderer instance owned"
    );
}

#[test]
fn matching_aliases_in_two_populated_runtimes_keep_faces_and_caches_independent() {
    let mut first = isolated_runtime();
    let mut second = isolated_runtime();
    let desc = super::super::font::FontDesc::new("SharedAlias", POINT_SIZE, false);
    first.load_font(&desc.name, INTER).unwrap();
    second.load_font(&desc.name, FIRA_MONO).unwrap();
    let first_font = first.registered_font(&desc).unwrap();
    let second_font = second.registered_font(&desc).unwrap();
    assert_eq!(first_font.desc, second_font.desc);
    assert_ne!(
        first_font.measure_text(TEXT),
        second_font.measure_text(TEXT)
    );
    let first_pixels = raster(&mut first, &desc.name).pixels;
    let second_pixels = raster(&mut second, &desc.name).pixels;
    assert_ne!(first_pixels, second_pixels);
    let second_faces: Vec<_> = second
        .font_system
        .db()
        .faces()
        .map(|face| face.id)
        .collect();
    let second_images = second.swash_cache.image_cache.len();
    assert!(second_images > 0);

    first.load_font(&desc.name, FIRA_MONO).unwrap();
    assert!(first.swash_cache.image_cache.is_empty());
    assert_eq!(second.swash_cache.image_cache.len(), second_images);
    assert_eq!(
        second
            .font_system
            .db()
            .faces()
            .map(|face| face.id)
            .collect::<Vec<_>>(),
        second_faces
    );
    assert!(Arc::ptr_eq(
        &second.registered_font(&desc).unwrap(),
        &second_font
    ));
    assert_eq!(raster(&mut second, &desc.name).pixels, second_pixels);
    assert_eq!(
        first.registered_font(&desc).unwrap().measure_text(TEXT),
        second_font.measure_text(TEXT)
    );
    drop(first);
    assert_eq!(raster(&mut second, &desc.name).pixels, second_pixels);
}

#[test]
fn actual_registered_measurement_refreshes_same_descriptor_display_layout() {
    use super::super::display_string::DisplayString;
    use super::super::font::FontDesc;

    let mut runtime = isolated_runtime();
    let desc = FontDesc::new("AuthoredUiFace", POINT_SIZE, false);
    runtime.load_font(&desc.name, INTER).unwrap();
    let first = runtime.registered_font(&desc).unwrap();
    assert_eq!(
        first.desc, desc,
        "authored alias, size and weight remain cache identity"
    );
    assert!(Arc::ptr_eq(
        &first,
        &runtime.registered_font(&desc).unwrap()
    ));
    let mut display = DisplayString::new();
    display.set_text(TEXT);
    display.refresh_registered_font(Some(Arc::clone(&first)));
    assert_eq!(display.get_size().0, first.measure_text(TEXT));
    let initial_size = display.get_size();

    runtime.load_font(&desc.name, FIRA_MONO).unwrap();
    let replacement = runtime.registered_font(&desc).unwrap();
    assert_eq!(replacement.desc, desc);
    assert!(
        !Arc::ptr_eq(&first, &replacement),
        "face replacement invalidates this owner's font cache"
    );
    assert_ne!(first.measure_text(TEXT), replacement.measure_text(TEXT));
    assert_eq!(
        first.measure_text(TEXT),
        initial_size.0,
        "existing immutable font handle remains valid"
    );
    // Same actual helper called before layout by draw_with_renderer.
    display.refresh_registered_font(Some(Arc::clone(&replacement)));
    assert!(Arc::ptr_eq(display.get_font().unwrap(), &replacement));
    assert_eq!(display.get_size().0, replacement.measure_text(TEXT));
    assert_ne!(
        display.get_size(),
        initial_size,
        "same descriptor does not suppress new face layout"
    );

    let bold_desc = FontDesc::new("AuthoredUiFace", 24, true);
    let bold = runtime.registered_font(&bold_desc).unwrap();
    assert_eq!(bold.desc, bold_desc);
    assert!(
        (bold.measure_text(TEXT) - replacement.measure_text(TEXT) * 2).abs() <= 1,
        "point24 selects32px em on the same registered face"
    );
    let second_owner = isolated_runtime();
    assert_eq!(second_owner.font_system.db().faces().count(), 0);
}
