//! Prepared-source ownership tests. These exercise the actual Common INI API.
//! Native file comparison and held-filesystem control are not WASM runtime proof.
use super::*;
use crate::common::thing::thing_factory::ThingFactory;

#[test]
fn inline_reader_accepts_cpp_indented_object_fields() {
    // C++ INI.cpp:1451-1528 consumes object fields from an already prepared
    // stream; leading spaces are delimiters, not a loose-file catalog header.
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    ini.with_inline_source("  BuildCost = 245\n  BuildTime = 4.5\nEnd", |ini| {
        factory
            .parse_object_definition(ini, "OwnedInlineObjectFields", "")
            .map_err(|_| INIError::InvalidData)
    })
    .expect("selected object field text must reach its real parser");
    let template = factory
        .find_template("OwnedInlineObjectFields", false)
        .expect("actual independently owned ThingFactory admitted the definition");
    assert_eq!(template.get_build_cost(), 245);
    assert_eq!(template.get_build_time(), 4.5);
}

#[test]
fn inline_reader_two_instances_keep_their_own_cursor_and_reuse() {
    let mut first = INI::new();
    let mut second = INI::new();
    first
        .with_inline_source("First = 17\nTail = 31", |ini| {
            ini.read_line()?;
            assert_eq!(ini.get_next_token()?, "First");
            assert_eq!(ini.get_next_token()?, "17");
            assert_eq!(ini.get_line_num(), 1);
            second.with_inline_source("Second = 42\nEnd", |other| {
                other.read_line()?;
                assert_eq!(other.get_buffer(), "Second = 42");
                assert_eq!(other.get_line_num(), 1);
                Ok(())
            })?;
            assert_eq!(ini.get_line_num(), 1);
            ini.read_line()?;
            assert_eq!(ini.get_buffer(), "Tail = 31");
            assert!(!ini.is_eof(), "final partial line is delivered first");
            ini.read_line()?;
            assert!(ini.is_eof());
            assert_eq!(ini.get_buffer(), "");
            Ok(())
        })
        .unwrap();
    for ini in [&mut first, &mut second] {
        assert_eq!(ini.get_filename(), "None");
        assert_eq!(ini.get_load_type(), INILoadType::Invalid);
        assert_eq!(ini.get_line_num(), 0);
        assert!(!ini.is_eof());
        ini.with_inline_source("Again = 53", |ini| {
            ini.read_line()?;
            assert_eq!(ini.get_buffer(), "Again = 53");
            assert_eq!(ini.get_line_num(), 1);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn inline_reader_nested_rejection_preserves_the_outer_source() {
    let mut ini = INI::new();
    ini.with_inline_source("Head = 11\nTail = 29", |ini| {
        ini.read_line()?;
        assert_eq!(ini.get_buffer(), "Head = 11");
        let outer_name = ini.get_filename().to_string();
        assert_eq!(
            ini.with_inline_source("Nested = 77", |_| Ok(())),
            Err(INIError::FileAlreadyOpen)
        );
        assert_eq!(ini.get_filename(), outer_name);
        assert_eq!(ini.get_load_type(), INILoadType::Overwrite);
        assert_eq!(ini.get_line_num(), 1);
        ini.read_line()?;
        assert_eq!(ini.get_buffer(), "Tail = 29");
        Ok(())
    })
    .unwrap();
}

#[test]
fn inline_reader_empty_selected_text_reaches_the_callback() {
    let mut ini = INI::new();
    let mut called = false;
    ini.with_inline_source("", |ini| {
        called = true;
        assert_eq!(ini.get_load_type(), INILoadType::Overwrite);
        ini.read_line()?;
        assert!(ini.is_eof());
        assert_eq!(ini.get_buffer(), "");
        Ok(())
    })
    .expect("an owned empty source is not a missing external file");
    assert!(called);
    assert_eq!(ini.get_filename(), "None");
}

#[test]
fn inline_reader_parser_error_cleans_up_without_changing_the_error() {
    let mut ini = INI::new();
    assert_eq!(
        ini.with_inline_source("UnknownOwnedInlineBlock437\nEnd", |ini| {
            ini.parse_current_file()
        }),
        Err(INIError::UnknownToken)
    );
    assert_eq!(ini.get_filename(), "None");
    assert_eq!(ini.get_load_type(), INILoadType::Invalid);
    assert_eq!(ini.get_line_num(), 0);
    assert!(!ini.is_eof());
    assert_eq!(ini.read_line(), Err(INIError::FileNotOpen));
    // read_line without a source increments the counter in the existing
    // lexer. The next successful operation is what resets the source state.
    ini.with_inline_source("End\n", |ini| ini.parse_current_file())
        .expect_err("End is not a registered top-level block");
    assert_eq!(ini.get_line_num(), 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn inline_reader_file_and_cursor_preserve_lines_and_cpp_crc() {
    let mut contents = String::from("Key = Value  ;comment\r\n");
    contents.push_str(&"A".repeat(INI_READ_BUFFER + 37));
    contents.push_str("\nTail\t= 7\r\nEnd");
    let temp = tempfile::Builder::new().suffix(".ini").tempfile().unwrap();
    std::fs::write(temp.path(), &contents).unwrap();
    let mut inline = INI::new();
    let mut file = INI::new();
    for ini in [&mut inline, &mut file] {
        ini.set_xfer(XferCRC::new(XferLoad::new(Cursor::new(Vec::new()), 1)));
    }
    fn read_all(ini: &mut INI) -> INIResult<Vec<String>> {
        let mut lines = Vec::new();
        while !ini.is_eof() {
            ini.read_line()?;
            if !ini.is_eof() {
                lines.push(ini.get_buffer().to_string());
            }
        }
        Ok(lines)
    }
    let inline_lines = inline.with_inline_source(&contents, read_all).unwrap();
    let file_lines = file
        .with_file_source(temp.path(), INILoadType::Overwrite, read_all)
        .unwrap();
    let mut expected = vec!["Key = Value  ".to_string()];
    expected.extend((0..8).map(|_| "A".repeat(INI_MAX_CHARS_PER_LINE)));
    expected.push("A".repeat(5));
    expected.push("Tail = 7 ".to_string());
    expected.push("End".to_string());
    assert_eq!(inline_lines, expected);
    assert_eq!(file_lines, expected);
    let inline_crc = inline.take_xfer().unwrap().get_crc();
    let file_crc = file.take_xfer().unwrap().get_crc();
    assert_eq!(inline_crc, file_crc);
    assert_eq!(
        inline_crc,
        0xE25E_B7DAu32.to_be(),
        "CPP XferCRC 74-93/116-159 golden"
    );
}

#[cfg(all(not(target_arch = "wasm32"), panic = "unwind"))]
#[test]
fn inline_reader_callback_unwind_releases_source_and_preserves_payload() {
    let mut ini = INI::new();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: INIResult<()> = ini.with_inline_source("Head = 11\nTail = 29", |ini| {
            ini.read_line()?;
            std::panic::panic_any(String::from("owned inline callback payload"));
        });
    }))
    .expect_err("callback panic must propagate");
    assert_eq!(
        panic.downcast_ref::<String>().map(String::as_str),
        Some("owned inline callback payload")
    );
    assert_eq!(ini.get_filename(), "None");
    assert_eq!(ini.get_load_type(), INILoadType::Invalid);
    assert_eq!(ini.get_line_num(), 0);
    assert!(!ini.is_eof());
    ini.with_inline_source("Reuse = 43", |ini| {
        ini.read_line()?;
        assert_eq!(ini.get_buffer(), "Reuse = 43");
        Ok(())
    })
    .expect("CPP catch/unPrep/rethrow leaves the source owner reusable");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn inline_reader_owned_fields_do_not_reenter_held_filesystem() {
    const CHILD: &str = "GENERALS_INLINE_INI_FS_GUARD_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // Real shared filesystem guard; no fake backend or replacement parser.
        // This is an isolation control, not proof of a normal caller holding it.
        let file_system = get_file_system();
        let _held = file_system.lock().unwrap();
        let mut ini = INI::new();
        ini.with_inline_source("  BuildCost = 245\nEnd", |ini| {
            ini.read_line()?;
            assert_eq!(ini.get_first_token().as_deref(), Some("BuildCost"));
            Ok(())
        })
        .expect("owned text must not acquire retail filesystem authority");
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let test_name = format!("{module}::inline_reader_owned_fields_do_not_reenter_held_filesystem");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env(CHILD, "1")
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "actual owned-source child failed: {status}"
            );
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("owned-source child exceeded unchanged 25-second deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
