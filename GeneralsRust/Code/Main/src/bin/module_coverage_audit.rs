use anyhow::{anyhow, Context, Result};
use generals_main::assets::archive::ArchiveFileSystem;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

    fn source_marker(self) -> &'static str {
        match self {
            Self::Behavior => "ModuleType::Behavior",
            Self::Draw => "ModuleType::Draw",
            Self::ClientUpdate => "ModuleType::ClientUpdate",
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

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| anyhow!("cannot resolve repo root from {}", manifest_dir.display()))?;

    let registered = collect_registered_modules(repo_root)?;
    let used = collect_retail_module_uses().await?;

    let missing = missing_modules(&used, &registered);
    println!(
        "retail module audit: {} unique used, {} registered, {} missing",
        used.iter()
            .map(|module| (module.kind, module.name.as_str()))
            .collect::<BTreeSet<_>>()
            .len(),
        registered.values().map(BTreeSet::len).sum::<usize>(),
        missing.len()
    );

    if missing.is_empty() {
        return Ok(());
    }

    for module in &missing {
        println!(
            "missing {:?} module {:<40} first seen in {}",
            module.kind, module.name, module.source
        );
    }

    Err(anyhow!(
        "{} retail module classes are not registered in Rust",
        missing.len()
    ))
}

fn collect_registered_modules(repo_root: &Path) -> Result<BTreeMap<ModuleKind, BTreeSet<String>>> {
    let mut registered = BTreeMap::<ModuleKind, BTreeSet<String>>::new();
    for kind in [
        ModuleKind::Behavior,
        ModuleKind::Draw,
        ModuleKind::ClientUpdate,
    ] {
        registered.entry(kind).or_default();
    }

    let files = [
        repo_root.join("GeneralsRust/Code/GameEngine/GameLogic/src/contain_module_overrides.rs"),
        repo_root.join("GeneralsRust/Code/GameEngine/Common/src/common/thing/module_factory.rs"),
    ];

    for path in files {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let include_builtin_behavior_tuples = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "module_factory.rs");
        collect_registered_from_source(&content, include_builtin_behavior_tuples, &mut registered);
    }

    Ok(registered)
}

fn collect_registered_from_source(
    content: &str,
    include_builtin_behavior_tuples: bool,
    registered: &mut BTreeMap<ModuleKind, BTreeSet<String>>,
) {
    let mut pending_name: Option<String> = None;
    for line in content.lines() {
        for literal in string_literals(line) {
            if looks_like_module_name(&literal) {
                pending_name = Some(literal);
            }
        }

        for kind in [
            ModuleKind::Behavior,
            ModuleKind::Draw,
            ModuleKind::ClientUpdate,
        ] {
            if line.contains(kind.source_marker()) {
                if let Some(name) = pending_name.take() {
                    registered.entry(kind).or_default().insert(name);
                }
                break;
            }
        }

        if line.contains("ModuleInterfaceType::") {
            if let Some(name) = pending_name.take() {
                registered
                    .entry(ModuleKind::Behavior)
                    .or_default()
                    .insert(name);
            }
        }

        if include_builtin_behavior_tuples && line.trim_end().ends_with("),") {
            if let Some(name) = pending_name.take() {
                registered
                    .entry(ModuleKind::Behavior)
                    .or_default()
                    .insert(name);
            }
        }
    }
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

fn missing_modules(
    used: &BTreeSet<ModuleUse>,
    registered: &BTreeMap<ModuleKind, BTreeSet<String>>,
) -> Vec<ModuleUse> {
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();

    for module in used {
        let key = (module.kind, module.name.clone());
        if !seen.insert(key) {
            continue;
        }
        if registered
            .get(&module.kind)
            .is_some_and(|names| names.contains(&module.name))
        {
            continue;
        }
        missing.push(module.clone());
    }

    missing
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

fn string_literals(line: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = line.char_indices();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut escaped = false;
        for (end, next) in chars.by_ref() {
            if escaped {
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == '"' {
                literals.push(line[start + 1..end].to_string());
                break;
            }
        }
    }
    literals
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
