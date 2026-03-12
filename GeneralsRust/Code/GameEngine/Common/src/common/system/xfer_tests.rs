// FILE: xfer_tests.rs /////////////////////////////////////////////////////////
// Comprehensive tests for the Xfer serialization framework
///////////////////////////////////////////////////////////////////////////////
// Tests the complete Xfer trait implementation including Save, Load, and CRC modes
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::super::geometry::{Coord3D, Matrix3D, Point2D};
    use super::super::xfer::*;
    use std::io::Cursor;

    /// Mock XferSave implementation for testing
    struct MockXferSave {
        buffer: Vec<u8>,
        identifier: String,
        options: XferOptions,
        mode: XferMode,
    }

    impl MockXferSave {
        fn new() -> Self {
            Self {
                buffer: Vec::new(),
                identifier: String::new(),
                options: XferOptions::new(),
                mode: XferMode::Save,
            }
        }

        fn get_buffer(&self) -> &[u8] {
            &self.buffer
        }
    }

    impl Xfer for MockXferSave {
        fn get_xfer_mode(&self) -> XferMode {
            self.mode
        }

        fn get_identifier(&self) -> &str {
            &self.identifier
        }

        fn set_options(&mut self, options: u32) {
            self.options.set(options);
        }

        fn clear_options(&mut self, options: u32) {
            self.options.clear(options);
        }

        fn get_options(&self) -> u32 {
            self.options.get()
        }

        fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
            self.identifier = identifier.to_string();
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_snapshot(
            &mut self,
            _snapshot: &mut super::super::snapshot::Snapshot,
        ) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
            let bytes = ascii_string_data.as_bytes();
            let len = bytes.len() as u32;
            self.buffer.extend_from_slice(&len.to_le_bytes());
            self.buffer.extend_from_slice(bytes);
            Ok(())
        }

        fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
            // For testing, treat same as ASCII
            self.xfer_ascii_string(unicode_string_data)
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> std::io::Result<()> {
            // SAFETY: Test code ensures valid pointers
            let slice = std::slice::from_raw_parts(data, data_size);
            self.buffer.extend_from_slice(slice);
            Ok(())
        }
    }

    /// Mock XferLoad implementation for testing
    struct MockXferLoad {
        cursor: Cursor<Vec<u8>>,
        identifier: String,
        options: XferOptions,
        mode: XferMode,
    }

    impl MockXferLoad {
        fn new(buffer: Vec<u8>) -> Self {
            Self {
                cursor: Cursor::new(buffer),
                identifier: String::new(),
                options: XferOptions::new(),
                mode: XferMode::Load,
            }
        }
    }

    impl Xfer for MockXferLoad {
        fn get_xfer_mode(&self) -> XferMode {
            self.mode
        }

        fn get_identifier(&self) -> &str {
            &self.identifier
        }

        fn set_options(&mut self, options: u32) {
            self.options.set(options);
        }

        fn clear_options(&mut self, options: u32) {
            self.options.clear(options);
        }

        fn get_options(&self) -> u32 {
            self.options.get()
        }

        fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
            self.identifier = identifier.to_string();
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
            use std::io::{Read, Seek, SeekFrom};
            self.cursor
                .seek(SeekFrom::Current(data_size as i64))
                .map_err(|_| XferStatus::SkipError)?;
            Ok(())
        }

        fn xfer_snapshot(
            &mut self,
            _snapshot: &mut super::super::snapshot::Snapshot,
        ) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
            use std::io::Read;

            let mut len_bytes = [0u8; 4];
            self.cursor.read_exact(&mut len_bytes)?;
            let len = u32::from_le_bytes(len_bytes) as usize;

            let mut bytes = vec![0u8; len];
            self.cursor.read_exact(&mut bytes)?;

            *ascii_string_data = String::from_utf8(bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8")
            })?;
            Ok(())
        }

        fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
            // For testing, treat same as ASCII
            self.xfer_ascii_string(unicode_string_data)
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> std::io::Result<()> {
            use std::io::Read;
            let slice = std::slice::from_raw_parts_mut(data, data_size);
            self.cursor.read_exact(slice)?;
            Ok(())
        }
    }

    // ============================================================================
    // Basic Type Tests
    // ============================================================================

    #[test]
    fn test_xfer_version_validation() {
        let mut xfer = MockXferSave::new();
        xfer.open("test").unwrap();

        // Valid version
        let mut version = 1u8;
        assert!(xfer.xfer_version(&mut version, 2).is_ok());

        // Invalid version (too high)
        let mut bad_version = 3u8;
        assert!(xfer.xfer_version(&mut bad_version, 2).is_err());
    }

    #[test]
    fn test_xfer_primitives_roundtrip() {
        // Save primitives
        let buffer = {
            let mut xfer = MockXferSave::new();
            xfer.open("test").unwrap();

            let mut byte_val = 42i8;
            let mut ubyte_val = 255u8;
            let mut bool_val = true;
            let mut int_val = -12345i32;
            let mut uint_val = 67890u32;
            let mut short_val = -1234i16;
            let mut ushort_val = 6789u16;
            let mut real_val = 3.14159f32;
            let mut int64_val = -9876543210i64;

            xfer.xfer_byte(&mut byte_val).unwrap();
            xfer.xfer_unsigned_byte(&mut ubyte_val).unwrap();
            xfer.xfer_bool(&mut bool_val).unwrap();
            xfer.xfer_int(&mut int_val).unwrap();
            xfer.xfer_unsigned_int(&mut uint_val).unwrap();
            xfer.xfer_short(&mut short_val).unwrap();
            xfer.xfer_unsigned_short(&mut ushort_val).unwrap();
            xfer.xfer_real(&mut real_val).unwrap();
            xfer.xfer_int64(&mut int64_val).unwrap();

            xfer.close().unwrap();
            xfer.get_buffer().to_vec()
        };

        // Load and verify
        let mut xfer = MockXferLoad::new(buffer);
        xfer.open("test").unwrap();

        let mut byte_val = 0i8;
        let mut ubyte_val = 0u8;
        let mut bool_val = false;
        let mut int_val = 0i32;
        let mut uint_val = 0u32;
        let mut short_val = 0i16;
        let mut ushort_val = 0u16;
        let mut real_val = 0.0f32;
        let mut int64_val = 0i64;

        xfer.xfer_byte(&mut byte_val).unwrap();
        xfer.xfer_unsigned_byte(&mut ubyte_val).unwrap();
        xfer.xfer_bool(&mut bool_val).unwrap();
        xfer.xfer_int(&mut int_val).unwrap();
        xfer.xfer_unsigned_int(&mut uint_val).unwrap();
        xfer.xfer_short(&mut short_val).unwrap();
        xfer.xfer_unsigned_short(&mut ushort_val).unwrap();
        xfer.xfer_real(&mut real_val).unwrap();
        xfer.xfer_int64(&mut int64_val).unwrap();

        assert_eq!(byte_val, 42i8);
        assert_eq!(ubyte_val, 255u8);
        assert_eq!(bool_val, true);
        assert_eq!(int_val, -12345i32);
        assert_eq!(uint_val, 67890u32);
        assert_eq!(short_val, -1234i16);
        assert_eq!(ushort_val, 6789u16);
        assert_eq!(real_val, 3.14159f32);
        assert_eq!(int64_val, -9876543210i64);

        xfer.close().unwrap();
    }

    // ============================================================================
    // Coordinate and Region Tests
    // ============================================================================

    #[test]
    fn test_xfer_coord_3d() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut coord = Coord3D::new(1.0, 2.0, 3.0);
            xfer.xfer_coord_3d(&mut coord).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut coord = Coord3D::new(0.0, 0.0, 0.0);
        xfer.xfer_coord_3d(&mut coord).unwrap();

        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);
    }

    #[test]
    fn test_xfer_icoord_3d() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut icoord = ICoord3D {
                x: 10,
                y: 20,
                z: 30,
            };
            xfer.xfer_icoord_3d(&mut icoord).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut icoord = ICoord3D { x: 0, y: 0, z: 0 };
        xfer.xfer_icoord_3d(&mut icoord).unwrap();

        assert_eq!(icoord.x, 10);
        assert_eq!(icoord.y, 20);
        assert_eq!(icoord.z, 30);
    }

    #[test]
    fn test_xfer_region_3d() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut region = Region3D {
                lo: Coord3D::new(0.0, 0.0, 0.0),
                hi: Coord3D::new(100.0, 200.0, 300.0),
            };
            xfer.xfer_region_3d(&mut region).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut region = Region3D {
            lo: Coord3D::new(0.0, 0.0, 0.0),
            hi: Coord3D::new(0.0, 0.0, 0.0),
        };
        xfer.xfer_region_3d(&mut region).unwrap();

        assert_eq!(region.lo.x, 0.0);
        assert_eq!(region.hi.x, 100.0);
        assert_eq!(region.hi.y, 200.0);
        assert_eq!(region.hi.z, 300.0);
    }

    #[test]
    fn test_xfer_coord_2d() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut coord = Point2D::new(5.5, 10.5);
            xfer.xfer_coord_2d(&mut coord).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut coord = Point2D::new(0.0, 0.0);
        xfer.xfer_coord_2d(&mut coord).unwrap();

        assert_eq!(coord.x, 5.5);
        assert_eq!(coord.y, 10.5);
    }

    // ============================================================================
    // Color Tests
    // ============================================================================

    #[test]
    fn test_xfer_rgb_color() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut color = RGBColor {
                red: 1.0,
                green: 0.5,
                blue: 0.25,
            };
            xfer.xfer_rgb_color(&mut color).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut color = RGBColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        };
        xfer.xfer_rgb_color(&mut color).unwrap();

        assert_eq!(color.red, 1.0);
        assert_eq!(color.green, 0.5);
        assert_eq!(color.blue, 0.25);
    }

    #[test]
    fn test_xfer_rgba_color_real() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut color = RGBAColorReal {
                red: 0.8,
                green: 0.6,
                blue: 0.4,
                alpha: 0.2,
            };
            xfer.xfer_rgba_color_real(&mut color).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut color = RGBAColorReal {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        };
        xfer.xfer_rgba_color_real(&mut color).unwrap();

        assert_eq!(color.red, 0.8);
        assert_eq!(color.green, 0.6);
        assert_eq!(color.blue, 0.4);
        assert_eq!(color.alpha, 0.2);
    }

    #[test]
    fn test_xfer_rgba_color_int() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut color = RGBAColorInt {
                red: 255,
                green: 128,
                blue: 64,
                alpha: 32,
            };
            xfer.xfer_rgba_color_int(&mut color).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut color = RGBAColorInt {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
        xfer.xfer_rgba_color_int(&mut color).unwrap();

        assert_eq!(color.red, 255);
        assert_eq!(color.green, 128);
        assert_eq!(color.blue, 64);
        assert_eq!(color.alpha, 32);
    }

    // ============================================================================
    // Matrix Tests
    // ============================================================================

    #[test]
    fn test_xfer_matrix_3d() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut mtx = Matrix3D::identity();
            mtx.set_translation(10.0, 20.0, 30.0);
            xfer.xfer_matrix_3d(&mut mtx).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut mtx = Matrix3D::identity();
        xfer.xfer_matrix_3d(&mut mtx).unwrap();

        assert_eq!(mtx.get_x_translation(), 10.0);
        assert_eq!(mtx.get_y_translation(), 20.0);
        assert_eq!(mtx.get_z_translation(), 30.0);
    }

    // ============================================================================
    // Collection Tests
    // ============================================================================

    #[test]
    fn test_xfer_vec_int_save_load() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut vec = vec![1, 2, 3, 4, 5];
            xfer.xfer_vec_int(&mut vec).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut vec = Vec::new();
        xfer.xfer_vec_int(&mut vec).unwrap();

        assert_eq!(vec, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_xfer_vec_int_empty_check() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut vec = vec![1, 2, 3];
            xfer.xfer_vec_int(&mut vec).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut vec = vec![99]; // NOT EMPTY - should error
        let result = xfer.xfer_vec_int(&mut vec);

        assert!(
            result.is_err(),
            "Should error when loading into non-empty vector"
        );
    }

    // ============================================================================
    // String Tests
    // ============================================================================

    #[test]
    fn test_xfer_ascii_string() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut text = String::from("Hello, World!");
            xfer.xfer_ascii_string(&mut text).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut text = String::new();
        xfer.xfer_ascii_string(&mut text).unwrap();

        assert_eq!(text, "Hello, World!");
    }

    // ============================================================================
    // Options Tests
    // ============================================================================

    #[test]
    fn test_xfer_options() {
        let mut xfer = MockXferSave::new();

        assert_eq!(xfer.get_options(), XferOptions::NONE);

        xfer.set_options(XferOptions::NO_POST_PROCESSING);
        assert_eq!(xfer.get_options(), XferOptions::NO_POST_PROCESSING);

        xfer.clear_options(XferOptions::NO_POST_PROCESSING);
        assert_eq!(xfer.get_options(), XferOptions::NONE);
    }

    // ============================================================================
    // Marker Label Test
    // ============================================================================

    #[test]
    fn test_xfer_marker_label() {
        let mut xfer = MockXferSave::new();
        // Should be no-op
        assert!(xfer.xfer_marker_label("=== Test Marker ===").is_ok());
    }

    // ============================================================================
    // Real Range Test
    // ============================================================================

    #[test]
    fn test_xfer_real_range() {
        let buffer = {
            let mut xfer = MockXferSave::new();
            let mut range = RealRange { lo: 1.5, hi: 9.5 };
            xfer.xfer_real_range(&mut range).unwrap();
            xfer.get_buffer().to_vec()
        };

        let mut xfer = MockXferLoad::new(buffer);
        let mut range = RealRange { lo: 0.0, hi: 0.0 };
        xfer.xfer_real_range(&mut range).unwrap();

        assert_eq!(range.lo, 1.5);
        assert_eq!(range.hi, 9.5);
    }

    // ============================================================================
    // Open/Close Tests
    // ============================================================================

    #[test]
    fn test_open_close() {
        let mut xfer = MockXferSave::new();

        assert!(xfer.open("test_file").is_ok());
        assert_eq!(xfer.get_identifier(), "test_file");
        assert!(xfer.close().is_ok());
    }

    // ============================================================================
    // Mode Tests
    // ============================================================================

    #[test]
    fn test_xfer_mode() {
        let save_xfer = MockXferSave::new();
        assert_eq!(save_xfer.get_xfer_mode(), XferMode::Save);

        let load_xfer = MockXferLoad::new(vec![]);
        assert_eq!(load_xfer.get_xfer_mode(), XferMode::Load);
    }
}
