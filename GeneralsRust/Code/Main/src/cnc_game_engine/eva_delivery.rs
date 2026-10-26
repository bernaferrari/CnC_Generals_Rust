//! Match-owned EVA delivery into native speech and HUD feedback.
use super::*;
use crate::game_logic::host_eva_log::{
    EvaDelivery, EvaOperation, EvaReceipt, eva_event_table_token,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum EvaAdmission {
    Repeated,
    Continued,
    NewSession,
}

/// Receipt ownership is independent of render frequency and logic frame resets.
/// The Arc in EvaReceipt is immutable identity; all mutable speech state is here.
#[derive(Debug)]
pub(super) struct EvaRecipient {
    last_receipt: Option<EvaReceipt>,
    #[cfg(feature = "game_client")]
    pub(super) speaker: game_client::eva::Eva,
    #[cfg(not(feature = "game_client"))]
    enabled: bool,
}
impl EvaRecipient {
    pub(super) fn from_startup_definitions() -> Self {
        Self {
            last_receipt: None,
            #[cfg(feature = "game_client")]
            speaker: game_client::eva::Eva::from_startup_definitions(),
            #[cfg(not(feature = "game_client"))]
            enabled: true,
        }
    }
    pub(super) fn is_enabled(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            self.speaker.is_enabled()
        }
        #[cfg(not(feature = "game_client"))]
        {
            self.enabled
        }
    }
    pub(super) fn admit(&mut self, delivery: &EvaDelivery) -> EvaAdmission {
        let same_session = self
            .last_receipt
            .as_ref()
            .is_some_and(|last| last.same_session(&delivery.receipt));
        if same_session
            && self
                .last_receipt
                .as_ref()
                .is_some_and(|last| delivery.receipt.cursor <= last.cursor)
        {
            return EvaAdmission::Repeated;
        }
        if !same_session {
            #[cfg(feature = "game_client")]
            {
                self.speaker.reset();
                self.speaker.set_enabled(delivery.enabled);
            }
            #[cfg(not(feature = "game_client"))]
            {
                self.enabled = delivery.enabled;
            }
        }
        for operation in &delivery.operations {
            match operation {
                EvaOperation::Request(event) => {
                    #[cfg(feature = "game_client")]
                    if let Some(message) =
                        game_client::eva::EvaMessage::from_name(eva_event_table_token(*event))
                    {
                        self.speaker.set_should_play(message);
                    }
                }
                EvaOperation::SetEnabled(enabled) => {
                    #[cfg(feature = "game_client")]
                    self.speaker.set_enabled(*enabled);
                    #[cfg(not(feature = "game_client"))]
                    {
                        self.enabled = *enabled;
                    }
                }
            }
        }
        self.last_receipt = Some(delivery.receipt.clone());
        if same_session {
            EvaAdmission::Continued
        } else {
            EvaAdmission::NewSession
        }
    }
}

/// Counter-only diagnostics cannot resurrect requests canceled by an ordered
/// SetEnabled operation. Observe all intents, even those absent from HUD output.
fn absorb_request_counters(
    delivery: &EvaDelivery,
    mut seen: [u32; 4],
    current: [u32; 4],
) -> [u32; 4] {
    use gamelogic::helpers::EvaEvent;
    for operation in &delivery.operations {
        let EvaOperation::Request(event) = operation else {
            continue;
        };
        let index = match event {
            EvaEvent::LowPower => 0,
            EvaEvent::InsufficientFunds => 1,
            EvaEvent::BaseUnderAttack => 2,
            EvaEvent::AllyUnderAttack => 3,
            _ => continue,
        };
        seen[index] = seen[index].max(current[index]);
    }
    seen
}

