use std::collections::HashMap;

use crate::devices::soundcore::{
    a3953::{packets::A3953StateUpdatePacket, state::A3953State},
    common::{
        device::fetch_state_from_state_update_packet,
        macros::soundcore_device,
        modules::{auto_power_off::AutoPowerOffDuration, equalizer::common_settings_type_2},
        packet::outbound::{RequestState, ToPacket},
    },
};

mod modules;
mod packets;
mod state;
pub mod structures;

// Battery, dual firmware version, serial number, ambient sound mode, wind noise suppression, wear
// detection, case battery level, LDAC, dual-connection support, auto power off, wearing tone (the
// app calls it "in ear beep"), low battery prompt, ambient sound prompt, spatial audio, press
// sensitivity, and custom equalizer have all been reverse-engineered against the official app's
// decompiled source (see packets::state_update, structures.rs,
// packets::set_equalizer_configuration). Button configuration is parsed, and each assignment's
// action ID is now decoded to a name (see structures::ButtonAction), but it's still not exposed as
// a setting: no outbound command that writes a button assignment back was found anywhere in the
// decompiled source, for this device or any other device family that shares the same
// `ControllerBtnModel` read path.
// "Bass up" (a device-family-wide EQ model, `BaseDefaultEqHasBassUpM`) has no confirmed link to
// A3953's own equalizer data (`A3953EqData`, `A3952EqM extends BaseM` directly, not the bass-up
// base class) and no located command, so it's left unimplemented. Hear ID (personalized hearing
// profile) is read and round-tripped through equalizer writes unchanged, but not exposed for
// editing, matching this project's own `a3947`/`a3955` devices, which share this exact command
// family and byte layout (down to the DRC coefficients).
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
        builder.low_battery_prompt();
        builder.a3953_misc_toggles();
        builder.a3953_spatial_audio();
        builder.a3953_press_sensitivity();
        builder.a3953_equalizer(common_settings_type_2()).await;
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
            structures::CommonVolumeAdjustments,
        },
        settings::{SettingId, Value},
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
            (SettingId::LowBatteryPrompt, true.into()),
            (SettingId::AmbientSoundPrompt, true.into()),
            (SettingId::SupportTwoConnections, false.into()),
            (SettingId::SpatialAudio, false.into()),
            (SettingId::SpatialAudioMode, "Music".into()),
            (SettingId::SpatialAudioMusicMode, "Fixed".into()),
            (SettingId::VolumeAdjustments, Value::I16Vec(vec![0; 8])),
        ]);
    }

    // Same real capture as `parses_known_packet`. Verifies the outbound custom EQ write packet
    // byte for byte: command `[0x03, 0x87]` (decompiled `Cmm2CmdData.p1`, reached via
    // `A3953CmdService.c5()` -> `CmmBtCmdService.z5`/`v5`), the plain custom EQ values, the
    // Hear-ID-uninitialized `255, 255` sentinel (this capture's own Hear ID data, round-tripped
    // unchanged from what `parses_known_packet` decodes: not enabled, all-`-60` stored values,
    // `time=0`, `hear_id_type=Initial`, `favorite_music_genre=0`), and the DRC-transformed copy of
    // the new custom EQ values as the trailing "real eq" block (see
    // `common::structures::VolumeAdjustments::apply_drc`, itself a verbatim transcription of the
    // decompiled app's `HearId2Utils.b`).
    #[tokio::test(start_paused = true)]
    async fn sets_custom_equalizer() {
        let mut device = TestSoundcoreDevice::new(
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

        // Bands 8 and 9 aren't settable through `SettingId::VolumeAdjustments` (only the 8 visible
        // bands are), so the equalizer module leaves them at 0 rather than the DRC transform's own
        // internal defaults for those two positions.
        let new_adjustments =
            CommonVolumeAdjustments::<10>::new([60, -60, 60, -60, 60, -60, 60, -60, 0, 0]);
        // Decoded from this capture's own Hear ID block by `parses_known_packet`'s sibling
        // assertions; round-tripped unchanged since this project doesn't expose Hear ID for
        // editing.
        let stored_hear_id_adjustments = CommonVolumeAdjustments::<10>::new([
            -60, -60, -60, -60, -60, -60, -60, -60, -120, -120,
        ]);

        let mut expected_body = Vec::new();
        expected_body.extend([254u8, 254u8]); // preset id: custom (0xFEFE)
        expected_body.extend([0u8, 0u8]); // favorite_music_genre (unchanged from the capture)
        expected_body.extend(new_adjustments.bytes());
        expected_body.extend(new_adjustments.bytes());
        expected_body.extend([255u8, 255u8]); // Hear ID not initialized in this capture
        expected_body.push(0); // hear_id.is_enabled: always disabled on write
        expected_body.extend(stored_hear_id_adjustments.bytes());
        expected_body.extend(stored_hear_id_adjustments.bytes());
        expected_body.extend(0u32.to_be_bytes()); // hear_id.time
        expected_body.push(0); // hear_id_type: Initial
        expected_body.extend(stored_hear_id_adjustments.bytes());
        expected_body.extend(stored_hear_id_adjustments.bytes());
        expected_body.extend(new_adjustments.apply_drc().bytes());
        expected_body.extend(new_adjustments.apply_drc().bytes());
        expected_body.push(0); // trailing byte

        device
            .assert_set_settings_response(
                vec![(
                    SettingId::VolumeAdjustments,
                    Value::I16Vec(vec![60, -60, 60, -60, 60, -60, 60, -60]),
                )],
                vec![packet::Outbound::new(
                    packet::Command([3, 135]),
                    expected_body,
                )],
            )
            .await;
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
