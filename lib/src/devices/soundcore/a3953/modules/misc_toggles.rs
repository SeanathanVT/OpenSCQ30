use std::sync::Arc;

use async_trait::async_trait;
use openscq30_lib_has::Has;
use strum::{EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use tokio::sync::watch;

use crate::{
    api::{
        device,
        settings::{CategoryId, Setting, SettingId, Value},
    },
    devices::soundcore::{
        a3953::structures::{AmbientSoundPrompt, SupportTwoConnections},
        common::{
            modules::ModuleCollection,
            packet::{self, PacketIOController},
            settings_manager::{SettingHandler, SettingHandlerResult},
            state_modifier::StateModifier,
        },
    },
    macros::enum_subset,
};

const AMBIENT_SOUND_PROMPT_COMMAND: packet::Command = packet::Command([0x10, 0x83]);
const SUPPORT_TWO_CONNECTIONS_COMMAND: packet::Command = packet::Command([0x0B, 0x84]);

enum_subset! {
    SettingId,
    #[derive(EnumString, EnumIter, IntoStaticStr)]
    enum MiscToggleSetting {
        AmbientSoundPrompt,
        SupportTwoConnections,
    }
}

impl<T> ModuleCollection<T>
where
    T: Has<AmbientSoundPrompt> + Has<SupportTwoConnections> + Clone + Send + Sync,
{
    pub fn add_a3953_misc_toggles(&mut self, packet_io: Arc<PacketIOController>) {
        self.setting_manager
            .add_handler(CategoryId::Miscellaneous, MiscToggleSettingHandler);
        self.state_modifiers
            .push(Box::new(AmbientSoundPromptStateModifier::new(
                packet_io.clone(),
            )));
        self.state_modifiers
            .push(Box::new(SupportTwoConnectionsStateModifier::new(packet_io)));
    }
}

#[derive(Default)]
struct MiscToggleSettingHandler;

#[async_trait]
impl<T> SettingHandler<T> for MiscToggleSettingHandler
where
    T: Has<AmbientSoundPrompt> + Has<SupportTwoConnections> + Send,
{
    fn settings(&self) -> Vec<SettingId> {
        MiscToggleSetting::iter().map(Into::into).collect()
    }

    fn get(&self, state: &T, setting_id: &SettingId) -> Option<Setting> {
        let setting: MiscToggleSetting = (*setting_id).try_into().ok()?;
        Some(match setting {
            MiscToggleSetting::AmbientSoundPrompt => {
                let value: &AmbientSoundPrompt = state.get();
                Setting::Toggle { value: value.0 }
            }
            MiscToggleSetting::SupportTwoConnections => {
                let value: &SupportTwoConnections = state.get();
                Setting::Toggle { value: value.0 }
            }
        })
    }

    async fn set(
        &self,
        state: &mut T,
        setting_id: &SettingId,
        value: Value,
    ) -> SettingHandlerResult<()> {
        let setting: MiscToggleSetting = (*setting_id)
            .try_into()
            .expect("already filtered to valid values only by SettingsManager");
        match setting {
            MiscToggleSetting::AmbientSoundPrompt => {
                let target: &mut AmbientSoundPrompt = state.get_mut();
                target.0 = value.try_as_bool()?;
            }
            MiscToggleSetting::SupportTwoConnections => {
                let target: &mut SupportTwoConnections = state.get_mut();
                target.0 = value.try_as_bool()?;
            }
        }
        Ok(())
    }
}

struct AmbientSoundPromptStateModifier {
    packet_io: Arc<PacketIOController>,
}

impl AmbientSoundPromptStateModifier {
    fn new(packet_io: Arc<PacketIOController>) -> Self {
        Self { packet_io }
    }
}

#[async_trait]
impl<T> StateModifier<T> for AmbientSoundPromptStateModifier
where
    T: Has<AmbientSoundPrompt> + Clone + Send + Sync,
{
    async fn move_to_state(
        &self,
        state_sender: &watch::Sender<T>,
        target_state: &T,
    ) -> device::Result<()> {
        let target: AmbientSoundPrompt = *target_state.get();
        {
            let state = state_sender.borrow();
            let current: &AmbientSoundPrompt = state.get();
            if *current == target {
                return Ok(());
            }
        }
        self.packet_io
            .send_with_response(&packet::Outbound::new(
                AMBIENT_SOUND_PROMPT_COMMAND,
                target.bytes().to_vec(),
            ))
            .await?;
        state_sender.send_modify(|state| {
            let value: &mut AmbientSoundPrompt = state.get_mut();
            *value = target;
        });
        Ok(())
    }
}

struct SupportTwoConnectionsStateModifier {
    packet_io: Arc<PacketIOController>,
}

impl SupportTwoConnectionsStateModifier {
    fn new(packet_io: Arc<PacketIOController>) -> Self {
        Self { packet_io }
    }
}

#[async_trait]
impl<T> StateModifier<T> for SupportTwoConnectionsStateModifier
where
    T: Has<SupportTwoConnections> + Clone + Send + Sync,
{
    async fn move_to_state(
        &self,
        state_sender: &watch::Sender<T>,
        target_state: &T,
    ) -> device::Result<()> {
        let target: SupportTwoConnections = *target_state.get();
        {
            let state = state_sender.borrow();
            let current: &SupportTwoConnections = state.get();
            if *current == target {
                return Ok(());
            }
        }
        self.packet_io
            .send_with_response(&packet::Outbound::new(
                SUPPORT_TWO_CONNECTIONS_COMMAND,
                target.bytes().to_vec(),
            ))
            .await?;
        state_sender.send_modify(|state| {
            let value: &mut SupportTwoConnections = state.get_mut();
            *value = target;
        });
        Ok(())
    }
}
