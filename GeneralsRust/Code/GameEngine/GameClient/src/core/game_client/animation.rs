// W3D animation duration resolution used by drawable preload.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

struct AnimationDurationResolver {
    asset_manager: Arc<AssetManager>,
    cache_ms: Mutex<HashMap<String, Option<Real>>>,
}

impl AnimationDurationResolver {
    fn new(asset_manager: Arc<AssetManager>) -> Self {
        Self {
            asset_manager,
            cache_ms: Mutex::new(HashMap::new()),
        }
    }

    fn get_duration_ms(&self, animation_name: &str) -> Option<Real> {
        let normalized = normalize_animation_name(animation_name);
        if normalized.is_empty() {
            return None;
        }

        if let Ok(cache) = self.cache_ms.lock() {
            if let Some(cached) = cache.get(&normalized) {
                return *cached;
            }
        }

        let resolved = self.resolve_uncached(&normalized);
        if let Ok(mut cache) = self.cache_ms.lock() {
            cache.insert(normalized, resolved);
        }
        resolved
    }

    fn resolve_uncached(&self, animation_name: &str) -> Option<Real> {
        for candidate in animation_file_candidates(animation_name) {
            let Ok(data) = pollster::block_on(self.asset_manager.load_raw_data_exact(&candidate))
            else {
                continue;
            };
            if let Some(duration_ms) = extract_animation_duration_ms(&data, animation_name) {
                return Some(duration_ms);
            }
        }
        self.resolve_via_global_scan(animation_name)
    }

    fn resolve_via_global_scan(&self, animation_name: &str) -> Option<Real> {
        let paths = self.asset_manager.list_asset_paths_with_extension("w3d");
        for path in paths {
            let Ok(data) = pollster::block_on(self.asset_manager.load_raw_data_exact(&path)) else {
                continue;
            };
            let durations = extract_all_animation_durations_ms(&data);
            if durations.is_empty() {
                continue;
            }

            let mut matched: Option<Real> = None;
            if let Ok(mut cache) = self.cache_ms.lock() {
                for (name, duration_ms) in durations {
                    let key = normalize_animation_name(&name);
                    if key.is_empty() || duration_ms <= 0.0 {
                        continue;
                    }
                    cache.entry(key.clone()).or_insert(Some(duration_ms));
                    if animation_name_matches(animation_name, &key) {
                        matched = Some(duration_ms);
                    }
                }

                if let Some(duration_ms) = matched {
                    cache.insert(animation_name.to_string(), Some(duration_ms));
                    return Some(duration_ms);
                }
            } else {
                for (name, duration_ms) in durations {
                    if duration_ms > 0.0 && animation_name_matches(animation_name, &name) {
                        return Some(duration_ms);
                    }
                }
            }
        }
        None
    }
}