impl CnCGameEngine {
    pub(super) fn apply_presentation_eva_alerts(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        match self.eva_recipient.admit(&pres.eva_delivery) {
            EvaAdmission::Repeated => return,
            EvaAdmission::NewSession => {
                self.last_eva_low_power_count = 0;
                self.last_eva_insufficient_funds_count = 0;
                self.last_eva_base_under_attack_count = 0;
                self.last_eva_ally_under_attack_count = 0;
            }
            EvaAdmission::Continued => {}
        }
        [
            self.last_eva_low_power_count,
            self.last_eva_insufficient_funds_count,
            self.last_eva_base_under_attack_count,
            self.last_eva_ally_under_attack_count,
        ] = absorb_request_counters(
            &pres.eva_delivery,
            [
                self.last_eva_low_power_count,
                self.last_eva_insufficient_funds_count,
                self.last_eva_base_under_attack_count,
                self.last_eva_ally_under_attack_count,
            ],
            [
                pres.eva_low_power_count,
                pres.eva_insufficient_funds_count,
                pres.eva_base_under_attack_count,
                pres.eva_ally_under_attack_count,
            ],
        );
        for ev in &pres.events {
            let crate::presentation_frame::PresentationEvent::EvaAlert { name } = ev else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            match Self::eva_alert_client_token(name).as_str() {
                "LOWPOWER" => {
                    self.last_eva_low_power_count =
                        self.last_eva_low_power_count.max(pres.eva_low_power_count)
                }
                "INSUFFICIENTFUNDS" => {
                    self.last_eva_insufficient_funds_count = self
                        .last_eva_insufficient_funds_count
                        .max(pres.eva_insufficient_funds_count)
                }
                "BASEUNDERATTACK" => {
                    self.last_eva_base_under_attack_count = self
                        .last_eva_base_under_attack_count
                        .max(pres.eva_base_under_attack_count)
                }
                "ALLYUNDERATTACK" => {
                    self.last_eva_ally_under_attack_count = self
                        .last_eva_ally_under_attack_count
                        .max(pres.eva_ally_under_attack_count)
                }
                _ => {}
            }
            if self.eva_recipient.is_enabled() {
                let human = Self::eva_alert_human_message(name);
                self.chat_panel.add_eva_message(&human);
                self.host_push_info_message(&human);
            }
        }
        // Admission does not advance speech: host_update_the_audio owns that
        // phase. HUD/render may apply the same completed snapshot repeatedly.
    }
    /// `EVA_LOWPOWER` / table token → chat line residual.
    pub(super) fn eva_alert_human_message(name: &str) -> String {
        let key = name
            .strip_prefix("EVA_")
            .unwrap_or(name)
            .trim()
            .to_ascii_uppercase();
        match key.as_str() {
            "LOWPOWER" => "Warning: low power".into(),
            "INSUFFICIENTFUNDS" => "Insufficient funds".into(),
            "BASEUNDERATTACK" => "Our base is under attack".into(),
            "ALLYUNDERATTACK" => "Ally under attack".into(),
            "BUILDINGLOST" => "Building lost".into(),
            "UNITLOST" => "Unit lost".into(),
            "BEACONDETECTED" => "Beacon detected".into(),
            "UPGRADECOMPLETE" => "Upgrade complete".into(),
            "GENERALLEVELUP" => "General level up".into(),
            "VEHICLESTOLEN" => "Vehicle stolen".into(),
            "BUILDINGSTOLEN" => "Building stolen".into(),
            "BUILDINGBEINGSTOLEN" => "Building being stolen".into(),
            "BUILDINGSABOTAGED" => "Building sabotaged".into(),
            "CASHSTOLEN" => "Cash stolen".into(),
            other => format!("EVA: {}", other.replace('_', " ").to_ascii_lowercase()),
        }
    }

    /// `EVA_LOWPOWER` → `LOWPOWER` for `EvaMessage::from_name` (C++ table token).
    pub(super) fn eva_alert_client_token(name: &str) -> String {
        name.strip_prefix("EVA_")
            .unwrap_or(name)
            .trim()
            .to_ascii_uppercase()
    }

    pub(super) fn sync_eva_messages_from_host_counts(
        &mut self,
        low_power: u32,
        funds: u32,
        base: u32,
        ally: u32,
    ) {
        // C++ Eva::update early-returns when disabled (Eva.cpp:264-267).
        if !self.eva_recipient.is_enabled() {
            self.last_eva_low_power_count = self.last_eva_low_power_count.max(low_power);
            self.last_eva_insufficient_funds_count =
                self.last_eva_insufficient_funds_count.max(funds);
            self.last_eva_base_under_attack_count = self.last_eva_base_under_attack_count.max(base);
            self.last_eva_ally_under_attack_count = self.last_eva_ally_under_attack_count.max(ally);
            return;
        }
        let chat_panel = &mut self.chat_panel;
        let hud = self.ui_manager.game_hud_mut();
        #[cfg(feature = "game_client")]
        let mut native_ui = self.game_client.in_game_ui_subsystem_mut();
        let logic_frame = self.game_logic.get_frame();
        let mut push = |msg: &str| {
            chat_panel.add_eva_message(msg);
            hud.push_info_message(msg);
            #[cfg(feature = "game_client")]
            if let Some(ui) = native_ui.as_deref_mut() {
                ui.push_hud_message_at(msg.to_owned(), logic_frame);
            }
        };
        if low_power > self.last_eva_low_power_count {
            self.last_eva_low_power_count = low_power;
            push("Warning: low power");
        }
        if funds > self.last_eva_insufficient_funds_count {
            self.last_eva_insufficient_funds_count = funds;
            push("Insufficient funds");
        }
        if base > self.last_eva_base_under_attack_count {
            self.last_eva_base_under_attack_count = base;
            push("Our base is under attack");
        }
        if ally > self.last_eva_ally_under_attack_count {
            self.last_eva_ally_under_attack_count = ally;
            push("Ally under attack");
        }
    }
}

#[cfg(test)]
#[path = "eva_recipient_tests.rs"]
mod eva_recipient_tests;
