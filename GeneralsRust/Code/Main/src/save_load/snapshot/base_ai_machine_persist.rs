//! Source-identified continuation of the actual receiving base AI machine.
//! The wire value is detached save data; mutable continuation stays on Object.
use crate::game_logic::object::BaseAiContinuation;
use crate::game_logic::{Object, ThingTemplate};
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseAiObjectSnapshot {
    source: Option<(u32, String, String)>,
    continuation: Option<BaseAiContinuation>,
}

fn bad(message: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("ReceivingBaseAI: {message}"))
}

impl BaseAiObjectSnapshot {
    pub(super) fn continuation(&self) -> Option<&BaseAiContinuation> {
        self.continuation.as_ref()
    }

    pub(crate) fn capture(object: &Object) -> SaveLoadResult<Self> {
        let source = object
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|template| template.receiving_ai_identity());
        if source.is_none() && !object.base_ai_control_is_inert() {
            return Err(bad("active control has no admitted AI source"));
        }
        let continuation = source
            .as_ref()
            .map(|_| object.capture_base_ai_continuation());
        let snapshot = Self {
            source,
            continuation,
        };
        snapshot.validate_for_template(&object.thing().template)?;
        Ok(snapshot)
    }

    /// Detached validation; callers run this before changing frame or objects.
    pub(crate) fn validate_for_template(&self, template: &ThingTemplate) -> SaveLoadResult<()> {
        let expected = template
            .receiving_client_template
            .as_ref()
            .and_then(|template| template.receiving_ai_identity());
        if self.source != expected || self.source.is_some() != self.continuation.is_some() {
            return Err(bad("source identity or continuation presence mismatch"));
        }
        if let Some(value) = &self.continuation {
            value.validate().map_err(bad)?;
        }
        Ok(())
    }

    /// The historical status scalar is a checked wire projection, never a
    /// restore producer. Shared detached preflight supplies that saved scalar.
    pub(crate) fn validate_ai_state_projection(
        &self,
        state: &crate::game_logic::AIState,
    ) -> SaveLoadResult<()> {
        match &self.continuation {
            Some(value) if &value.projected_state() == state => Ok(()),
            None if matches!(
                state,
                crate::game_logic::AIState::Idle | crate::game_logic::AIState::Uninitialized
            ) =>
            {
                Ok(())
            }
            _ => Err(bad("status AI state disagrees with native14 owner/source")),
        }
    }

    /// Apply after receiving admission; never dispatch a command or replay entry.
    pub(crate) fn apply_after_admission(&self, object: &mut Object) -> SaveLoadResult<()> {
        self.validate_for_template(&object.thing().template)?;
        if let Some(value) = &self.continuation {
            object
                .restore_base_ai_continuation(value.clone())
                .map_err(bad)?;
        } else if !object.base_ai_control_is_inert() {
            return Err(bad("source-less receiving object has active AI control"));
        }
        Ok(())
    }
}

impl XferData for BaseAiObjectSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut version = 3u8;
        xfer.xfer_u8(&mut version)?;
        if !(1..=3).contains(&version) {
            return Err(bad("unsupported module version"));
        }
        let mut present = self.source.is_some();
        xfer.xfer_bool(&mut present)?;
        if !present {
            if xfer.get_mode() == XferMode::Load {
                self.source = None;
                self.continuation = None;
            } else if self.continuation.is_some() {
                return Err(bad("owner continuation without source"));
            }
            return Ok(());
        }
        let (mut ordinal, mut tag, mut class) = self.source.clone().unwrap_or_default();
        xfer.xfer_u32(&mut ordinal)?;
        xfer.xfer_string(&mut tag)?;
        xfer.xfer_string(&mut class)?;
        let mut body = if xfer.get_mode() == XferMode::Load {
            String::new()
        } else {
            let value = self
                .continuation
                .as_ref()
                .ok_or_else(|| bad("missing owner continuation"))?;
            value.validate().map_err(bad)?;
            serde_json::to_string(value).map_err(bad)?
        };
        if version == 1 {
            xfer.xfer_string(&mut body)?;
        } else {
            super::xfer_helpers::xfer_ai_continuation_string(xfer, &mut body)?;
        }
        if xfer.get_mode() == XferMode::Load {
            let mut value: BaseAiContinuation = serde_json::from_str(&body).map_err(bad)?;
            if version < 3 {
                value.discard_legacy_guard_history();
            }
            value.validate().map_err(bad)?;
            // Commit only a completely decoded detached value.
            self.source = Some((ordinal, tag, class));
            self.continuation = Some(value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod owner_tests;
