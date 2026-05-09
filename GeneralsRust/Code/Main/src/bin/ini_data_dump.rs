use anyhow::{bail, Context, Result};
use generals_main::assets::ini_parser::{IniParser, ObjectDefinition};
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_INI_ROOT: &str = "../windows_game/extracted_big_files_v2/INIZH/Data/INI";

#[derive(Debug, Serialize)]
struct Dump {
    source_root: String,
    counts: Counts,
    objects: BTreeMap<String, ObjectDump>,
    weapons: BTreeMap<String, SectionDump>,
    armors: BTreeMap<String, SectionDump>,
}

#[derive(Debug, Serialize)]
struct Counts {
    object_files: usize,
    object_templates: usize,
    weapon_templates: usize,
    armor_templates: usize,
}

#[derive(Debug, Serialize)]
struct ObjectDump {
    parent_name: Option<String>,
    object_type: String,
    display_name: String,
    model_name: Option<String>,
    textures: BTreeMap<String, String>,
    draw_module: Option<String>,
    armor_type: Option<String>,
    hit_points: Option<u32>,
    scale: f32,
    owner: Option<String>,
    attributes: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct SectionDump {
    source_file: String,
    properties: Vec<PropertyDump>,
}

#[derive(Debug, Serialize)]
struct PropertyDump {
    key: String,
    value: String,
}

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1))?;
    let ini_root = args.ini_root;

    let object_dir = ini_root.join("Object");
    let object_files = collect_ini_files(&object_dir)
        .with_context(|| format!("collecting object INI files from {}", object_dir.display()))?;

    let mut parser = IniParser::new();
    for path in &object_files {
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let file_name = path
            .strip_prefix(&ini_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        parser
            .parse_ini_content(&content, &file_name)
            .with_context(|| format!("parsing {}", path.display()))?;
    }

    let objects = parser
        .get_all_definitions()
        .iter()
        .map(|(name, definition)| (name.clone(), ObjectDump::from(definition)))
        .collect::<BTreeMap<_, _>>();

    let weapons = parse_named_sections(&ini_root.join("Weapon.ini"), "Weapon", &ini_root)?;
    let armors = parse_named_sections(&ini_root.join("Armor.ini"), "Armor", &ini_root)?;

    if objects.len() < args.min_objects {
        bail!(
            "object template coverage too low: {} < {}",
            objects.len(),
            args.min_objects
        );
    }
    if weapons.len() < args.min_weapons {
        bail!(
            "weapon template coverage too low: {} < {}",
            weapons.len(),
            args.min_weapons
        );
    }
    if armors.len() < args.min_armors {
        bail!(
            "armor template coverage too low: {} < {}",
            armors.len(),
            args.min_armors
        );
    }

    let dump = Dump {
        source_root: ini_root.to_string_lossy().to_string(),
        counts: Counts {
            object_files: object_files.len(),
            object_templates: objects.len(),
            weapon_templates: weapons.len(),
            armor_templates: armors.len(),
        },
        objects,
        weapons,
        armors,
    };

    let json = serde_json::to_string_pretty(&dump)?;
    if let Some(output) = args.output {
        fs::write(&output, json).with_context(|| format!("writing {}", output.display()))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

struct Args {
    ini_root: PathBuf,
    output: Option<PathBuf>,
    min_objects: usize,
    min_weapons: usize,
    min_armors: usize,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut ini_root = PathBuf::from(DEFAULT_INI_ROOT);
        let mut output = None;
        let mut min_objects = 0usize;
        let mut min_weapons = 0usize;
        let mut min_armors = 0usize;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--ini-root" => {
                    ini_root = PathBuf::from(value_for(&arg, args.next())?);
                }
                "--output" => {
                    output = Some(PathBuf::from(value_for(&arg, args.next())?));
                }
                "--min-objects" => {
                    min_objects = value_for(&arg, args.next())?
                        .parse()
                        .context("parsing --min-objects")?;
                }
                "--min-weapons" => {
                    min_weapons = value_for(&arg, args.next())?
                        .parse()
                        .context("parsing --min-weapons")?;
                }
                "--min-armors" => {
                    min_armors = value_for(&arg, args.next())?
                        .parse()
                        .context("parsing --min-armors")?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument: {arg}"),
            }
        }

        Ok(Self {
            ini_root,
            output,
            min_objects,
            min_weapons,
            min_armors,
        })
    }
}

fn value_for(flag: &str, value: Option<String>) -> Result<String> {
    value.with_context(|| format!("missing value for {flag}"))
}

fn print_usage() {
    eprintln!(
        "Usage: ini_data_dump [--ini-root PATH] [--output PATH] [--min-objects N] [--min-weapons N] [--min-armors N]"
    );
}

impl From<&ObjectDefinition> for ObjectDump {
    fn from(definition: &ObjectDefinition) -> Self {
        Self {
            parent_name: definition.parent_name.clone(),
            object_type: definition.object_type.clone(),
            display_name: definition.display_name.clone(),
            model_name: definition.model_name.clone(),
            textures: definition
                .textures
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            draw_module: definition.draw_module.clone(),
            armor_type: definition.armor_type.clone(),
            hit_points: definition.hit_points,
            scale: definition.scale,
            owner: definition.owner.clone(),
            attributes: definition
                .attributes
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        }
    }
}

fn collect_ini_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_ini_files_recursive(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ini_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_ini_files_recursive(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ini"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_named_sections(
    path: &Path,
    expected_type: &str,
    ini_root: &Path,
) -> Result<BTreeMap<String, SectionDump>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let source_file = path
        .strip_prefix(ini_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut sections = BTreeMap::new();
    let mut current: Option<(String, Vec<PropertyDump>)> = None;

    for raw_line in content.lines() {
        let line = strip_inline_comment(raw_line.trim()).trim();
        if line.is_empty() {
            continue;
        }

        if let Some((section_type, name)) = parse_section_header(line) {
            if let Some((current_name, properties)) = current.take() {
                sections.insert(
                    current_name,
                    SectionDump {
                        source_file: source_file.clone(),
                        properties,
                    },
                );
            }

            if section_type.eq_ignore_ascii_case(expected_type) {
                current = Some((name.to_string(), Vec::new()));
            }
            continue;
        }

        if line.eq_ignore_ascii_case("End") {
            if let Some((current_name, properties)) = current.take() {
                sections.insert(
                    current_name,
                    SectionDump {
                        source_file: source_file.clone(),
                        properties,
                    },
                );
            }
            continue;
        }

        if let Some((_, properties)) = current.as_mut() {
            if let Some((key, value)) = line.split_once('=') {
                properties.push(PropertyDump {
                    key: key.trim().to_string(),
                    value: unquote(value.trim()).to_string(),
                });
            }
        }
    }

    if let Some((current_name, properties)) = current.take() {
        sections.insert(
            current_name,
            SectionDump {
                source_file,
                properties,
            },
        );
    }

    Ok(sections)
}

fn parse_section_header(line: &str) -> Option<(&str, &str)> {
    if line.contains('=') {
        return None;
    }

    let mut parts = line.split_whitespace();
    let section_type = parts.next()?;
    match section_type {
        "Weapon" | "Armor" => parts.next().map(|name| (section_type, name)),
        _ => None,
    }
}

fn strip_inline_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b';' | b'#' if !in_single && !in_double => return value[..i].trim_end(),
            b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return value[..i].trim_end();
            }
            _ => {}
        }
        i += 1;
    }

    value
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}
