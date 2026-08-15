//! T6: game-path production code must not call `Adapter::request_device`.
//!
//! The only legal wgpu device request is `ww3d_gpu::request_device` (first
//! owner, hard-fails a second call) or `ww3d_gpu::acquire_device` (share).
//! `device_authority.rs` is the single allowed `.request_device(` call site.

use std::fs;
use std::path::{Path, PathBuf};

const ILLEGAL: &str = ".request_device(";
const AUTHORITY_FILE: &str = "device_authority.rs";

fn code_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../..")
        .canonicalize()
        .expect("resolve GeneralsRust/Code from ww3d-gpu")
}

fn skip_dir(name: &str) -> bool {
    matches!(
        name,
        "examples" | "bin" | "tests" | "GameNetwork" | "target"
    )
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if skip_dir(name) {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.contains("example") {
                continue;
            }
            out.push(path);
        }
    }
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
}

/// Drop `#[cfg(test)]` modules/items so census ignores test-only adapters.
fn production_source(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut pending_cfg_test = false;
    while let Some(ch) = chars.next() {
        if ch == '#' && chars.peek() == Some(&'[') {
            let mut attr = String::from("#");
            for next in chars.by_ref() {
                attr.push(next);
                if next == ']' {
                    break;
                }
            }
            if attr.contains("cfg(test)") {
                pending_cfg_test = true;
                skip_ws_and_item(&mut chars);
                continue;
            }
            out.push_str(&attr);
            continue;
        }
        if pending_cfg_test {
            pending_cfg_test = false;
        }
        out.push(ch);
    }
    out
}

fn skip_ws_and_item(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
    let mut depth = 0;
    let mut seen_body = false;
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                seen_body = true;
            }
            '}' => {
                depth -= 1;
                if seen_body && depth == 0 {
                    break;
                }
            }
            ';' if !seen_body => break,
            _ => {}
        }
    }
}

#[test]
fn game_path_production_does_not_call_adapter_request_device() {
    let root = code_root();
    let mut files = Vec::new();
    collect_rs_files(&root.join("Main/src"), &mut files);
    collect_rs_files(&root.join("GameEngine"), &mut files);
    collect_rs_files(&root.join("Libraries/Source/WWVegas"), &mut files);
    assert!(
        !files.is_empty(),
        "expected game-path Rust sources under {}",
        root.display()
    );

    let mut offenders = Vec::new();
    for path in &files {
        if path.file_name().and_then(|s| s.to_str()) == Some(AUTHORITY_FILE) {
            continue;
        }
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let production = production_source(&text);
        for (idx, line) in production.lines().enumerate() {
            if is_comment_line(line) {
                continue;
            }
            if line.contains(ILLEGAL) {
                offenders.push(format!("{}:{}: {}", path.display(), idx + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "game-path production must use ww3d_gpu::acquire_device / request_device, not Adapter::request_device:\n{}",
        offenders.join("\n")
    );
}
