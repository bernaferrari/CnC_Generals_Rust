use super::*;

impl AudioManager {
    pub fn get_audio_length_ms(&mut self, event: &AudioEventRts) -> Real {
        // C++ parity: clone event, resolve concrete filenames, then sum attack/main/decay lengths.
        let mut tmp_event = event.clone();
        if tmp_event.get_audio_event_info().is_none() {
            if let Some(info) = self.find_audio_event_info(event.get_event_name()) {
                tmp_event.set_audio_event_info(info);
            } else {
                return 0.0;
            }
        }

        let paths = self.audio_paths();
        tmp_event.generate_filename_with_settings(&paths, &mut self.audio_random);
        tmp_event.generate_play_info_with_settings(&paths, &mut self.audio_random);

        self.get_file_length_ms(tmp_event.get_attack_filename())
            + self.get_file_length_ms(tmp_event.get_filename())
            + self.get_file_length_ms(tmp_event.get_decay_filename())
    }

    pub(super) fn get_file_length_ms(&self, file_path: &str) -> Real {
        let normalized = file_path.trim();
        if normalized.is_empty() {
            return 0.0;
        }

        if let Some(length) = self
            .file_length_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(normalized)
            .copied()
        {
            return length;
        }

        let length = Self::read_audio_file_bytes(normalized)
            .and_then(|bytes| Self::duration_ms_from_audio_data(&bytes))
            .unwrap_or(0.0);

        self.file_length_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(normalized.to_string(), length);

        length
    }

    pub(super) fn read_audio_file_bytes(file_path: &str) -> Option<Vec<u8>> {
        let mut candidates: Vec<String> = Vec::new();
        let trimmed = file_path.trim();
        if trimmed.is_empty() {
            return None;
        }

        candidates.push(trimmed.to_string());
        let slash_variant = trimmed.replace('\\', "/");
        if slash_variant != trimmed {
            candidates.push(slash_variant.clone());
        }

        if std::path::Path::new(trimmed).extension().is_none() {
            for ext in [".wav", ".mp3", ".ogg"] {
                candidates.push(format!("{trimmed}{ext}"));
                if slash_variant != trimmed {
                    candidates.push(format!("{slash_variant}{ext}"));
                }
            }
        }

        candidates.sort();
        candidates.dedup();

        for candidate in candidates {
            if let Some(data) = Self::read_from_virtual_file_system(&candidate) {
                return Some(data);
            }

            if let Ok(data) = std::fs::read(&candidate) {
                return Some(data);
            }
        }

        None
    }

    pub(super) fn read_from_virtual_file_system(path: &str) -> Option<Vec<u8>> {
        let file_system = get_file_system();
        let Ok(mut guard) = file_system.lock() else {
            return None;
        };
        let mut file = guard.open_file(path, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }

    pub(super) fn duration_ms_from_audio_data(data: &[u8]) -> Option<Real> {
        Self::duration_ms_from_wav(data)
            .or_else(|| Self::duration_ms_from_mp3(data))
            .or_else(|| Self::duration_ms_from_ogg(data))
    }

    pub(super) fn duration_ms_from_wav(data: &[u8]) -> Option<Real> {
        let reader = WavReader::new(Cursor::new(data)).ok()?;
        let spec = reader.spec();
        if spec.sample_rate == 0 {
            return None;
        }

        // hound duration is interleaved-sample count; divide by channel count to get frame count.
        let channels = spec.channels.max(1) as f64;
        let samples = reader.duration() as f64;
        let frames = samples / channels;
        Some((frames * 1000.0 / spec.sample_rate as f64) as Real)
    }

    pub(super) fn duration_ms_from_mp3(data: &[u8]) -> Option<Real> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = data;
            return None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut decoder = Mp3Decoder::new(Cursor::new(data));
            let mut total_ms = 0.0f64;

            loop {
                match decoder.next_frame() {
                    Ok(frame) => {
                        if frame.sample_rate <= 0 {
                            continue;
                        }
                        let channels = frame.channels.max(1) as f64;
                        let samples = frame.data.len() as f64;
                        let frames = samples / channels;
                        total_ms += frames * 1000.0 / frame.sample_rate as f64;
                    }
                    Err(Mp3Error::Eof) => break,
                    Err(Mp3Error::SkippedData) => continue,
                    Err(_) => return None,
                }
            }

            (total_ms > 0.0).then_some(total_ms as Real)
        }
    }

    pub(super) fn duration_ms_from_ogg(data: &[u8]) -> Option<Real> {
        let mut reader = OggStreamReader::new(Cursor::new(data)).ok()?;
        let sample_rate = reader.ident_hdr.audio_sample_rate;
        if sample_rate == 0 {
            return None;
        }

        let channels = reader.ident_hdr.audio_channels.max(1) as f64;
        let mut total_frames = 0.0f64;
        loop {
            match reader.read_dec_packet_itl() {
                Ok(Some(packet)) => {
                    total_frames += packet.len() as f64 / channels;
                }
                Ok(None) => break,
                Err(_) => return None,
            }
        }

        (total_frames > 0.0).then_some((total_frames * 1000.0 / sample_rate as f64) as Real)
    }
}
