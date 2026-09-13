use std::sync::Arc;

use openscq30_lib_has::Has;

use crate::{
    api::settings::{CategoryId, SettingId},
    devices::soundcore::{
        a3953::structures::{AmbientSoundPrompt, SupportTwoConnections},
        common::{
            modules::{ModuleCollection, flag::FlagConfiguration},
            packet::{self, PacketIOController},
        },
    },
};

impl<T> ModuleCollection<T>
where
    T: Has<AmbientSoundPrompt> + Has<SupportTwoConnections> + Send + Sync,
{
    pub fn add_a3953_misc_toggles(&mut self, packet_io: Arc<PacketIOController>) {
        self.add_flag::<AmbientSoundPrompt>(
            packet_io.clone(),
            FlagConfiguration {
                category_id: CategoryId::Miscellaneous,
                setting_id: SettingId::AmbientSoundPrompt,
                set_command: packet::Command([0x10, 0x83]),
                update_command: None,
                is_inverted: false,
            },
        );
        self.add_flag::<SupportTwoConnections>(
            packet_io,
            FlagConfiguration {
                category_id: CategoryId::Miscellaneous,
                setting_id: SettingId::SupportTwoConnections,
                set_command: packet::Command([0x0B, 0x84]),
                update_command: None,
                is_inverted: false,
            },
        );
    }
}
