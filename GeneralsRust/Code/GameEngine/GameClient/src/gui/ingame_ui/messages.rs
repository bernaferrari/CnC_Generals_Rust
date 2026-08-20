// HUD messages, military subtitles, and tooltip helpers.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    // ── HUD message system ──────────────────────────────────────────────
    // C++: InGameUI::message() (InGameUI.cpp:1993), addMessageText() (InGameUI.cpp:2061)

    pub fn message(&mut self, text: &str) {
        self.add_message_text(text, None);
    }

    pub fn message_color(&mut self, text: &str, color: u32) {
        // C++ messageColor ORs GameMakeColor(0,0,0,255) (InGameUI.cpp:2069).
        self.add_message_text(text, Some(color | 0xFF00_0000));
    }

    /// C++ InGameUI.cpp:1633 — `m_messageDelayMS / LOGICFRAMES_PER_SECOND / 1000`.
    pub fn message_timeout_frames(message_delay_ms: i32) -> u32 {
        (message_delay_ms / 30 / 1000).max(0) as u32
    }

    /// C++ addMessageText color pick (InGameUI.cpp:2100-2103).
    pub fn next_message_color(
        list_empty: bool,
        newest_original: u32,
        color1: u32,
        color2: u32,
    ) -> u32 {
        if list_empty || newest_original == color2 {
            color1
        } else {
            color2
        }
    }

    /// C++ per-frame fade from add-time alpha (InGameUI.cpp:1639-1656).
    /// Idempotent: recomputes from the original color so update+pre_draw
    /// cannot double-subtract.
    pub fn message_alpha_at_age(original_a: u8, age: u32, timeout: u32) -> u8 {
        if age <= timeout {
            return original_a;
        }
        let mut alpha = original_a as i32;
        for t in (timeout + 1)..=age {
            let amount = (t as f32 * 0.01) as i32;
            alpha -= amount;
            if alpha <= 0 {
                return 0;
            }
        }
        alpha as u8
    }

    fn add_message_text(&mut self, text: &str, rgb_color: Option<u32>) {
        // C++ addMessageText has no m_messagesOn gate — toggle only hides draw.
        let color1 = rgb_color.unwrap_or(self.message_color1);
        let color2 = rgb_color.unwrap_or(self.message_color2);
        let newest = self.messages.first().map(|m| m.original_color).unwrap_or(0);
        let color = Self::next_message_color(self.messages.is_empty(), newest, color1, color2);

        let msg = MessageText {
            text: text.to_string(),
            color,
            original_color: color,
            creation_frame: self.current_frame,
        };

        self.messages.insert(0, msg);
        if self.messages.len() > MAX_UI_MESSAGES {
            self.messages.truncate(MAX_UI_MESSAGES);
        }
    }

    pub fn toggle_messages(&mut self) -> bool {
        self.messages_enabled = !self.messages_enabled;
        self.messages_enabled
    }

    pub fn are_messages_enabled(&self) -> bool {
        self.messages_enabled
    }

    pub fn get_message_color(&self, index: i32) -> u32 {
        if index % 2 == 0 {
            self.message_color1
        } else {
            self.message_color2
        }
    }

    /// C++ InGameUI::update message fade (InGameUI.cpp:1636-1661).
    pub fn expire_messages(&mut self) {
        let message_timeout = Self::message_timeout_frames(self.message_delay_ms);
        let current_frame = self.current_frame;
        let mut i = self.messages.len();
        while i > 0 {
            i -= 1;
            let age = current_frame.saturating_sub(self.messages[i].creation_frame);
            let (r, g, b, orig_a) = Self::unpack_argb(self.messages[i].original_color);
            let new_a = Self::message_alpha_at_age(orig_a, age, message_timeout);
            if new_a == 0 {
                self.messages.remove(i);
            } else {
                self.messages[i].color = Self::pack_argb(r, g, b, new_a);
            }
        }
    }

    fn unpack_argb(color: u32) -> (u8, u8, u8, u8) {
        (
            ((color >> 16) & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            (color & 0xFF) as u8,
            ((color >> 24) & 0xFF) as u8,
        )
    }

    fn pack_argb(r: u8, g: u8, b: u8, a: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    pub fn remove_message_at_index(&mut self, index: usize) {
        if index < self.messages.len() {
            self.messages.remove(index);
        }
    }

    pub fn get_messages(&self) -> &[MessageText] {
        &self.messages
    }

    pub fn get_message_color1(&self) -> u32 {
        self.message_color1
    }

    pub fn get_message_color2(&self) -> u32 {
        self.message_color2
    }

    pub fn get_message_position(&self) -> (i32, i32) {
        self.message_position
    }

    pub fn get_message_font_name(&self) -> &str {
        &self.message_font_name
    }

    pub fn get_message_point_size(&self) -> i32 {
        self.message_point_size
    }

    pub fn is_message_bold(&self) -> bool {
        self.message_bold
    }

    // ── Military subtitle system ─────────────────────────────────────────
    // C++: InGameUI::militarySubtitle() (InGameUI.cpp:4039)
    // C++: InGameUI::removeMilitarySubtitle() (InGameUI.cpp:4093)

    pub fn military_subtitle(&mut self, title: &str, duration_ms: i32) {
        crate::gui::ingame_ui::live_hud::start_military_subtitle(title, duration_ms);
        // C++ InGameUI.cpp:4042 — drop any existing caption first.
        self.remove_military_subtitle();
        update_diplomacy_briefing_text(title, false);
        let title = Self::military_caption_text(title);
        if title.is_empty() || duration_ms <= 0 {
            return;
        }

        let multiplier_x = self.screen_size.x / 800.0;
        let multiplier_y = self.screen_size.y / 600.0;

        let pos_x = self.military_caption_position.0 as f32 * multiplier_x;
        let pos_y = self.military_caption_position.1 as f32 * multiplier_y;

        let lifetime_frame = self.current_frame + (30 * duration_ms as u32) / 1000;
        self.disable_tooltips_until(lifetime_frame);

        let color = ((self.military_caption_color.3 as u32) << 24)
            | ((self.military_caption_color.0 as u32) << 16)
            | ((self.military_caption_color.1 as u32) << 8)
            | (self.military_caption_color.2 as u32);

        self.current_military_subtitle = Some(MilitarySubtitle {
            text: title,
            index: 0,
            position: (pos_x, pos_y),
            lifetime_frame,
            block_drawn: true,
            block_begin_frame: self.current_frame,
            block_pos: (pos_x, pos_y),
            increment_on_frame: self.current_frame + Self::military_caption_delay_frames(),
            color,
            display_lines: vec![String::new()],
            current_display_string: 0,
        });
    }


    fn military_caption_text(label: &str) -> String {
        GameText::fetch(label)
    }

    fn mouseover_tooltip_text(real_template_name: &str, display_name: &str) -> Option<String> {
        let raw = display_name.trim().to_string();
        // C++ compares the RAW display name to OBJECT:Prop before fallback.
        if raw == GameText::fetch("OBJECT:Prop") {
            return None;
        }
        let tooltip = if raw.is_empty() {
            GameText::fetch(&format!("ThingTemplate:{real_template_name}"))
        } else {
            raw
        };
        if tooltip.is_empty() {
            return None;
        }
        Some(tooltip)
    }

    fn mouseover_tooltip_for_template(template_name: &str) -> Option<String> {
        Self::mouseover_tooltip_for_templates(template_name, template_name)
    }

    fn mouseover_tooltip_for_templates(
        apparent_template_name: &str,
        real_template_name: &str,
    ) -> Option<String> {
        let display_name = get_thing_factory()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .and_then(|factory| factory.find_template(apparent_template_name, false))
                    .map(|template| template.get_display_name().to_string())
            })
            .unwrap_or_default();
        Self::mouseover_tooltip_text(real_template_name, &display_name)
    }

    fn format_supply_warehouse_tooltip_feedback(
        label: &str,
        boxes_stored: i32,
        base_value_per_supply_box: i32,
    ) -> String {
        let value = boxes_stored * base_value_per_supply_box;
        let value_text = value.to_string();
        if label.contains("%d") {
            label.replace("%d", &value_text)
        } else if label.contains("%i") {
            label.replace("%i", &value_text)
        } else if label.contains("{}") {
            label.replacen("{}", &value_text, 1)
        } else {
            format!("{label}{value_text}")
        }
    }

    fn supply_warehouse_tooltip_feedback(
        boxes_stored: i32,
        base_value_per_supply_box: i32,
    ) -> String {
        let label = GameText::fetch("TOOLTIP:SupplyWarehouse");
        Self::format_supply_warehouse_tooltip_feedback(
            &label,
            boxes_stored,
            base_value_per_supply_box,
        )
    }

    fn supply_warehouse_boxes_for_object(object: &Object) -> Option<i32> {
        for behavior in object.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(dock) = behavior.get_dock_update_interface() else {
                continue;
            };
            if let Some(boxes) = dock.supply_warehouse_boxes_stored() {
                return Some(boxes);
            }
        }
        None
    }

    fn mouseover_tooltip_template_for_object(object: &Object) -> String {
        let local_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let local_player_guard = local_player.as_ref().and_then(|player| player.read().ok());

        if Self::disguise_visible_player_index_for_object(object, local_player_guard.as_deref())
            .is_some()
        {
            if let Some(template_name) = get_disguise_manager()
                .lock()
                .ok()
                .and_then(|manager| manager.get_disguise(object.get_id()).ok())
            {
                return template_name;
            }
        }

        object.get_template_name().to_string()
    }

    fn mouseover_tooltip_color_for_object(object: &Object) -> [u8; 4] {
        let local_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let local_player_guard = local_player.as_ref().and_then(|player| player.read().ok());

        if let Some(disguised_index) =
            Self::disguise_visible_player_index_for_object(object, local_player_guard.as_deref())
        {
            if let Some(disguised_color) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(disguised_index).cloned())
                .and_then(|player| player.read().ok().map(|player| player.get_player_color()))
            {
                return [
                    disguised_color.r,
                    disguised_color.g,
                    disguised_color.b,
                    disguised_color.a,
                ];
            }
        }

        let mut color = object.get_indicator_color();
        if let Some(contain) = object.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                if contain_guard.is_garrisonable() {
                    if let Some(player) =
                        contain_guard.get_apparent_controlling_player(local_player_guard.as_deref())
                    {
                        if let Ok(player_guard) = player.read() {
                            color = player_guard.get_player_color();
                        }
                    }
                }
            }
        }

        [color.r, color.g, color.b, color.a]
    }

    fn mouseover_tooltip_player_for_object(object: &Object) -> Option<Arc<RwLock<Player>>> {
        let local_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let local_player_guard = local_player.as_ref().and_then(|player| player.read().ok());

        let mut player = object.get_contain().and_then(|contain| {
            contain.lock().ok().and_then(|contain_guard| {
                contain_guard.get_apparent_controlling_player(local_player_guard.as_deref())
            })
        });

        if player.is_none() {
            player = object.get_controlling_player();
        }

        if let Some(disguised_index) =
            Self::disguise_visible_player_index_for_object(object, local_player_guard.as_deref())
        {
            if let Some(disguised_player) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(disguised_index).cloned())
            {
                player = Some(disguised_player);
            }
        }

        player
    }

    fn mouseover_tooltip_with_player_suffix(
        tooltip: &str,
        player: &Player,
        is_multiplayer: bool,
    ) -> String {
        if is_multiplayer && player.is_playable_side() {
            format!("{}\n{}", tooltip, player.get_player_display_name())
        } else {
            tooltip.to_string()
        }
    }

    fn mouseover_tooltip_is_multiplayer() -> bool {
        with_recorder(|recorder| recorder.is_multiplayer()).unwrap_or(false)
    }

    fn mouseover_tooltip_visible_for_shroud(status: ObjectShroudStatus) -> bool {
        status == ObjectShroudStatus::Clear
    }

    fn military_caption_delay_frames() -> u32 {
        let delay_ms = get_global_language_read()
            .map(|language| language.military_caption_delay_ms)
            .unwrap_or(750);
        Self::milliseconds_to_logic_frames(delay_ms)
    }

    fn milliseconds_to_logic_frames(milliseconds: i32) -> u32 {
        (30 * milliseconds.max(0) as u32) / 1000
    }

    pub fn remove_military_subtitle(&mut self) {
        self.current_military_subtitle = None;
        self.clear_tooltips_disabled();
    }

    pub fn get_military_subtitle(&self) -> Option<&MilitarySubtitle> {
        self.current_military_subtitle.as_ref()
    }

    /// Typed prefix only. Empty until the first post-delay character.
    pub fn military_subtitle_visible_text(&self) -> Option<String> {
        self.current_military_subtitle
            .as_ref()
            .map(MilitarySubtitle::visible_text)
    }


    pub fn expire_military_subtitle(&mut self) {
        if let Some(sub) = &self.current_military_subtitle {
            if self.current_frame >= sub.lifetime_frame {
                self.remove_military_subtitle();
            }
        }
    }

    pub fn disable_tooltips_until(&mut self, frame_num: u32) {
        if frame_num > self.tooltips_disabled_until {
            self.tooltips_disabled_until = frame_num;
        }
    }

    pub fn clear_tooltips_disabled(&mut self) {
        self.tooltips_disabled_until = 0;
    }

    pub fn are_tooltips_disabled(&self) -> bool {
        self.current_frame < self.tooltips_disabled_until
    }

    fn update_military_subtitle(&mut self) {
        let had_subtitle = self.current_military_subtitle.is_some();
        if let Some(subtitle) = self.current_military_subtitle.as_mut() {
            if gamelogic::helpers::TheScriptEngine::is_time_frozen_script() {
                subtitle.lifetime_frame = subtitle.lifetime_frame.saturating_sub(1);
                subtitle.block_begin_frame = subtitle.block_begin_frame.saturating_sub(1);
                subtitle.increment_on_frame = subtitle.increment_on_frame.saturating_sub(1);
            }
        }
        let speed_frames = self.military_caption_speed_frames();
        let point_size = self.military_caption_point_size;
        let char_width = self.caption_char_width();
        let delay_frames = Self::military_caption_delay_frames();
        if Self::update_military_subtitle_state(
            &mut self.current_military_subtitle,
            self.current_frame,
            speed_frames,
            point_size,
            char_width,
            delay_frames,
        ) {
            Self::play_military_subtitle_typing_sound();
        }
        // C++ removeMilitarySubtitle (InGameUI.cpp:4099) clears tooltips on fade-out.
        if had_subtitle && self.current_military_subtitle.is_none() {
            self.clear_tooltips_disabled();
        }
    }

    fn update_military_subtitle_state(
        current_subtitle: &mut Option<MilitarySubtitle>,
        current_frame: u32,
        speed_frames: u32,
        point_size: i32,
        char_width: f32,
        delay_frames: u32,
    ) -> bool {
        let Some(subtitle) = current_subtitle.as_mut() else {
            return false;
        };

        if subtitle.lifetime_frame < current_frame {
            let alpha = (subtitle.color >> 24) as i32;
            let fade_amount = ((current_frame - subtitle.lifetime_frame) as f32 * 0.1) as i32;
            if alpha - fade_amount < 0 {
                *current_subtitle = None;
            } else {
                let new_alpha = (alpha - fade_amount) as u32;
                subtitle.color = (subtitle.color & 0x00FF_FFFF) | (new_alpha << 24);
            }
            return false;
        }

        if subtitle.block_begin_frame + 9 < current_frame {
            subtitle.block_begin_frame = current_frame;
            subtitle.block_drawn = !subtitle.block_drawn;
        }

        if subtitle.increment_on_frame >= current_frame {
            return false;
        }

        let Some(ch) = subtitle.text.chars().nth(subtitle.index) else {
            subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
            return false;
        };

        let mut typed_visible_char = false;
        if ch == '\n' {
            // C++ InGameUI.cpp:1707-1731 — advance line, cap at MAX_SUBTITLE_LINES.
            subtitle.block_pos.1 += point_size.max(1) as f32;
            subtitle.current_display_string = subtitle.current_display_string.saturating_add(1);
            if subtitle.current_display_string >= MAX_SUBTITLE_LINES {
                subtitle.index = subtitle.text.chars().count();
            } else {
                subtitle.block_pos.0 = subtitle.position.0;
                if subtitle.display_lines.len() <= subtitle.current_display_string {
                    subtitle.display_lines.push(String::new());
                }
                subtitle.block_drawn = true;
                subtitle.increment_on_frame = current_frame + delay_frames;
            }
        } else {
            if subtitle.display_lines.is_empty() {
                subtitle.display_lines.push(String::new());
                subtitle.current_display_string = 0;
            }
            let line = subtitle
                .current_display_string
                .min(subtitle.display_lines.len().saturating_sub(1));
            subtitle.display_lines[line].push(ch);
            let printed_chars_on_line = subtitle.display_lines[line].chars().count();
            subtitle.block_pos.0 =
                subtitle.position.0 + (printed_chars_on_line as f32 * char_width);
            subtitle.increment_on_frame = current_frame + speed_frames;
            typed_visible_char = true;
        }

        subtitle.index += 1;
        if subtitle.index >= subtitle.text.chars().count() {
            subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
        }
        typed_visible_char
    }


    fn play_military_subtitle_typing_sound() {
        if let Some(audio) = TheAudio::get() {
            let event = AudioEventRts::new("MilitarySubtitlesTyping");
            let _ = audio.add_audio_event(&event);
        }
    }

    fn caption_char_width(&self) -> f32 {
        self.military_caption_point_size.max(1) as f32 * 0.6
    }

    /// C++ InGameUI.cpp:3461-3483 postDraw — typed lines + blinking block.
    pub fn draw_military_subtitle(&self, renderer: &mut UIRenderer) {
        let Some(subtitle) = self.current_military_subtitle.as_ref() else {
            return;
        };
        let (r, g, b, a) = Self::unpack_argb(subtitle.color);
        let color = [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ];
        let drop = [0.0, 0.0, 0.0, a as f32 / 255.0];
        let font_size = self.military_caption_point_size.max(1) as f32;
        let mut y = subtitle.position.1;
        for line in subtitle.visible_lines() {
            let pos = Vec2::new(subtitle.position.0, y);
            let _ = renderer.draw_text_simple(line, pos + Vec2::new(1.0, 1.0), font_size, drop);
            let _ = renderer.draw_text_simple(line, pos, font_size, color);
            y += font_size;
        }
        if subtitle.block_drawn {
            let height = font_size;
            let width = height * 0.8;
            renderer.draw_rect(
                UIRect::new(subtitle.block_pos.0, subtitle.block_pos.1, width, height),
                color,
                0.0,
            );
        }
    }


    fn military_caption_speed_frames(&self) -> u32 {
        get_global_language_read()
            .map(|language| language.military_caption_speed.max(0) as u32)
            .unwrap_or_else(|| self.military_caption_speed.max(0) as u32)
    }

    // ── Popup message system ─────────────────────────────────────────────
    // C++: InGameUI::popupMessage() (InGameUI.cpp:5137)

    pub fn get_popup_message_color(&self) -> u32 {
        self.popup_message_color
    }

    // ── INI settings loading ─────────────────────────────────────────────
    // C++: InGameUI::init() loads Data\INI\InGameUI.ini via TheINIParser

}
