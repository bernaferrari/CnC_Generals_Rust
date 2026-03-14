use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug)]
struct LegacyUnit {
    relative_path: String,
    module_name: String,
    display_name: String,
    group: String,
    category: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct Lifecycle {
    init: bool,
    update: bool,
    shutdown: bool,
    system: bool,
    input: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir.join("../../../../../").canonicalize()?;
    let cpp_gui_root = repo_root.join("GeneralsMD/Code/GameEngine/Source/GameClient/GUI");
    let callbacks_header =
        repo_root.join("GeneralsMD/Code/GameEngine/Include/GameClient/GUICallbacks.h");

    let units = collect_units(&cpp_gui_root)?;
    let screens = parse_screens(&callbacks_header)?;

    println!("cargo:rerun-if-changed={}", callbacks_header.display());
    for unit in &units {
        println!(
            "cargo:rerun-if-changed={}",
            cpp_gui_root.join(&unit.relative_path).display()
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let destination = out_dir.join("legacy_registry.rs");
    let mut file = fs::File::create(destination)?;

    writeln!(file, "pub static LEGACY_CPP_UNITS: &[LegacyCppUnit] = &[")?;
    for unit in &units {
        writeln!(
            file,
            "    LegacyCppUnit {{ relative_path: {:?}, module_name: {:?}, display_name: {:?}, group: {:?}, category: {:?} }},",
            unit.relative_path, unit.module_name, unit.display_name, unit.group, unit.category
        )?;
    }
    writeln!(file, "];")?;

    writeln!(
        file,
        "pub static LEGACY_SCREENS: &[LegacyScreenDescriptor] = &["
    )?;
    for (name, lifecycle) in screens {
        writeln!(
            file,
            "    LegacyScreenDescriptor {{ name: {:?}, group: {:?}, lifecycle: LegacyLifecycle {{ init: {}, update: {}, shutdown: {}, system: {}, input: {} }} }},",
            name,
            screen_group(&name),
            lifecycle.init,
            lifecycle.update,
            lifecycle.shutdown,
            lifecycle.system,
            lifecycle.input
        )?;
    }
    writeln!(file, "];")?;

    Ok(())
}

fn collect_units(root: &Path) -> Result<Vec<LegacyUnit>, Box<dyn std::error::Error>> {
    let mut units = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension() == Some(OsStr::new("cpp")))
    {
        let relative = entry.path().strip_prefix(root)?;
        let relative_path = relative.to_string_lossy().replace('\\', "/");
        let stem = entry
            .path()
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown");
        units.push(LegacyUnit {
            group: source_group(relative),
            category: source_category(relative),
            display_name: stem.to_string(),
            module_name: to_snake_case(stem),
            relative_path,
        });
    }
    units.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(units)
}

fn parse_screens(path: &Path) -> Result<Vec<(String, Lifecycle)>, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let mut screens: BTreeMap<String, Lifecycle> = BTreeMap::new();
    for line in source.lines().map(str::trim) {
        if !line.starts_with("extern ") {
            continue;
        }

        let function_name = match line
            .split('(')
            .next()
            .and_then(|prefix| prefix.split_whitespace().last())
        {
            Some(function_name) => function_name,
            None => continue,
        };

        for suffix in ["Init", "Update", "Shutdown", "System", "Input"] {
            if let Some(screen_name) = function_name.strip_suffix(suffix) {
                let lifecycle = screens.entry(screen_name.to_string()).or_default();
                match suffix {
                    "Init" => lifecycle.init = true,
                    "Update" => lifecycle.update = true,
                    "Shutdown" => lifecycle.shutdown = true,
                    "System" => lifecycle.system = true,
                    "Input" => lifecycle.input = true,
                    _ => {}
                }
                break;
            }
        }
    }

    Ok(screens.into_iter().collect())
}

fn source_group(relative: &Path) -> String {
    relative
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_else(|| "Core".to_string())
}

fn source_category(relative: &Path) -> String {
    let path = relative.to_string_lossy();
    if path.contains("ControlBar/") {
        "control_bar".to_string()
    } else if path.contains("GUICallbacks/Menus/") {
        "menu_callback".to_string()
    } else if path.contains("GUICallbacks/") {
        "callback".to_string()
    } else if path.contains("Gadget/") {
        "gadget".to_string()
    } else if path.contains("Shell/") {
        "shell".to_string()
    } else if path.contains("DisconnectMenu/") || path.contains("EstablishConnectionsMenu/") {
        "screen".to_string()
    } else {
        "core".to_string()
    }
}

fn screen_group(name: &str) -> &'static str {
    if name.starts_with("WOL") {
        "WOL"
    } else if name.starts_with("Lan") {
        "LAN"
    } else if name.starts_with("Popup") {
        "Popup"
    } else if name.contains("Menu") {
        "Shell"
    } else {
        "HUD"
    }
}

fn to_snake_case(input: &str) -> String {
    let mut result = String::with_capacity(input.len() + 8);
    let mut previous_was_lowercase = false;
    for ch in input.chars() {
        if ch.is_ascii_uppercase() {
            if previous_was_lowercase {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            previous_was_lowercase = false;
        } else if ch.is_ascii_alphanumeric() {
            result.push(ch);
            previous_was_lowercase = ch.is_ascii_lowercase();
        } else {
            if !result.ends_with('_') {
                result.push('_');
            }
            previous_was_lowercase = false;
        }
    }
    result
}
