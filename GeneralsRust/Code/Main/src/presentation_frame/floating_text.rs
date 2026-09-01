use super::*;

/// C++ `DEFAULT_FLOATING_TEXT_TIMEOUT = LOGICFRAMES_PER_SECOND / 3` → **10** frames.
pub const PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES: u32 = 10;
/// C++ `m_floatingTextMoveUpSpeed` default (world units per logic frame, draw residual).
pub const PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED: f32 = 1.0;
/// C++ `m_floatingTextMoveVanishRate` default (alpha decay residual after timeout).
pub const PRESENTATION_FLOATING_TEXT_VANISH_RATE: f32 = 0.1;
/// Host residual fade window after world-anim display time (seconds) when Fades=Yes.
///
/// Mirrors C++ WORLD_ANIM_FADE_ON_EXPIRE ~1s window. Fail-closed: not live GPU blend.
pub const PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS: f32 = 1.0;
/// Logic FPS residual for age → seconds conversion (presentation dual-tick).
pub const PRESENTATION_LOGIC_FPS: f32 = 30.0;

/// Source residual family for frozen floating cash / caption text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PresentationFloatingTextKind {
    /// AutoDepositUpdate (oil derrick / black market).
    AutoDeposit,
    /// HackInternet / Internet Center floating cash.
    Hacker,
    /// CashBounty kill bounty floating cash.
    CashBounty,
    /// MoneyCrateCollide pickup floating cash.
    MoneyCrate,
    /// Combat HP damage residual (from DamageApplied events).
    CombatDamage,
    /// Wave 514: Drawable emoticon residual (status bubble above unit).
    Emoticon,
}

/// Snapshot-owned InGameUI::addFloatingText residual for dual-tick consumers.
///
/// Built only from host residual registries at presentation build time so the
/// UI / GPU layout pack path does not re-read live GameLogic mid-render.
/// Fail-closed: not full DisplayString GPU draw / Unicode GameText localization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFloatingText {
    pub kind: PresentationFloatingTextKind,
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    /// Source object (derrick / hacker / killer / crate).
    pub source_id: ObjectId,
    /// Frame when residual times out (`spawn + PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES`).
    pub timeout_frame: u32,
}

impl PresentationFloatingText {
    pub fn from_parts(
        kind: PresentationFloatingTextKind,
        text: String,
        text_key: String,
        position: Vec3,
        color_rgba: (u8, u8, u8, u8),
        amount: u32,
        spawn_frame: u32,
        source_id: ObjectId,
    ) -> Self {
        Self {
            kind,
            text,
            text_key,
            position,
            color_rgba,
            amount,
            spawn_frame,
            source_id,
            timeout_frame: spawn_frame.saturating_add(PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES),
        }
    }

