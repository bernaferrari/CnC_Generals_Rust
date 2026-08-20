// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl SubsystemInterface for ControlBar {
    fn init(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!("Initializing Control Bar");

        if let Some(scheme_manager) = &self.scheme_manager {
            scheme_manager.load_scheme("Default")?;
        }

        self.ensure_generals_exp_layout();
        leftover_ensure_named_window(WIN_U_ATTACK);
        leftover_ensure_named_window(BUTTON_GENERAL);
        leftover_ensure_named_window(BUTTON_LARGE);
        leftover_ensure_named_window(CONTROL_BAR_PARENT);


        log::info!("Control Bar initialized successfully");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn Error>> {
        let delta_time = Duration::from_millis(33);
        self.update(delta_time)?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!("Resetting Control Bar");

        if let Ok(mut context) = self.context.write() {
            *context = ControlBarContext::default();
        }

        self.button_states.clear();
        self.is_animating = false;
        self.build_queue_data.clear();
        self.displayed_queue_count = 0;
        self.ui_dirty = false;
        self.flash_active = false;
        self.control_bar_stage = ControlBarStage::Default;
        self.portrait_state = PortraitDisplayState::default();
        self.science_state = SciencePurchaseState::default();
        self.gen_star_flash = true;
        self.last_flashed_at_point_value = -1;
        self.radar_attack_glow_on = false;
        self.remaining_radar_attack_glow_frames = 0;
        self.science_layout_loaded = false;
        if self.rally_point_drawable_id != 0 {
            gamelogic::helpers::TheGameClient.destroy_drawable(self.rally_point_drawable_id);
        }
        self.rally_point_drawable_id = 0;
        self.default_control_bar_captured = false;
        self.special_power_shortcut_layout.clear();
        self.radar_glow_window_enabled = true;
        self.displayed_construct_percent = -1.0;
        self.displayed_ocl_timer_seconds = 0;
        self.special_power_shortcuts.clear();
        self.special_power_shortcut_count = 0;

        Ok(())
    }
}

impl Default for ControlBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            enabled: true,
            visible: true,
            pressed: false,
            progress: 0.0,
            flash_time: None,
            availability: CommandAvailability::Available,
            check_like_active: false,
        }
    }
}
