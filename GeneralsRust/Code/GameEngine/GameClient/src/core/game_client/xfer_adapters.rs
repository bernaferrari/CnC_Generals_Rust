// Runtime/common Xfer adapters and live GameClient snapshot xfer.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

fn runtime_status_to_common(status: RuntimeXferStatus) -> CommonXferStatus {
    match status {
        RuntimeXferStatus::Invalid => CommonXferStatus::Invalid,
        RuntimeXferStatus::Ok => CommonXferStatus::Ok,
        RuntimeXferStatus::Eof => CommonXferStatus::Eof,
        RuntimeXferStatus::FileNotFound => CommonXferStatus::FileNotFound,
        RuntimeXferStatus::FileNotOpen => CommonXferStatus::FileNotOpen,
        RuntimeXferStatus::FileAlreadyOpen => CommonXferStatus::FileAlreadyOpen,
        RuntimeXferStatus::ReadError => CommonXferStatus::ReadError,
        RuntimeXferStatus::WriteError => CommonXferStatus::WriteError,
        RuntimeXferStatus::ModeUnknown => CommonXferStatus::ModeUnknown,
        RuntimeXferStatus::SkipError => CommonXferStatus::SkipError,
        RuntimeXferStatus::BeginEndMismatch => CommonXferStatus::BeginEndMismatch,
        RuntimeXferStatus::OutOfMemory => CommonXferStatus::OutOfMemory,
        RuntimeXferStatus::StringError => CommonXferStatus::StringError,
        RuntimeXferStatus::InvalidVersion => CommonXferStatus::InvalidVersion,
        RuntimeXferStatus::InvalidParameters => CommonXferStatus::InvalidParameters,
        RuntimeXferStatus::InvalidData => CommonXferStatus::InvalidData,
        RuntimeXferStatus::ListNotEmpty => CommonXferStatus::ListNotEmpty,
        RuntimeXferStatus::UnknownString => CommonXferStatus::UnknownString,
        RuntimeXferStatus::UnknownBlock | RuntimeXferStatus::ErrorUnknown => {
            CommonXferStatus::ErrorUnknown
        }
    }
}

fn common_status_to_runtime(status: CommonXferStatus) -> RuntimeXferStatus {
    match status {
        CommonXferStatus::Invalid => RuntimeXferStatus::Invalid,
        CommonXferStatus::Ok => RuntimeXferStatus::Ok,
        CommonXferStatus::Eof => RuntimeXferStatus::Eof,
        CommonXferStatus::FileNotFound => RuntimeXferStatus::FileNotFound,
        CommonXferStatus::FileNotOpen => RuntimeXferStatus::FileNotOpen,
        CommonXferStatus::FileAlreadyOpen => RuntimeXferStatus::FileAlreadyOpen,
        CommonXferStatus::ReadError => RuntimeXferStatus::ReadError,
        CommonXferStatus::WriteError => RuntimeXferStatus::WriteError,
        CommonXferStatus::ModeUnknown => RuntimeXferStatus::ModeUnknown,
        CommonXferStatus::SkipError => RuntimeXferStatus::SkipError,
        CommonXferStatus::BeginEndMismatch => RuntimeXferStatus::BeginEndMismatch,
        CommonXferStatus::OutOfMemory => RuntimeXferStatus::OutOfMemory,
        CommonXferStatus::StringError => RuntimeXferStatus::StringError,
        CommonXferStatus::InvalidVersion => RuntimeXferStatus::InvalidVersion,
        CommonXferStatus::InvalidParameters => RuntimeXferStatus::InvalidParameters,
        CommonXferStatus::InvalidData => RuntimeXferStatus::InvalidData,
        CommonXferStatus::ListNotEmpty => RuntimeXferStatus::ListNotEmpty,
        CommonXferStatus::UnknownString => RuntimeXferStatus::UnknownString,
        CommonXferStatus::ErrorUnknown => RuntimeXferStatus::ErrorUnknown,
    }
}

