use std::collections::HashMap;

use crate::devices::soundcore::{
    a3953::{packets::A3953StateUpdatePacket, state::A3953State},
    common::{
        device::fetch_state_from_state_update_packet,
        macros::soundcore_device,
        modules::auto_power_off::AutoPowerOffDuration,
        packet::outbound::{RequestState, ToPacket},
    },
};

mod modules;
mod packets;
mod state;
pub mod structures;

// Battery, dual firmware version, serial number, ambient sound mode, wind noise suppression, wear
// detection, case battery level, LDAC, auto power off, wearing tone (the app calls it "in ear
// beep"), and press sensitivity have all been reverse-engineered against the official app's
// decompiled source (see packets::state_update, structures.rs). Button configuration is parsed
// (see structures::ButtonConfig) but not exposed: no set command or action-ID meaning was found.
// Equalizer and the remaining flags (bass up, dual connection, low battery alert, ambient sound
// prompt, spatial audio) are parsed but not yet exposed either, for the same reason (see
// packets::state_update for the full field list).
soundcore_device!(
    A3953State,
    async |packet_io| {
        fetch_state_from_state_update_packet::<A3953State, A3953StateUpdatePacket>(packet_io).await
    },
    async |builder| {
        builder.module_collection().add_state_update();

        builder.serial_number_and_dual_firmware_version();
        // Only ever observed at 5/5 (fully charged); assumed max_level 5 to match the structurally
        // similar A3947 (Liberty 4 NC) until a partial-charge capture confirms or corrects this.
        builder.dual_battery(5);
        builder.a3953_sound_modes();
        builder.wearing_detection();
        // Same caveat as dual_battery above: the device's own S() clamp permits 0-9, but no
        // partial-charge capture exists to confirm whether 5 is really this device's max.
        builder.case_battery_level(5);
        builder.ldac();
        builder.auto_power_off(AutoPowerOffDuration::ten_twenty_thirty_sixty());
        builder.wearing_tone();
        builder.a3953_press_sensitivity();
    },
    {
        HashMap::from([(
            RequestState::COMMAND,
            A3953StateUpdatePacket::default().to_packet(),
        )])
    },
);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        DeviceModel,
        devices::soundcore::common::{
            device::{SoundcoreDeviceConfig, test_utils::TestSoundcoreDevice},
            packet,
        },
        settings::SettingId,
    };

    #[tokio::test(start_paused = true)]
    async fn parses_known_packet() {
        let device = TestSoundcoreDevice::new(
            super::device_registry,
            DeviceModel::SoundcoreA3953,
            HashMap::from([(
                packet::Command([1, 1]),
                packet::Inbound::new(
                    packet::Command([1, 1]),
                    vec![
                        1, 1, 5, 5, 0, 0, 48, 51, 46, 50, 51, 48, 51, 46, 50, 51, 51, 57, 53, 51,
                        53, 52, 67, 54, 51, 51, 67, 67, 69, 69, 69, 56, 0, 0, 120, 120, 120, 120,
                        120, 120, 120, 120, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60, 60, 60, 60, 60,
                        60, 0, 0, 0, 0, 0, 0, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60,
                        60, 60, 60, 60, 60, 0, 0, 0, 0, 18, 17, 102, 17, 102, 17, 52, 17, 52, 17,
                        36, 17, 36, 17, 82, 17, 83, 3, 2, 48, 0, 0, 1, 1, 0, 0, 0, 0, 255, 255, 1,
                        1, 0, 5, 0, 0, 0, 1, 2, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 120, 255, 50, 0, 255,
                        255,
                    ],
                ),
            )]),
            SoundcoreDeviceConfig::default(),
        )
        .await;

        device.assert_setting_values([
            (SettingId::BatteryLevelLeft, "5/5".into()),
            (SettingId::BatteryLevelRight, "5/5".into()),
            (SettingId::IsChargingLeft, "No".into()),
            (SettingId::IsChargingRight, "No".into()),
            (SettingId::FirmwareVersionLeft, "03.23".into()),
            (SettingId::FirmwareVersionRight, "03.23".into()),
            (SettingId::SerialNumber, "395354C633CCEEE8".into()),
            (SettingId::AmbientSoundMode, "Normal".into()),
            (SettingId::WindNoiseSuppression, true.into()),
            (SettingId::WearingDetection, true.into()),
            (SettingId::CaseBatteryLevel, "5/5".into()),
            (SettingId::Ldac, false.into()),
            (SettingId::AutoPowerOff, "30m".into()),
            (SettingId::WearingTone, true.into()),
            (SettingId::PressSensitivity, 0.into()),
        ]);
    }

    // The three tests below are real captures of the same device, differing only in byte 121 (the
    // ambient sound mode toggled in the official app between captures): 2 (Normal), 0 (Noise
    // Canceling), and 1 (Transparency). See a3953/structures.rs for the byte citation.
    #[tokio::test(start_paused = true)]
    async fn parses_noise_canceling_mode() {
        let device = TestSoundcoreDevice::new(
            super::device_registry,
            DeviceModel::SoundcoreA3953,
            HashMap::from([(
                packet::Command([1, 1]),
                packet::Inbound::new(
                    packet::Command([1, 1]),
                    vec![
                        1, 1, 5, 5, 0, 0, 48, 51, 46, 50, 51, 48, 51, 46, 50, 51, 51, 57, 53, 51,
                        53, 52, 67, 54, 51, 51, 67, 67, 69, 69, 69, 56, 0, 0, 120, 120, 120, 120,
                        120, 120, 120, 120, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60, 60, 60, 60, 60,
                        60, 0, 0, 0, 0, 0, 0, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60,
                        60, 60, 60, 60, 60, 0, 0, 0, 0, 18, 17, 102, 17, 102, 17, 52, 17, 52, 17,
                        36, 17, 36, 17, 82, 17, 83, 3, 0, 48, 0, 0, 1, 1, 0, 0, 0, 0, 255, 255, 1,
                        1, 0, 5, 0, 0, 0, 1, 2, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 120, 255, 50, 0, 255,
                        255,
                    ],
                ),
            )]),
            SoundcoreDeviceConfig::default(),
        )
        .await;

        device.assert_setting_values([(SettingId::AmbientSoundMode, "NoiseCanceling".into())]);
    }

    #[tokio::test(start_paused = true)]
    async fn parses_transparency_mode() {
        let device = TestSoundcoreDevice::new(
            super::device_registry,
            DeviceModel::SoundcoreA3953,
            HashMap::from([(
                packet::Command([1, 1]),
                packet::Inbound::new(
                    packet::Command([1, 1]),
                    vec![
                        1, 1, 5, 5, 0, 0, 48, 51, 46, 50, 51, 48, 51, 46, 50, 51, 51, 57, 53, 51,
                        53, 52, 67, 54, 51, 51, 67, 67, 69, 69, 69, 56, 0, 0, 120, 120, 120, 120,
                        120, 120, 120, 120, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60, 60, 60, 60, 60,
                        60, 0, 0, 0, 0, 0, 0, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60,
                        60, 60, 60, 60, 60, 0, 0, 0, 0, 18, 17, 102, 17, 102, 17, 52, 17, 52, 17,
                        36, 17, 36, 17, 82, 17, 83, 3, 1, 48, 0, 0, 1, 1, 0, 0, 0, 0, 255, 255, 1,
                        1, 0, 5, 0, 0, 0, 1, 2, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 120, 255, 50, 0, 255,
                        255,
                    ],
                ),
            )]),
            SoundcoreDeviceConfig::default(),
        )
        .await;

        device.assert_setting_values([(SettingId::AmbientSoundMode, "Transparency".into())]);
    }
}
