// C++ ownership: TheFileSystem virtual-path resolution and map-file access helpers.

static TERRAIN_ROADS_LOAD_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

fn normalize_virtual_path(path: &Path) -> String {
    normalize_virtual_path_str(&path.to_string_lossy())
}

fn normalize_virtual_path_str(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('"').to_string()
}

fn normalize_lookup_path(path: &str) -> String {
    normalize_virtual_path_str(path)
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn push_unique_string(vec: &mut Vec<String>, candidate: String) {
    if !vec.iter().any(|existing| existing == &candidate) {
        vec.push(candidate);
    }
}

fn resolve_with_file_system(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    if let Ok(file_system) = get_file_system().try_lock() {
        if file_system.does_file_exist(&normalized) {
            return Some(PathBuf::from(&normalized));
        }
    }

    None
}

fn read_file_bytes_via_file_system(path: &Path) -> Option<Vec<u8>> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    let access = FileAccess::READ.combine(FileAccess::BINARY);
    let file_system = get_file_system();
    let mut file_system = file_system.try_lock().ok()?;
    let mut file = file_system.open_file(&normalized, access)?;
    file.read_entire_and_close().ok()
}

fn read_file_bytes_for_runtime(path: &Path) -> Option<Vec<u8>> {
    read_file_bytes_via_file_system(path).or_else(|| {
        let normalized = normalize_virtual_path(path);
        if normalized.is_empty() {
            None
        } else if Path::new(&normalized).exists() {
            fs::read(&normalized).ok()
        } else {
            None
        }
    })
}

fn read_text_via_file_system(path: &Path) -> Option<String> {
    let bytes = read_file_bytes_via_file_system(path)?;
    String::from_utf8(bytes).ok()
}

fn read_text_with_fallback(path: &Path) -> Option<String> {
    if let Some(contents) = read_text_via_file_system(path) {
        return Some(contents);
    }
    if normalize_lookup_path(path.to_string_lossy().as_ref()).is_empty() {
        return None;
    }
    if path.exists() {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

fn first_readable_map_ini_companion(dir: &Path, names: &[&str]) -> Option<(PathBuf, String)> {
    for name in names {
        let path = dir.join(name);
        if let Some(contents) = read_text_with_fallback(&path) {
            return Some((path, contents));
        }
    }
    None
}

fn path_is_accessible(path: &Path) -> bool {
    resolve_with_file_system(path).is_some() || path.exists()
}

fn resolve_path_candidate(candidate: &Path) -> Option<PathBuf> {
    if let Some(found) = resolve_with_file_system(candidate) {
        return Some(found);
    }
    if candidate.exists() {
        return Some(candidate.to_path_buf());
    }

    None
}

fn materialize_to_temporary(path: &str, bytes: &[u8]) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    bytes.hash(&mut hasher);
    let filename_hash = hasher.finish();

    let path_obj = Path::new(path);
    let base = path_obj
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset");
    let extension = path_obj
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin");

    let temp_dir = env::temp_dir().join("generals_zero_hour");
    fs::create_dir_all(&temp_dir).ok()?;

    let temp_path = temp_dir.join(format!("{}_{}.{}", base, filename_hash, extension));
    if let Ok(existing) = fs::metadata(&temp_path) {
        if existing.len() == bytes.len() as u64 {
            return Some(temp_path);
        }
    }

    fs::write(&temp_path, bytes).ok()?;
    Some(temp_path)
}

fn resolve_runtime_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    let candidate = Path::new(&normalized);
    if let Some(bytes) = read_file_bytes_via_file_system(candidate) {
        return materialize_to_temporary(&normalized, &bytes);
    }

    if candidate.exists() {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

fn resolve_runtime_ini_path(requested: &Path) -> Option<PathBuf> {
    let requested_normalized = normalize_virtual_path(requested);
    if requested_normalized.is_empty() {
        return None;
    }

    let mut candidates = Vec::new();
    push_unique_string(
        &mut candidates,
        normalize_lookup_path(&requested_normalized),
    );
    if let Some(stripped) = requested_normalized
        .strip_prefix("Data/")
        .or_else(|| requested_normalized.strip_prefix("data/"))
    {
        push_unique_string(&mut candidates, stripped.to_string());
    }

    candidates.sort();
    candidates.dedup();

    for candidate in candidates {
        let Some(candidate_path) = resolve_path_candidate(Path::new(&candidate)) else {
            continue;
        };
        if let Some(runtime_path) = resolve_runtime_path(&candidate_path) {
            return Some(runtime_path);
        }
    }

    None
}

fn ensure_terrain_roads_loaded() {
    TERRAIN_ROADS_LOAD_RESULT.get_or_init(|| {
        let result = (|| {
            let mut ini = INI::new();

            if let Some(default_path) =
                resolve_runtime_ini_path(Path::new("Data/INI/Default/Roads.ini"))
            {
                ini.load(&default_path, INILoadType::Overwrite)
                    .map_err(|err| {
                        format!("failed loading '{}': {}", default_path.display(), err)
                    })?;
            }

            if let Some(override_path) = resolve_runtime_ini_path(Path::new("Data/INI/Roads.ini")) {
                ini.load(&override_path, INILoadType::MultiFile)
                    .map_err(|err| {
                        format!("failed loading '{}': {}", override_path.display(), err)
                    })?;
            }

            Ok(())
        })();
        if let Err(err) = &result {
            // The result is cached for the process lifetime; report a failure once
            // rather than once per object placement that asks whether it is a road.
            warn!("Terrain roads registry unavailable: {}", err);
        }
        result
    });
}

fn is_terrain_road_name(name: &str) -> bool {
    ensure_terrain_roads_loaded();
    try_get_terrain_roads().is_some_and(|roads| roads.find_road(name).is_some())
}

/// Public helper to resolve a map name to an on-disk .map file if present.
pub fn find_map_file(map_name: &str) -> Option<PathBuf> {
    locate_map_file(map_name)
}

/// List the chunky chunk labels present in a map file (for debugging/loading).
pub fn inspect_map_chunks(map_name: &str) -> Option<Vec<String>> {
    inspect_map_chunks_from_chunky(&load_chunky_map(map_name).ok()??)
}

pub fn inspect_map_chunks_from_chunky(chunky: &ChunkyMap) -> Option<Vec<String>> {
    let mut labels: Vec<String> = chunky.toc.values().cloned().collect();
    labels.sort();
    Some(labels)
}

/// Load and decompress a chunky map file, returning metadata for further parsing.
pub fn load_chunky_map(map_name: &str) -> LoaderResult<Option<ChunkyMap>> {
    let Some(path) = locate_map_file(map_name) else {
        return Ok(None);
    };
    if let Some(cached) = cached_chunky_for(&path) {
        return Ok(Some(cached));
    }

    let raw_bytes = read_file_bytes_for_runtime(&path).ok_or_else(|| {
        configuration_error(format!(
            "Failed to read map '{}': path not found in virtual file system",
            path.display()
        ))
    })?;
    let bytes = if raw_bytes.starts_with(CHUNK_MAGIC) {
        raw_bytes
    } else {
        decompress_map_bytes(&raw_bytes).map_err(|err| {
            configuration_error(format!(
                "Failed to decompress map '{}': {}",
                path.display(),
                err
            ))
        })?
    };

    let (toc, body_offset) = parse_chunk_toc(&bytes)?;
    let chunky = ChunkyMap {
        source: path,
        toc,
        body_offset,
        bytes,
    };
    remember_loaded_chunky(&chunky);
    Ok(Some(chunky))
}