fn normalize_animation_name(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn animation_file_candidates(animation_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    let normalized = normalize_animation_name(animation_name);

    let mut push_candidate = |raw: String| {
        let candidate = raw.trim().replace('\\', "/");
        if candidate.is_empty() {
            return;
        }
        if seen.insert(candidate.clone()) {
            candidates.push(PathBuf::from(candidate));
        }
    };

    let with_ext = |name: &str| -> String {
        if name.ends_with(".w3d") {
            name.to_string()
        } else {
            format!("{name}.w3d")
        }
    };

    push_candidate(with_ext(&normalized));
    if normalized.ends_with(".w3d") {
        push_candidate(normalized.trim_end_matches(".w3d").to_string());
    }

    if let Some((prefix, _)) = normalized.split_once('.') {
        push_candidate(with_ext(prefix));
    }
    if let Some((prefix, _)) = normalized.rsplit_once('.') {
        push_candidate(with_ext(prefix));
    }

    candidates
}

fn extract_animation_duration_ms(data: &[u8], animation_name: &str) -> Option<Real> {
    let mut reader = W3DReader::new(Cursor::new(data));
    let chunks = reader.read_all_chunks().ok()?;
    extract_animation_duration_ms_from_chunks(&chunks, animation_name)
}

fn extract_all_animation_durations_ms(data: &[u8]) -> Vec<(String, Real)> {
    let mut reader = W3DReader::new(Cursor::new(data));
    let Ok(chunks) = reader.read_all_chunks() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    extract_all_animation_durations_ms_from_chunks(&chunks, &mut found);
    found
}

fn extract_animation_duration_ms_from_chunks(
    chunks: &[W3DChunk],
    animation_name: &str,
) -> Option<Real> {
    for chunk in chunks {
        match chunk {
            W3DChunk::Animation(animation) => {
                if animation_name_matches(animation_name, &animation.header.name_str()) {
                    if let Some(duration_ms) = calculate_duration_ms(
                        animation.header.num_frames,
                        animation.header.frame_rate,
                    ) {
                        return Some(duration_ms);
                    }
                }
            }
            W3DChunk::AnimationHeader(header) => {
                if animation_name_matches(animation_name, &header.name_str()) {
                    if let Some(duration_ms) =
                        calculate_duration_ms(header.num_frames, header.frame_rate)
                    {
                        return Some(duration_ms);
                    }
                }
            }
            W3DChunk::CompressedAnimation(sub_chunks) => {
                if let Some(duration_ms) =
                    extract_animation_duration_ms_from_chunks(sub_chunks, animation_name)
                {
                    return Some(duration_ms);
                }
            }
            W3DChunk::CompressedAnimationHeader(header) => {
                let header_name = chunk_name_str(&header.name);
                if animation_name_matches(animation_name, &header_name) {
                    if let Some(duration_ms) =
                        calculate_duration_ms(header.num_frames, u32::from(header.frame_rate))
                    {
                        return Some(duration_ms);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_all_animation_durations_ms_from_chunks(
    chunks: &[W3DChunk],
    out: &mut Vec<(String, Real)>,
) {
    for chunk in chunks {
        match chunk {
            W3DChunk::Animation(animation) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(animation.header.num_frames, animation.header.frame_rate)
                {
                    out.push((animation.header.name_str(), duration_ms));
                }
            }
            W3DChunk::AnimationHeader(header) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(header.num_frames, header.frame_rate)
                {
                    out.push((header.name_str(), duration_ms));
                }
            }
            W3DChunk::CompressedAnimation(sub_chunks) => {
                extract_all_animation_durations_ms_from_chunks(sub_chunks, out);
            }
            W3DChunk::CompressedAnimationHeader(header) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(header.num_frames, u32::from(header.frame_rate))
                {
                    out.push((chunk_name_str(&header.name), duration_ms));
                }
            }
            _ => {}
        }
    }
}

fn chunk_name_str(bytes: &[u8; 16]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_ascii_lowercase()
}

fn calculate_duration_ms(num_frames: u32, frame_rate: u32) -> Option<Real> {
    if num_frames == 0 || frame_rate == 0 {
        return None;
    }
    Some((num_frames as Real * 1000.0) / frame_rate as Real)
}

fn animation_name_matches(requested: &str, candidate: &str) -> bool {
    let requested = normalize_animation_name(requested);
    let candidate = normalize_animation_name(candidate);
    if requested.is_empty() || candidate.is_empty() {
        return false;
    }
    if requested == candidate {
        return true;
    }

    let requested_trimmed = requested.strip_suffix(".w3d").unwrap_or(&requested);
    let candidate_trimmed = candidate.strip_suffix(".w3d").unwrap_or(&candidate);
    if requested_trimmed == candidate_trimmed {
        return true;
    }

    if let Some((_, requested_tail)) = requested_trimmed.rsplit_once('.') {
        if requested_tail == candidate_trimmed {
            return true;
        }
    }
    if let Some((_, candidate_tail)) = candidate_trimmed.rsplit_once('.') {
        if requested_trimmed == candidate_tail {
            return true;
        }
    }

    false
}
