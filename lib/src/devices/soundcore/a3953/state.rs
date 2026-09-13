use openscq30_lib_macros::Has;

use crate::devices::soundcore::{
    a3953,
    common::structures::{
        AutoPowerOff, CaseBatteryLevel, CommonEqualizerConfiguration, CustomHearId, DualBattery,
        DualFirmwareVersion, Ldac, LowBatteryPrompt, SerialNumber, TwsStatus, WearingDetection,
        WearingTone,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Has)]
pub struct A3953State {
    tws_status: TwsStatus,
    battery: DualBattery,
    dual_firmware_version: DualFirmwareVersion,
    serial_number: SerialNumber,
    equalizer_configuration: CommonEqualizerConfiguration<2, 10>,
    is_hear_id_initialized: a3953::structures::IsHearIdInitialized,
    hear_id: CustomHearId<2, 10>,
    sound_modes: a3953::structures::SoundModes,
    wearing_detection: WearingDetection,
    case_battery_level: CaseBatteryLevel,
    ldac: Ldac,
    support_two_connections: a3953::structures::SupportTwoConnections,
    auto_power_off: AutoPowerOff,
    wearing_tone: WearingTone,
    low_battery_prompt: LowBatteryPrompt,
    ambient_sound_prompt: a3953::structures::AmbientSoundPrompt,
    spatial_audio: a3953::structures::SpatialAudio,
    press_sensitivity: a3953::structures::PressSensitivity,
}

impl From<a3953::packets::A3953StateUpdatePacket> for A3953State {
    fn from(packet: a3953::packets::A3953StateUpdatePacket) -> Self {
        let a3953::packets::A3953StateUpdatePacket {
            tws_status,
            battery,
            dual_firmware_version,
            serial_number,
            equalizer_configuration,
            is_hear_id_initialized,
            hear_id,
            custom_length: _,
            button_config: _,
            unknown_gap: _,
            ambient_sound_mode_cycle: _,
            sound_modes,
            wearing_detection,
            case_battery_level,
            ldac,
            support_two_connections,
            auto_power_off,
            wearing_tone,
            low_battery_prompt,
            ambient_sound_prompt,
            spatial_audio,
            device_colour: _,
            press_sensitivity,
            unknown_suffix: _,
        } = packet;

        Self {
            tws_status,
            battery,
            dual_firmware_version,
            serial_number,
            equalizer_configuration,
            is_hear_id_initialized,
            hear_id,
            sound_modes,
            wearing_detection,
            case_battery_level,
            ldac,
            support_two_connections,
            auto_power_off,
            wearing_tone,
            low_battery_prompt,
            ambient_sound_prompt,
            spatial_audio,
            press_sensitivity: press_sensitivity.unwrap_or_default(),
        }
    }
}
