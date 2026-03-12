use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use regex::Regex;
use walkdir::WalkDir;

const MASK_CONSTANTS: &[(&str, u32)] = &[
    ("MODULEINTERFACE_UPDATE", 0x0000_0001),
    ("MODULEINTERFACE_DIE", 0x0000_0002),
    ("MODULEINTERFACE_DAMAGE", 0x0000_0004),
    ("MODULEINTERFACE_CREATE", 0x0000_0008),
    ("MODULEINTERFACE_COLLIDE", 0x0000_0010),
    ("MODULEINTERFACE_BODY", 0x0000_0020),
    ("MODULEINTERFACE_CONTAIN", 0x0000_0040),
    ("MODULEINTERFACE_UPGRADE", 0x0000_0080),
    ("MODULEINTERFACE_SPECIAL_POWER", 0x0000_0100),
    ("MODULEINTERFACE_DESTROY", 0x0000_0200),
    ("MODULEINTERFACE_DRAW", 0x0000_0400),
    ("MODULEINTERFACE_CLIENT_UPDATE", 0x0000_0800),
];

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let include_dir = manifest_dir.join("../../../GeneralsMD/Code/GameEngine/Include");
    let output_path = manifest_dir.join("../../../tools/module_interface_masks.json");

    let mask_regex = Regex::new(r"static\s+Int\s+getInterfaceMask\s*\([^)]*\)\s*\{([^}]*)\}")
        .context("failed to compile mask regex")?;
    let class_regex = Regex::new(r"class\s+(\w+)").context("failed to compile class regex")?;

    let mut mask_map: HashMap<String, u32> = HashMap::new();

    for entry in WalkDir::new(&include_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("h") {
            continue;
        }

        let contents = fs::read_to_string(entry.path())?;

        for caps in mask_regex.captures_iter(&contents) {
            let body = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let mut value = 0u32;
            for &(name, bit) in MASK_CONSTANTS {
                if body.contains(name) {
                    value |= bit;
                }
            }

            let prefix = &contents[..caps.get(0).unwrap().start()];
            let class_name = class_regex
                .captures_iter(prefix)
                .last()
                .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
                .unwrap_or_else(|| {
                    entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                });

            mask_map.insert(class_name, value);
        }
    }

    let json_map: BTreeMap<_, _> = mask_map.into_iter().collect();
    let json = serde_json::to_string_pretty(&json_map)?;
    fs::create_dir_all(output_path.parent().unwrap())?;
    fs::write(&output_path, json)?;

    println!("Wrote module interface masks to {}", output_path.display());

    Ok(())
}