fn runtime_status_to_io(status: RuntimeXferStatus) -> io::Error {
    io::Error::new(
        match status {
            RuntimeXferStatus::Eof => ErrorKind::UnexpectedEof,
            RuntimeXferStatus::FileNotFound => ErrorKind::NotFound,
            RuntimeXferStatus::InvalidParameters | RuntimeXferStatus::InvalidVersion => {
                ErrorKind::InvalidInput
            }
            RuntimeXferStatus::InvalidData
            | RuntimeXferStatus::UnknownString
            | RuntimeXferStatus::UnknownBlock => ErrorKind::InvalidData,
            RuntimeXferStatus::WriteError => ErrorKind::WriteZero,
            _ => ErrorKind::Other,
        },
        format!("{status:?}"),
    )
}

/// Adapt the runtime save-game Xfer ABI to the older Common snapshot trait.
///
/// Several live GameClient systems still implement the Common `Snapshotable`
/// contract. Keeping this adapter crate-visible prevents individual systems
/// from inventing partial, incompatible serializers for the runtime chunk
/// stream.
pub(crate) struct RuntimeCommonXferAdapter<'a> {
    inner: &'a mut dyn RuntimeXfer,
}

impl<'a> RuntimeCommonXferAdapter<'a> {
    pub(crate) fn new(inner: &'a mut dyn RuntimeXfer) -> Self {
        Self { inner }
    }
}

impl Xfer for RuntimeCommonXferAdapter<'_> {
    fn get_xfer_mode(&self) -> CommonXferMode {
        match self.inner.get_xfer_mode() {
            RuntimeXferMode::Invalid => CommonXferMode::Invalid,
            RuntimeXferMode::Save => CommonXferMode::Save,
            RuntimeXferMode::Load => CommonXferMode::Load,
            RuntimeXferMode::Crc => CommonXferMode::Crc,
        }
    }

    fn get_identifier(&self) -> &str {
        self.inner.get_identifier()
    }

    fn set_options(&mut self, options: u32) {
        self.inner.set_options(options);
    }

    fn clear_options(&mut self, options: u32) {
        self.inner.clear_options(options);
    }

    fn get_options(&self) -> u32 {
        self.inner.get_options()
    }

    fn open(&mut self, identifier: &str) -> Result<(), CommonXferStatus> {
        self.inner
            .open(identifier.to_string())
            .map_err(runtime_status_to_common)
    }

    fn close(&mut self) -> Result<(), CommonXferStatus> {
        self.inner.close().map_err(runtime_status_to_common)
    }

    fn begin_block(
        &mut self,
    ) -> Result<game_engine::common::system::XferBlockSize, CommonXferStatus> {
        self.inner.begin_block().map_err(runtime_status_to_common)
    }

    fn end_block(&mut self) -> Result<(), CommonXferStatus> {
        self.inner.end_block().map_err(runtime_status_to_common)
    }

    fn skip(&mut self, data_size: i32) -> Result<(), CommonXferStatus> {
        self.inner.skip(data_size).map_err(runtime_status_to_common)
    }

    fn xfer_snapshot(
        &mut self,
        _snapshot: &mut dyn Snapshotable,
    ) -> Result<(), CommonXferStatus> {
        Err(CommonXferStatus::ModeUnknown)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        self.inner
            .xfer_ascii_string(ascii_string_data)
            .map_err(runtime_status_to_io)
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        self.inner
            .xfer_unicode_string(unicode_string_data)
            .map_err(runtime_status_to_io)
    }

    // SAFETY: Trait-contract forwarding: the caller owes a writable `data` buffer
    // SAFETY: of data_size bytes (RuntimeXfer contract); we pass it straight to the
    // SAFETY: inner adapter which enforces the same bounds. No pointer is stored.
    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        // SAFETY: Same caller-established contract as the enclosing method; see the
        // SAFETY: safety note on xfer_implementation above.
        unsafe { self.inner.xfer_implementation(data, data_size) }.map_err(runtime_status_to_io)
    }
}

