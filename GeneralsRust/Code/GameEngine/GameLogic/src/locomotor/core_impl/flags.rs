impl Locomotor {
    // ========================================================================
    // SNAPSHOTABLE (Xfer / CRC / LoadPostProcess)
    // Matches C++ Locomotor.cpp:712-758
    // ========================================================================

    /// CRC — checksum for save game validation.
    /// Matches C++ Locomotor::crc (Locomotor.cpp:712-715)
    pub fn loco_crc(&self, xfer: &mut dyn game_engine::system::Xfer) -> Result<(), String> {
        // C++ implementation is empty
        let _ = xfer;
        Ok(())
    }

    /// Xfer — serialize/deserialize locomotor state for save/load.
    /// Matches C++ Locomotor::xfer (Locomotor.cpp:722-750)
    /// Version 2 adds donutTimer.
    pub fn loco_xfer(&mut self, xfer: &mut dyn game_engine::system::Xfer) -> Result<(), String> {
        use game_engine::system::Xfer as XferTrait;

        // Version
        const CURRENT_VERSION: u8 = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("Locomotor xfer version: {:?}", e))?;

        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.donut_timer)
                .map_err(|e| format!("Locomotor xfer donutTimer: {:?}", e))?;
        }

        xfer.xfer_real(&mut self.maintain_pos.x)
            .map_err(|e| format!("Locomotor xfer maintainPos.x: {:?}", e))?;
        xfer.xfer_real(&mut self.maintain_pos.y)
            .map_err(|e| format!("Locomotor xfer maintainPos.y: {:?}", e))?;
        xfer.xfer_real(&mut self.maintain_pos.z)
            .map_err(|e| format!("Locomotor xfer maintainPos.z: {:?}", e))?;

        xfer.xfer_real(&mut self.braking_factor)
            .map_err(|e| format!("Locomotor xfer brakingFactor: {:?}", e))?;
        xfer.xfer_real(&mut self.max_lift)
            .map_err(|e| format!("Locomotor xfer maxLift: {:?}", e))?;
        xfer.xfer_real(&mut self.max_speed)
            .map_err(|e| format!("Locomotor xfer maxSpeed: {:?}", e))?;
        xfer.xfer_real(&mut self.max_accel)
            .map_err(|e| format!("Locomotor xfer maxAccel: {:?}", e))?;
        xfer.xfer_real(&mut self.max_braking)
            .map_err(|e| format!("Locomotor xfer maxBraking: {:?}", e))?;
        xfer.xfer_real(&mut self.max_turn_rate)
            .map_err(|e| format!("Locomotor xfer maxTurnRate: {:?}", e))?;
        xfer.xfer_real(&mut self.close_enough_dist)
            .map_err(|e| format!("Locomotor xfer closeEnoughDist: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.flags)
            .map_err(|e| format!("Locomotor xfer flags: {:?}", e))?;
        xfer.xfer_real(&mut self.preferred_height)
            .map_err(|e| format!("Locomotor xfer preferredHeight: {:?}", e))?;
        xfer.xfer_real(&mut self.preferred_height_damping)
            .map_err(|e| format!("Locomotor xfer preferredHeightDamping: {:?}", e))?;
        xfer.xfer_real(&mut self.angle_offset)
            .map_err(|e| format!("Locomotor xfer angleOffset: {:?}", e))?;
        xfer.xfer_real(&mut self.offset_increment)
            .map_err(|e| format!("Locomotor xfer offsetIncrement: {:?}", e))?;

        Ok(())
    }

    /// Load post-process — no-op, matches C++ Locomotor::loadPostProcess (Locomotor.cpp:755-758)
    pub fn loco_load_post_process(&mut self) -> Result<(), String> {
        // C++ implementation is empty
        Ok(())
    }

    // Flag helpers
    fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    pub fn is_braking(&self) -> bool {
        self.get_flag(FLAG_IS_BRAKING)
    }

    pub fn is_moving_backwards(&self) -> bool {
        self.get_flag(FLAG_MOVING_BACKWARDS)
    }

    pub fn is_climbing(&self) -> bool {
        self.get_flag(FLAG_CLIMBING)
    }

    pub fn is_offset_increasing(&self) -> bool {
        self.get_flag(FLAG_OFFSET_INCREASING)
    }

    // Setters
    pub fn set_max_speed(&mut self, speed: Real) {
        self.max_speed = speed;
    }

    pub fn set_max_turn_rate(&mut self, rate: Real) {
        self.max_turn_rate = rate;
    }

    pub fn set_max_acceleration(&mut self, accel: Real) {
        self.max_accel = accel;
    }

    pub fn set_max_lift(&mut self, lift: Real) {
        self.max_lift = lift;
    }

    pub fn set_preferred_height(&mut self, height: Real) {
        self.preferred_height = height;
    }

    pub fn set_close_enough_dist(&mut self, dist: Real) {
        self.close_enough_dist = dist;
    }

    pub fn get_close_enough_dist(&self) -> Real {
        self.close_enough_dist
    }

    pub fn set_precise_z_pos(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_PRECISE_Z_POS;
        } else {
            self.flags &= !FLAG_PRECISE_Z_POS;
        }
    }

    pub fn set_no_slow_down(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_NO_SLOW_DOWN;
        } else {
            self.flags &= !FLAG_NO_SLOW_DOWN;
        }
    }

    pub fn set_allow_invalid_position(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_ALLOW_INVALID_POS;
        } else {
            self.flags &= !FLAG_ALLOW_INVALID_POS;
        }
    }

    pub fn is_allowing_invalid_positions(&self) -> bool {
        (self.flags & FLAG_ALLOW_INVALID_POS) != 0
    }

    pub fn set_ultra_accurate(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_ULTRA_ACCURATE;
        } else {
            self.flags &= !FLAG_ULTRA_ACCURATE;
        }
    }

    // Getters for flags
    pub fn uses_precise_z_pos(&self) -> bool {
        (self.flags & FLAG_PRECISE_Z_POS) != 0
    }

    pub fn no_slow_down_approaching_dest(&self) -> bool {
        (self.flags & FLAG_NO_SLOW_DOWN) != 0
    }

    pub fn allows_invalid_position(&self) -> bool {
        (self.flags & FLAG_ALLOW_INVALID_POS) != 0
    }

    pub fn is_ultra_accurate(&self) -> bool {
        (self.flags & FLAG_ULTRA_ACCURATE) != 0
    }
}