    /// True while the presentation residual still draws the entry.
    ///
    /// Visibility follows the linear vanish window: alpha stays 1.0 through
    /// `timeout_frame` and reaches 0 exactly `1/rate` frames later, so the
    /// entry drops out on that exact frame (spawn + timeout + 10 at the
    /// retail 0.1 vanish rate). Not the byte-cumulative C++ color tail.
    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        self.vanish_alpha_at(logic_frame) > 0.0
    }

    /// Age in logic frames at `logic_frame` (0 at spawn).
    pub fn age_frames_at(&self, logic_frame: u32) -> u32 {
        logic_frame.saturating_sub(self.spawn_frame)
    }

    /// C++ draw residual lift: `frameCount * m_floatingTextMoveUpSpeed`.
    pub fn lift_y_at(&self, logic_frame: u32) -> f32 {
        self.age_frames_at(logic_frame) as f32 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED
    }

    /// Vanish-rate alpha residual (1.0 while active; decays after timeout).
    ///
    /// C++: after timeout, alpha pulls toward 0 by `m_floatingTextMoveVanishRate`
    /// per frame until erased. Fail-closed: not live Display surface blend.
    pub fn vanish_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age < timeout {
            1.0
        } else {
            let past = (age - timeout) as f32;
            (1.0 - past * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0)
        }
    }

    /// C++ `updateFloatingText` integer alpha residual after timeout.
    ///
    /// ```text
    /// amount = REAL_TO_INT((currFrame - timeout) * m_floatingTextMoveVanishRate);
    /// if (a - amount < 0) a = 0; else a -= amount;
    /// ```
    /// Fail-closed: not live DisplayString surface blend / StretchRect.
    pub fn vanish_color_alpha_u8_at(&self, logic_frame: u32, base_alpha: u8) -> u8 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age <= timeout {
            return base_alpha;
        }
        let past = (age - timeout) as f32;
        // REAL_TO_INT truncates toward zero (C++ `(Int)(x)`).
        let amount = (past * PRESENTATION_FLOATING_TEXT_VANISH_RATE) as i32;
        let next = base_alpha as i32 - amount;
        if next < 0 { 0 } else { next as u8 }
    }

    /// Apply vanish-rate residual to a frozen color_rgba (RGB preserved, A decays).
    pub fn color_with_vanish_alpha_at(&self, logic_frame: u32) -> (u8, u8, u8, u8) {
        let (r, g, b, a) = self.color_rgba;
        (r, g, b, self.vanish_color_alpha_u8_at(logic_frame, a))
    }

    /// Honesty: retail vanish-rate / move-up / timeout presentation fields.
    pub fn honesty_vanish_rate_residual_ok() -> bool {
        (PRESENTATION_FLOATING_TEXT_VANISH_RATE - 0.1).abs() < 0.001
            && PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES == 10
            && (PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED - 1.0).abs() < 0.001
            && {
                let t = PresentationFloatingText::synthetic_cash(50, 0);
                (t.vanish_alpha_at(0) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(9) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(10) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(15) - 0.5).abs() < 0.001
                    && (t.vanish_alpha_at(20) - 0.0).abs() < 0.001
                    && (t.lift_y_at(5) - 5.0).abs() < 0.001
            }
    }

    /// Wave 76 residual honesty: C++ integer color-alpha vanish path residual.
    ///
    /// Matches `InGameUI::updateFloatingText` REAL_TO_INT amount subtract on A.
    /// With default vanish rate **0.1**, past=10 → amount **1** (255→254);
    /// past=5 → amount **0** (truncation). Fail-closed vs live Display surface.
    pub fn honesty_vanish_color_alpha_residual_ok() -> bool {
        let t = PresentationFloatingText::synthetic_cash(50, 0);
        // Synthetic cash uses green (0,255,0,255).
        t.color_rgba == (0, 255, 0, 255)
            && t.vanish_color_alpha_u8_at(0, 255) == 255
            && t.vanish_color_alpha_u8_at(10, 255) == 255
            && t.vanish_color_alpha_u8_at(15, 255) == 255 // past=5 → amount=0
            && t.vanish_color_alpha_u8_at(20, 255) == 254 // past=10 → amount=1
            && t.vanish_color_alpha_u8_at(30, 255) == 253 // past=20 → amount=2
            && t.vanish_color_alpha_u8_at(20, 1) == 0 // saturating subtract residual
            && {
                let c = t.color_with_vanish_alpha_at(20);
                c == (0, 255, 0, 254)
            }
            && Self::honesty_vanish_rate_residual_ok()
    }

    /// Synthetic cash residual for host-testable floating-text pack honesty.
    pub fn synthetic_cash(amount: u32, spawn_frame: u32) -> Self {
        Self::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            format!("+${amount}"),
            "GUI:AddCash".into(),
            Vec3::new(10.0, 20.0, 5.0),
            (0, 255, 0, 255),
            amount,
            spawn_frame,
            ObjectId(7001),
        )
    }
}

/// Snapshot-owned InGameUI::addWorldAnimation residual (MoneyPickUp Anim2D family).
///
/// Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationWorldAnim {
    pub template: String,
    pub position: Vec3,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub fades: bool,
    pub spawn_frame: u32,
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
}

impl PresentationWorldAnim {
    pub fn from_money_pickup(
        anim: &crate::game_logic::host_money_crate::HostMoneyPickUpAnim,
    ) -> Self {
        Self {
            template: anim.template.clone(),
            position: anim.position,
            display_time_seconds: anim.display_time_seconds,
            z_rise_per_second: anim.z_rise_per_second,
            fades: anim.fades,
            spawn_frame: anim.spawn_frame,
            crate_id: anim.crate_id,
            picker_id: anim.picker_id,
        }
    }

    /// Synthetic MoneyPickUp residual for host-testable world-anim pack honesty.
    pub fn synthetic_money_pickup(spawn_frame: u32) -> Self {
        Self {
            template: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE.to_string(),
            position: Vec3::new(12.0, 0.0, 8.0),
            display_time_seconds:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS,
            z_rise_per_second:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND,
            fades: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_FADES,
            spawn_frame,
            crate_id: ObjectId(8001),
            picker_id: ObjectId(8002),
        }
    }

