use openscq30_lib_macros::Has;

use crate::devices::soundcore::{
    a3953,
    common::structures::{DualBattery, DualFirmwareVersion, SerialNumber, TwsStatus},
};

#[derive(Debug, Clone, PartialEq, Eq, Has)]
pub struct A3953State {
    tws_status: TwsStatus,
    battery: DualBattery,
    dual_firmware_version: DualFirmwareVersion,
    serial_number: SerialNumber,
    sound_modes: a3953::structures::SoundModes,
}

impl From<a3953::packets::A3953StateUpdatePacket> for A3953State {
    fn from(packet: a3953::packets::A3953StateUpdatePacket) -> Self {
        let a3953::packets::A3953StateUpdatePacket {
            tws_status,
            battery,
            dual_firmware_version,
            serial_number,
            unknown_before_custom_length: _,
            custom_length: _,
            button_config: _,
            unknown_gap: _,
            ambient_sound_mode_cycle: _,
            sound_modes,
            unknown_suffix: _,
        } = packet;

        Self {
            tws_status,
            battery,
            dual_firmware_version,
            serial_number,
            sound_modes,
        }
    }
}
