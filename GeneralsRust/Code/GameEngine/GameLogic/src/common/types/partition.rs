// DummyXfer, partition manager, find-position options
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Module interfaces that the Object system needs

/// Dummy Xfer implementation for now
pub struct DummyXfer;

impl Xfer for DummyXfer {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Invalid
    }

    fn get_identifier(&self) -> &str {
        ""
    }

    fn set_options(&mut self, _options: u32) {}

    fn clear_options(&mut self, _options: u32) {}

    fn get_options(&self) -> u32 {
        0
    }

    fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        Ok(0)
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer_snapshot(
        &mut self,
        _snapshot: &mut dyn game_engine::system::Snapshotable,
    ) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
        Ok(())
    }

    fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
        Ok(())
    }

    // SAFETY: trait contract: caller guarantees `_data` is valid for
    // `_data_size` bytes; this no-op never dereferences it.
    unsafe fn xfer_implementation(&mut self, _data: *mut u8, _data_size: usize) -> io::Result<()> {
        let _ = (_data, _data_size); // Silence unused warning
        Ok(())
    }
}

// Partition manager traits (matching C++ partition system)

/// Base partition manager trait for spatial partitioning
pub trait PartitionManager: Send + Sync {
    /// Get objects within radius of a point
    fn get_objects_in_radius(&self, _pos: &Coord3D, _radius: Real) -> Vec<ObjectID> {
        Vec::new() // Default implementation
    }

    /// Reveal map for a specific player
    fn reveal_map_for_player(
        &self,
        _player_id: PlayerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation
    }
}

/// Unit-specific partition manager
pub trait UnitPartitionManager: PartitionManager {
    /// Get units within radius of a point
    fn get_units_in_radius(&self, pos: &Coord3D, radius: Real) -> Vec<ObjectID>;

    /// Find a legal position around a point
    fn find_position_around(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Options for finding positions around objects
#[derive(Debug, Clone)]
pub struct FindPositionOptions {
    pub min_radius: Real,
    pub max_radius: Real,
    pub start_angle: Option<Real>,
    pub max_z_delta: Real,
    pub flags: u32,
    pub relationship_object_id: Option<ObjectID>,
    pub ignore_object_id: Option<ObjectID>,
    pub source_to_path_to_dest_id: Option<ObjectID>,
}

impl Default for FindPositionOptions {
    fn default() -> Self {
        Self {
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: None,
            max_z_delta: 99999.0,
            flags: 0,
            relationship_object_id: None,
            ignore_object_id: None,
            source_to_path_to_dest_id: None,
        }
    }
}

