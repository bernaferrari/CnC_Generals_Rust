//! The native input and audio update phases take turns borrowing one stream.
use super::menu_audio::NativeMenuAudio;
pub(super) type NativeInputWithMenuAudio<'a> = dyn FnMut(
        &mut dyn game_client::gui::menu_audio::MenuAudioEffects,
        &game_client::gui::window_manager::WindowManagerOwner,
        &mut game_client::gui::campaign_manager::CampaignManager,
        &mut game_client::gui::shell::main_menu::MainMenu,
        &mut game_client::gui::campaign_launch_host_bridge::MenuHostMailbox,
        &mut game_client::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'_>,
        &mut game_client::core::script_action_handler::ScriptDisplayReceiver,
        &mut game_client::gui::window_video_manager::ClientWindowVideos,
        &mut game_client::gui::ime_manager::ImeManager,
        &mut game_client::core::subsystems::InGameUISubsystem,
        Option<&mut game_client::input::Mouse>,
        game_engine::common::message_stream::WindowInputDispatch,
        &mut Vec<game_client::message_stream::game_message::GameMessage>,
    ) -> Option<game_engine::common::message_stream::WindowInputDispatchResult>
    + 'a;

pub(super) struct NativeTranslationPhases<'a, 'context> {
    pub(super) dispatch: &'a mut NativeInputWithMenuAudio<'context>,
    pub(super) audio: NativeMenuAudio<'a>,
    pub(super) facts: game_client::gui::campaign_launch_host_bridge::MenuWorldFacts,
    pub(super) playback: bool,
    pub(super) pick_objects: game_engine::common::message_stream::TranslatorPickObjectView,
    pub(super) eva_speaker: &'a mut game_client::eva::Eva,
}
impl crate::subsystem_manager::ReceivingClientPhaseDriver for NativeTranslationPhases<'_, '_> {
    fn propagate(
        &mut self,
        client: &mut game_client::core::GameClient,
        cursor: Option<game_engine::common::message_stream::PropagationCursor>,
    ) -> anyhow::Result<game_engine::common::message_stream::PropagationProgress> {
        let dispatch = &mut *self.dispatch;
        let audio = &mut self.audio;
        let mut driver =
            |owner: &game_client::gui::window_manager::WindowManagerOwner,
             campaign: &mut game_client::gui::campaign_manager::CampaignManager,
             main_menu: &mut game_client::gui::shell::main_menu::MainMenu,
             menu_host: &mut game_client::gui::campaign_launch_host_bridge::MenuHostMailbox,
             menus: &mut game_client::gui::callbacks::menu_callbacks::MenuCallbackRecipients<
                '_,
            >,
             display: &mut game_client::core::script_action_handler::ScriptDisplayReceiver,
             videos: &mut game_client::gui::window_video_manager::ClientWindowVideos,
             ime: &mut game_client::gui::ime_manager::ImeManager,
             ui: &mut game_client::core::subsystems::InGameUISubsystem,
             mouse: Option<&mut game_client::input::Mouse>,
             request,
             messages: &mut Vec<game_engine::common::message_stream::GameMessage>| {
                dispatch(
                    audio, owner, campaign, main_menu, menu_host, menus, display, videos, ime, ui,
                    mouse, request, messages,
                )
            };
        client
            .take_message_stream_until_menu_host_with_receiving_eva(
                self.playback,
                self.facts.logic_frame,
                self.facts.game_mode,
                &mut driver,
                cursor,
                Some(&self.pick_objects),
                Some(
                    self.eva_speaker
                        as &mut dyn game_engine::common::message_stream::ReceivingEvaFeedback,
                ),
            )
            .map_err(|error| anyhow::Error::msg(error.to_string()))
    }
    fn update_audio(
        &mut self,
        audio: &mut crate::subsystem_manager::AudioManagerSubsystem,
        timing: Option<&ww3d_engine::FrameTiming>,
    ) -> anyhow::Result<()> {
        audio.update_receiving_audio(timing, self.audio.random_mut())
    }
}