    /// Display duration residual in logic frames (30 Hz).
    pub fn display_frames(&self) -> u32 {
        (self.display_time_seconds * PRESENTATION_LOGIC_FPS)
            .ceil()
            .max(1.0) as u32
    }

    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.spawn_frame.saturating_add(self.display_frames())
    }

    /// Age in seconds at `logic_frame` (0 at spawn).
    pub fn age_seconds_at(&self, logic_frame: u32) -> f32 {
        logic_frame.saturating_sub(self.spawn_frame) as f32 / PRESENTATION_LOGIC_FPS
    }

    /// WORLD_ANIM_FADE_ON_EXPIRE residual alpha at `logic_frame`.
    ///
    /// - age < display → 1.0
    /// - age ≥ display and fades → clamp(1 - past/fade_window, 0..1)
    /// - age ≥ display and !fades → 0.0
    pub fn fade_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_seconds_at(logic_frame);
        if age < self.display_time_seconds {
            1.0
        } else if self.fades {
            let past = age - self.display_time_seconds;
            (1.0 - past / PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Honesty: MoneyPickUp fade presentation residual fields.
    pub fn honesty_fade_residual_ok(&self) -> bool {
        (PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS - 1.0).abs() < 0.01
            && self.display_time_seconds > 0.0
            && {
                // Sample fade curve residual around display boundary.
                let mid = self
                    .spawn_frame
                    .saturating_add((self.display_time_seconds * PRESENTATION_LOGIC_FPS) as u32);
                let before = mid.saturating_sub(1);
                let half = mid.saturating_add((PRESENTATION_LOGIC_FPS * 0.5) as u32);
                let end = mid.saturating_add(PRESENTATION_LOGIC_FPS as u32);
                (self.fade_alpha_at(before) - 1.0).abs() < 0.05
                    && if self.fades {
                        (self.fade_alpha_at(half) - 0.5).abs() < 0.1
                            && (self.fade_alpha_at(end) - 0.0).abs() < 0.05
                    } else {
                        self.fade_alpha_at(half) <= 0.0
                    }
            }
    }

    /// Static honesty for retail MoneyPickUp fade residual defaults.
    pub fn honesty_money_pickup_fade_params_ok() -> bool {
        let a = Self::synthetic_money_pickup(0);
        a.fades
            && (a.display_time_seconds - 4.0).abs() < 0.01
            && (a.z_rise_per_second - 15.0).abs() < 0.01
            && a.honesty_fade_residual_ok()
    }
}

/// Collect host residual floating texts into a stable presentation list.
pub(crate) fn collect_presentation_floating_texts(
    logic: &GameLogic,
) -> Vec<PresentationFloatingText> {
    let mut out = Vec::new();

    for t in &logic.oil_derricks().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.black_markets().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.hacker_income().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::Hacker,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.hacker_id,
        ));
    }
    for t in &logic.cash_bounty_registry().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::CashBounty,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.killer_id,
        ));
    }
    for t in &logic.host_money_crates().money_floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.crate_id,
        ));
    }

    // Stable presentation order: spawn frame then source id then kind.
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.source_id.0.cmp(&b.source_id.0))
            .then(a.kind.cmp(&b.kind))
    });
    out
}

pub(crate) fn collect_presentation_world_anims(logic: &GameLogic) -> Vec<PresentationWorldAnim> {
    let mut out: Vec<PresentationWorldAnim> = logic
        .host_money_crates()
        .money_pickup_anims
        .iter()
        .map(PresentationWorldAnim::from_money_pickup)
        .collect();
    for anim in crate::game_logic::host_unit_training::promote_anims_snapshot() {
        let spawn_frame = if anim.spawn_frame == 0 {
            logic.frame
        } else {
            anim.spawn_frame
        };
        out.push(PresentationWorldAnim {
            template: crate::game_logic::host_unit_training::LEVEL_GAIN_ANIM_TEMPLATE.to_string(),
            position: anim.position,
            display_time_seconds:
                crate::game_logic::host_unit_training::LEVEL_GAIN_ANIM_DISPLAY_TIME_SECONDS,
            z_rise_per_second:
                crate::game_logic::host_unit_training::LEVEL_GAIN_ANIM_Z_RISE_PER_SECOND,
            fades: true,
            spawn_frame,
            crate_id: anim.object,
            picker_id: anim.object,
        });
    }
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.crate_id.0.cmp(&b.crate_id.0))
            .then(a.picker_id.0.cmp(&b.picker_id.0))
    });
    out
}


#[cfg(test)]
mod vanish_window_tests {
    use super::*;

    #[test]
    fn vanish_window_ends_when_fade_alpha_hits_zero() {
        let t = PresentationFloatingText::synthetic_cash(50, 0);
        // timeout=10. Linear vanish_alpha_at stays >0 through frame 19 and
        // reaches 0 exactly on frame 20; visibility flips on that frame.
        assert!(t.is_active_at(0));
        assert!(t.is_active_at(10));
        assert!(t.is_active_at(19));
        assert!(!t.is_active_at(20));
        assert!(!t.is_active_at(200));
    }
}
