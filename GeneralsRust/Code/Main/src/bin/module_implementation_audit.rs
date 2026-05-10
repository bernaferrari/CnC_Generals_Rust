use anyhow::{anyhow, Result};
use game_engine::common::rts::AsciiString;
use game_engine::common::thing::module::{ModuleInterfaceType, ModuleType};
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_template::{ModuleDescriptor, ModuleDescriptorSet};
use gamelogic::contain_module_overrides::ensure_module_overrides_installed;
use generals_main::assets::archive::ArchiveFileSystem;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ModuleKind {
    Behavior,
    Draw,
    ClientUpdate,
}

impl ModuleKind {
    fn from_assignment_key(key: &str) -> Option<Self> {
        match key.to_ascii_lowercase().as_str() {
            "draw" => Some(Self::Draw),
            "clientupdate" => Some(Self::ClientUpdate),
            "behavior" | "body" | "collide" | "create" | "die" | "destroy" | "physics" => {
                Some(Self::Behavior)
            }
            _ => None,
        }
    }

    fn module_type(self) -> ModuleType {
        match self {
            Self::Behavior => ModuleType::Behavior,
            Self::Draw => ModuleType::Draw,
            Self::ClientUpdate => ModuleType::ClientUpdate,
        }
    }

    fn interface_mask(self) -> ModuleInterfaceType {
        match self {
            Self::Behavior => ModuleInterfaceType::UPDATE,
            Self::Draw => ModuleInterfaceType::DRAW,
            Self::ClientUpdate => ModuleInterfaceType::CLIENT_UPDATE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModuleUse {
    kind: ModuleKind,
    name: String,
    source: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let used = collect_retail_module_uses().await?;
    let unique = unique_module_uses(&used);
    let missing = audit_runtime_factory(&unique)?;

    println!(
        "retail module implementation audit: {} unique used, {} factory checked, {} missing implementations",
        unique.len(),
        unique.len(),
        missing.len()
    );

    if missing.is_empty() {
        return Ok(());
    }

    for module in &missing {
        println!(
            "missing implementation for {:?} module {:<40} first seen in {}",
            module.kind, module.name, module.source
        );
    }

    Err(anyhow!(
        "{} retail module classes still lack runtime implementations",
        missing.len()
    ))
}

async fn collect_retail_module_uses() -> Result<BTreeSet<ModuleUse>> {
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.add_search_path("assets");
    archive_system.init().await?;

    let ini_paths: Vec<String> = archive_system
        .list_all_files()
        .into_iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.starts_with("data/ini/") && lower.ends_with(".ini")
        })
        .collect();

    let mut uses = BTreeSet::new();
    for path in ini_paths {
        let Ok(bytes) = archive_system.open_file(&path).await else {
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        collect_module_uses_from_ini(&content, &path, &mut uses);
    }

    Ok(uses)
}

fn collect_module_uses_from_ini(content: &str, source: &str, uses: &mut BTreeSet<ModuleUse>) {
    for raw_line in content.lines() {
        let line = strip_inline_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(kind) = ModuleKind::from_assignment_key(key.trim()) else {
            continue;
        };
        let Some(name) = value.split_whitespace().next() else {
            continue;
        };
        if !looks_like_module_name(name) {
            continue;
        }

        uses.insert(ModuleUse {
            kind,
            name: name.to_string(),
            source: source.to_string(),
        });
    }
}

fn unique_module_uses(used: &BTreeSet<ModuleUse>) -> Vec<ModuleUse> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for module in used {
        if seen.insert((module.kind, module.name.clone())) {
            unique.push(module.clone());
        }
    }
    unique
}

fn audit_runtime_factory(unique: &[ModuleUse]) -> Result<Vec<ModuleUse>> {
    ensure_module_overrides_installed().map_err(|err| anyhow!(err))?;

    let mut descriptor_set = ModuleDescriptorSet::default();
    for module in unique {
        descriptor_set
            .for_type_mut(module.kind.module_type())
            .push(ModuleDescriptor {
                name: AsciiString::from(module.name.as_str()),
                module_tag: AsciiString::new(),
                interface_mask: module.kind.interface_mask(),
                inheritable: false,
                overrideable_by_like_kind: false,
                copied_from_default: false,
            });
    }

    let mut factory = ModuleFactory::new();
    factory.register_descriptor_set(&descriptor_set);

    let mut missing = Vec::new();
    for kind in [
        ModuleKind::Behavior,
        ModuleKind::Draw,
        ModuleKind::ClientUpdate,
    ] {
        let stubbed = factory
            .stubbed_module_names(kind.module_type())
            .into_iter()
            .collect::<BTreeSet<_>>();
        for module in unique.iter().filter(|module| module.kind == kind) {
            if stubbed.contains(module.name.as_str()) {
                missing.push(module.clone());
            }
        }
    }

    Ok(missing)
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
            b';' | b'#' if !in_single && !in_double => return &value[..i],
            b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &value[..i];
            }
            _ => {}
        }
        i += 1;
    }
    value
}

fn looks_like_module_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "yes" | "no" | "true" | "false" | "none" | "normal"
        )
}