pub(crate) fn xfer_live_game_client_state(
    xfer: &mut dyn RuntimeXfer,
) -> Result<(), RuntimeXferStatus> {
    with_live_game_client_mut(|client| {
        let current_version: u8 = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        // Backward-compatibility with older Rust save chunk version before
        // the C++-ordered envelope was introduced.
        if version <= 1 {
            xfer.xfer_unsigned_int(&mut client.frame)?;
            xfer.xfer_int(&mut client.local_player_id)?;
            xfer.xfer_unsigned_int(&mut client.rendered_object_count)?;

            let mut next_drawable_id = client.next_drawable_id.0;
            xfer.xfer_unsigned_int(&mut next_drawable_id)?;
            client.next_drawable_id = DrawableId(next_drawable_id.max(1));

            let mut startup_sizzle_pending = client.startup_sizzle_pending;
            xfer.xfer_bool(&mut startup_sizzle_pending)?;
            client.startup_sizzle_pending = startup_sizzle_pending;

            let mut drawable_count = client.drawable_map.len() as u32;
            xfer.xfer_unsigned_int(&mut drawable_count)?;

            if xfer.get_xfer_mode() == RuntimeXferMode::Load {
                if client.next_drawable_id.0 <= 1 && !client.drawable_map.is_empty() {
                    let max_id = client.drawable_map.keys().map(|id| id.0).max().unwrap_or(1);
                    client.next_drawable_id = DrawableId(max_id.saturating_add(1));
                }
                client.set_drawable_id_counter(client.next_drawable_id.0);
            }
            return Ok(());
        }

        // C++ parity envelope:
        // v3, frame, drawable TOC, drawable archive blocks, briefing history (v2+)
        xfer.xfer_unsigned_int(&mut client.frame)?;
        {
            let mut adapter = RuntimeCommonXferAdapter::new(xfer);
            client
                .xfer_drawable_toc(&mut adapter)
                .map_err(|_| RuntimeXferStatus::InvalidData)?;

            let save_entries = if adapter.is_writing() {
                client
                    .collect_saveable_drawables_sorted()
                    .map_err(|_| RuntimeXferStatus::InvalidData)?
            } else {
                client.drawable_map.clear();
                client.drawable_object_map.clear();
                client.presentation_direct_drawable_bindings.clear();
                Vec::new()
            };

            let mut drawable_count: u16 = save_entries
                .len()
                .try_into()
                .map_err(|_| RuntimeXferStatus::InvalidData)?;
            adapter
                .xfer_unsigned_short(&mut drawable_count)
                .map_err(|_| RuntimeXferStatus::InvalidData)?;

            if adapter.is_writing() {
                let toc_lookup: HashMap<String, u16> = client
                    .drawable_toc
                    .iter()
                    .map(|entry| (entry.name.clone(), entry.id))
                    .collect();

                for (drawable_id, template_name) in save_entries {
                    let Some(drawable) = client.drawable_map.get_mut(&drawable_id) else {
                        return Err(RuntimeXferStatus::InvalidData);
                    };
                    let mut toc_id = toc_lookup
                        .get(&template_name)
                        .copied()
                        .ok_or(RuntimeXferStatus::InvalidData)?;
                    adapter
                        .xfer_unsigned_short(&mut toc_id)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;

                    adapter.begin_block().map_err(common_status_to_runtime)?;
                    let mut object_id: ObjectID = drawable.get_object_id().unwrap_or(INVALID_ID);
                    adapter
                        .xfer_unsigned_int(&mut object_id)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;
                    GameClient::xfer_drawable_snapshot(drawable.as_mut(), &mut adapter)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;
                    adapter.end_block().map_err(common_status_to_runtime)?;
                }
            } else {
                let factory_guard =
                    get_thing_factory().map_err(|_| RuntimeXferStatus::InvalidData)?;
                let factory = factory_guard
                    .as_ref()
                    .ok_or(RuntimeXferStatus::InvalidData)?;

                for _ in 0..drawable_count {
                    let mut toc_id: u16 = 0;
                    adapter
                        .xfer_unsigned_short(&mut toc_id)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;

                    let toc_name = client
                        .find_toc_entry_by_id(toc_id)
                        .map(|entry| entry.name.clone())
                        .ok_or(RuntimeXferStatus::InvalidData)?;

                    let data_size = adapter.begin_block().map_err(common_status_to_runtime)?;

                    let Some(template) = factory.find_template(&toc_name, false) else {
                        adapter.skip(data_size).map_err(common_status_to_runtime)?;
                        continue;
                    };

                    let mut object_id: ObjectID = INVALID_ID;
                    adapter
                        .xfer_unsigned_int(&mut object_id)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;
                    // Host/presentation path: OBJECT_REGISTRY may be empty. Do not
                    // fail xfer; bind_drawable_to_object runs only when present.

                    let mut reuse_id = None;
                    if object_id != INVALID_ID {
                        if let Some(existing_id) = client.get_drawable_for_object(object_id) {
                            reuse_id = Some(existing_id);
                        }
                    }

                    let mut drawable = if let Some(existing_id) = reuse_id {
                        let needs_replace = client
                            .drawable_map
                            .get(&existing_id)
                            .map(|existing| {
                                !GameClient::drawable_matches_saved_template(
                                    existing.as_ref(),
                                    &template,
                                    factory,
                                )
                            })
                            .unwrap_or(true);
                        if needs_replace {
                            client
                                .destroy_drawable(existing_id)
                                .map_err(|_| RuntimeXferStatus::InvalidData)?;
                            None
                        } else {
                            client.drawable_map.remove(&existing_id)
                        }
                    } else {
                        None
                    };

                    if drawable.is_none() {
                        let created_id = client
                            .create_drawable_from_template(template.as_ref())
                            .map_err(|_| RuntimeXferStatus::InvalidData)?;
                        let mut created = client
                            .drawable_map
                            .remove(&created_id)
                            .ok_or(RuntimeXferStatus::InvalidData)?;
                        if object_id != INVALID_ID {
                            created.set_object_id(Some(object_id));
                        }
                        drawable = Some(created);
                    }

                    let mut drawable = drawable.ok_or(RuntimeXferStatus::InvalidData)?;
                    GameClient::xfer_drawable_snapshot(drawable.as_mut(), &mut adapter)
                        .map_err(|_| RuntimeXferStatus::InvalidData)?;

                    let id = drawable.get_id();
                    if let Some(object_id) = drawable.get_object_id() {
                        client.drawable_object_map.insert(object_id, id);
                    }
                    client.drawable_map.insert(id, drawable);

                    adapter.end_block().map_err(common_status_to_runtime)?;

                    if object_id != INVALID_ID {
                        // Dual-world residual bind only; host maps via drawable_object_map above.
                        if OBJECT_REGISTRY.get_object(object_id).is_some() {
                            let _ = client.bind_drawable_to_object(id, object_id);
                        }
                    }
                }
            }
        }

        if version >= 2 {
            let mut adapter = RuntimeCommonXferAdapter::new(xfer);
            xfer_diplomacy_briefing_history(&mut adapter, version)
                .map_err(|_| RuntimeXferStatus::InvalidData)?;
        }

        Ok(())
    })
    .unwrap_or(Err(RuntimeXferStatus::InvalidData))
}

pub(crate) fn run_live_game_client_load_post_process() -> Result<(), RuntimeXferStatus> {
    with_live_game_client_mut(|client| {
        client
            .load_post_process()
            .map_err(|_| RuntimeXferStatus::InvalidData)
    })
    .unwrap_or(Err(RuntimeXferStatus::InvalidData))
}
