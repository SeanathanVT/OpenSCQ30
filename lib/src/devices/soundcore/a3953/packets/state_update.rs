use std::iter;

use nom::{
    IResult, Parser,
    bytes::complete::take,
    combinator::{map, rest},
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::{
    a3953::{self, state::A3953State},
    common::{
        macros::state_update_packet_module,
        packet::{self, inbound::FromPacketBody, outbound::ToPacket},
        structures::{
            AmbientSoundModeCycle, AutoPowerOff, CaseBatteryLevel, DualBattery,
            DualFirmwareVersion, Ldac, SerialNumber, TwsStatus, WearingDetection, WearingTone,
        },
    },
};

/// Offsets below are reverse-engineered from the official Soundcore Android app
/// (com.oceanwing.soundcore v6.4.0-17), decompiling
/// `com.oceanwing.devicecmd.manager.product.a3953.A3953AnalysisService.R0`, which parses this same
/// packet body (there, `bArr[9..]`; here, `input[0..]`, i.e. every cited `bArr` index there is this
/// packet's index minus 9).
///
/// - `unknown_before_custom_length` (`bArr[41..112]`, 71 bytes): read by `R0` for EQ index/type,
///   custom EQ values, and Hear ID data, none of which are implemented here yet.
/// - `custom_length` (`bArr[112]`, `R0`'s local variable `m6`, logged as `"customLength"`): used to
///   compute every following offset as `custom_length + N`. In the one real capture this is built
///   from, its value is 18.
/// - `button_config` (`bArr[113..129]`, 16 bytes): see `a3953::structures::ButtonConfig` for the
///   per-byte citation. Parsed for round-trip fidelity but not exposed as a setting: no outbound
///   command that writes it back was found, and no action-ID-to-behavior mapping was found either.
/// - `unknown_gap`: `R0` computes the next two fields' positions as `custom_length + 111` and
///   `custom_length + 112` (`bArr` indices). Converted to indices into this packet body (`- 9`) and
///   relative to the end of `button_config` (`bArr[129]`, i.e. index 120 here), that leaves a gap of
///   `custom_length - 18` unknown bytes whenever `custom_length != 18`; zero-length in the captures
///   seen so far.
/// - `ambient_sound_mode_cycle` (`bArr[custom_length + 111]`): bits read via `BytesUtil.J(b, 1..3)`
///   into `cmm2BtDeviceInfo.setAncSelected/TransSelected/NormalSelected`, matching this project's
///   existing `AmbientSoundModeCycle` bit layout exactly (bit0=noise_canceling, bit1=transparency,
///   bit2=normal). No outbound command that sets these bits was found anywhere in the decompiled
///   `CmmBtCmdService`/`A3953CmdService`, so this is parsed for round-trip fidelity only and not
///   exposed as a setting.
/// - `sound_modes` (`bArr[custom_length + 112 .. custom_length + 118]`, 6 bytes): see
///   `a3953::structures::SoundModes` for the per-byte citation.
/// - `unknown_personal_anc_test_info` (`bArr[custom_length + 118 .. custom_length + 124]`, 6 bytes):
///   `PersonalAncInfo` test time (4 bytes, unix-ish timestamp), volume dB, and result index. Not
///   implemented; both observed captures show the "unset" sentinel `255` for the latter two.
/// - `wearing_detection` (`bArr[custom_length + 124]`): same command (`[0x01, 0x81]`, decompiled
///   constant `Cmm2CmdData.J`, used by a base `CmmBtCmdService` method not overridden by
///   `A3953CmdService`) as this project's shared `WearingDetection` flag. Cross-confirmed via the
///   UI layer: `A3953MoreVM.sendWearTestCmd` → `Cmm2BtDeviceManager.y5` → `CmmBtCmdService.Y0` →
///   same `Cmm2CmdData.J`, whose success callback (`dealSendWearDetectionCmd`) writes the result to
///   `Cmm2BtDeviceInfo.setWearDetectionSwitch`, the same bean field `R0` reads here.
/// - `unknown_wearing_status` (`bArr[custom_length + 125 .. custom_length + 127]`, 2 bytes): left
///   and right earbud in-ear status, reported only, no corresponding set command exists.
/// - `case_battery_level` (`bArr[custom_length + 127]`): read via the same `S()` clamp
///   (`0..=9`) as the earbuds' own battery level (see a3953.rs's `dual_battery` comment for why
///   that clamp alone isn't enough to conclude the true max level); displayed out of 5 to match
///   the structurally identical A3947 pending a lower-charge capture.
/// - `unknown_bass_up` (`bArr[custom_length + 128]`): a "bass up" toggle with no located set
///   command.
/// - `ldac` (`bArr[custom_length + 129]`): same command (`[0x01, 0xFF]`, decompiled constant
///   `Cmm2CmdData.f16o0`) as this project's shared `Ldac` flag.
/// - `unknown_dual_connection` (`bArr[custom_length + 130]`): a two-simultaneous-connections
///   toggle. This project's `DualConnections` structure is a much larger feature (a whole device
///   list with its own inbound/outbound packets) that doesn't match this single bit, so it isn't
///   reused here, and no matching single-bit set command was located either.
/// - `auto_power_off` (`bArr[custom_length + 131 .. custom_length + 133]`, 2 bytes): same command
///   (`[0x01, 0x86]`, decompiled constant `Cmm2CmdData.L`) and same
///   `(bool is_enabled, u8 duration_index)` layout as this project's shared `AutoPowerOff`
///   structure. The duration index is clamped to `0..=3` by `A3953AnalysisService.U2`. Confirmed,
///   not just inferred from the clamp: `A3952PowerOffModel.A3953_POWER_OFF_OPTIONS` is a literal
///   `{"10 ", "20 ", "30 ", "60 "}` array, selected specifically via
///   `"A3953".equalsIgnoreCase(str)`, matching `AutoPowerOffDuration::ten_twenty_thirty_sixty()`
///   exactly.
/// - `unknown_hear_id_volume_db` (`bArr[custom_length + 133]`): part of the Hear ID feature, not
///   implemented.
/// - `wearing_tone` (`bArr[custom_length + 134]`): the app calls this field "in ear beep", but it
///   uses the same command (`[0x01, 0x8C]`, decompiled constant `Cmm2CmdData.f17p0`) as this
///   project's shared `WearingTone` flag, so it's reused under that name.
/// - `unknown_tail` (`bArr[custom_length + 135 .. custom_length + 145]`, 10 bytes): low battery
///   alert, ambient sound prompt, spatial audio switch/effect mode/sound mode, and four
///   "daily care" health-tracking fields (sedentary reminder, sitting posture, heart rate
///   abnormality alarm and its threshold) that read as all-zero on this earbud and may belong to a
///   different product category sharing the same parser. None have a located set command.
/// - `device_colour` and `press_sensitivity`: only present when `R0`'s own length check
///   (`bArr.length > 163`, i.e. this packet's body is longer than 154 bytes) holds; a 1-byte ASCII
///   colour code with no located meaning, and a 1-byte value (see
///   `a3953::structures::PressSensitivity` for its citation).
/// - `unknown_suffix`: whatever's left; 2 bytes in the captures seen so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A3953StateUpdatePacket {
    pub tws_status: TwsStatus,
    pub battery: DualBattery,
    pub dual_firmware_version: DualFirmwareVersion,
    pub serial_number: SerialNumber,
    pub unknown_before_custom_length: Vec<u8>,
    pub custom_length: u8,
    pub button_config: a3953::structures::ButtonConfig,
    pub unknown_gap: Vec<u8>,
    pub ambient_sound_mode_cycle: AmbientSoundModeCycle,
    pub sound_modes: a3953::structures::SoundModes,
    pub unknown_personal_anc_test_info: Vec<u8>,
    pub wearing_detection: WearingDetection,
    pub unknown_wearing_status: Vec<u8>,
    pub case_battery_level: CaseBatteryLevel,
    pub unknown_bass_up: Vec<u8>,
    pub ldac: Ldac,
    pub unknown_dual_connection: Vec<u8>,
    pub auto_power_off: AutoPowerOff,
    pub unknown_hear_id_volume_db: Vec<u8>,
    pub wearing_tone: WearingTone,
    pub unknown_tail: Vec<u8>,
    pub device_colour: Option<u8>,
    pub press_sensitivity: Option<a3953::structures::PressSensitivity>,
    pub unknown_suffix: Vec<u8>,
}

impl Default for A3953StateUpdatePacket {
    fn default() -> Self {
        Self {
            tws_status: Default::default(),
            battery: Default::default(),
            dual_firmware_version: Default::default(),
            serial_number: Default::default(),
            // sized to match the fixed-width `take()` calls in `FromPacketBody::take` below, so
            // that `Self::default().to_packet()` round-trips through `take()` correctly
            unknown_before_custom_length: vec![0; 71],
            custom_length: 0,
            button_config: Default::default(),
            unknown_gap: Vec::new(),
            ambient_sound_mode_cycle: Default::default(),
            sound_modes: Default::default(),
            unknown_personal_anc_test_info: vec![0; 6],
            wearing_detection: Default::default(),
            unknown_wearing_status: vec![0; 2],
            case_battery_level: Default::default(),
            unknown_bass_up: vec![0; 1],
            ldac: Default::default(),
            unknown_dual_connection: vec![0; 1],
            auto_power_off: Default::default(),
            unknown_hear_id_volume_db: vec![0; 1],
            wearing_tone: Default::default(),
            unknown_tail: vec![0; 10],
            device_colour: None,
            press_sensitivity: None,
            unknown_suffix: Vec::new(),
        }
    }
}

impl FromPacketBody for A3953StateUpdatePacket {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        let total_len = input.len();
        context("a3953 state update packet", move |input| {
            let (input, tws_status) = TwsStatus::take(input)?;
            let (input, battery) = DualBattery::take(input)?;
            let (input, dual_firmware_version) = DualFirmwareVersion::take(input)?;
            let (input, serial_number) = SerialNumber::take(input)?;
            let (input, unknown_before_custom_length) = take(71usize)(input)?;
            let (input, custom_length) = le_u8(input)?;
            let (input, button_config) = a3953::structures::ButtonConfig::take(input)?;
            let gap_len = (custom_length as usize).saturating_sub(18);
            let (input, unknown_gap) = take(gap_len)(input)?;
            let (input, ambient_sound_mode_cycle) = AmbientSoundModeCycle::take(input)?;
            let (input, sound_modes) = a3953::structures::SoundModes::take(input)?;
            let (input, unknown_personal_anc_test_info) = take(6usize)(input)?;
            let (input, wearing_detection) = WearingDetection::take(input)?;
            let (input, unknown_wearing_status) = take(2usize)(input)?;
            let (input, case_battery_level) = CaseBatteryLevel::take(input)?;
            let (input, unknown_bass_up) = take(1usize)(input)?;
            let (input, ldac) = Ldac::take(input)?;
            let (input, unknown_dual_connection) = take(1usize)(input)?;
            let (input, auto_power_off) = AutoPowerOff::take(input)?;
            let (input, unknown_hear_id_volume_db) = take(1usize)(input)?;
            let (input, wearing_tone) = WearingTone::take(input)?;
            let (input, unknown_tail) = take(10usize)(input)?;
            let (input, (device_colour, press_sensitivity)) = if total_len > 154 {
                let (input, colour_byte) = le_u8(input)?;
                let (input, press_sensitivity) = a3953::structures::PressSensitivity::take(input)?;
                (input, (Some(colour_byte), Some(press_sensitivity)))
            } else {
                (input, (None, None))
            };
            let (input, unknown_suffix) = map(rest, |s: &[u8]| s.to_vec()).parse(input)?;
            Ok((
                input,
                Self {
                    tws_status,
                    battery,
                    dual_firmware_version,
                    serial_number,
                    unknown_before_custom_length: unknown_before_custom_length.to_vec(),
                    custom_length,
                    button_config,
                    unknown_gap: unknown_gap.to_vec(),
                    ambient_sound_mode_cycle,
                    sound_modes,
                    unknown_personal_anc_test_info: unknown_personal_anc_test_info.to_vec(),
                    wearing_detection,
                    unknown_wearing_status: unknown_wearing_status.to_vec(),
                    case_battery_level,
                    unknown_bass_up: unknown_bass_up.to_vec(),
                    ldac,
                    unknown_dual_connection: unknown_dual_connection.to_vec(),
                    auto_power_off,
                    unknown_hear_id_volume_db: unknown_hear_id_volume_db.to_vec(),
                    wearing_tone,
                    unknown_tail: unknown_tail.to_vec(),
                    device_colour,
                    press_sensitivity,
                    unknown_suffix,
                },
            ))
        })
        .parse_complete(input)
    }
}

impl ToPacket for A3953StateUpdatePacket {
    type DirectionMarker = packet::InboundMarker;

    fn command(&self) -> packet::Command {
        packet::inbound::STATE_COMMAND
    }

    fn body(&self) -> Vec<u8> {
        self.tws_status
            .bytes()
            .into_iter()
            .chain(self.battery.bytes())
            .chain(self.dual_firmware_version.bytes())
            .chain(self.serial_number.to_string().into_bytes())
            .chain(self.unknown_before_custom_length.iter().copied())
            .chain(iter::once(self.custom_length))
            .chain(self.button_config.bytes())
            .chain(self.unknown_gap.iter().copied())
            .chain(self.ambient_sound_mode_cycle.bytes())
            .chain(self.sound_modes.bytes())
            .chain(self.unknown_personal_anc_test_info.iter().copied())
            .chain(self.wearing_detection.bytes())
            .chain(self.unknown_wearing_status.iter().copied())
            .chain(self.case_battery_level.bytes())
            .chain(self.unknown_bass_up.iter().copied())
            .chain(self.ldac.bytes())
            .chain(self.unknown_dual_connection.iter().copied())
            .chain(self.auto_power_off.bytes())
            .chain(self.unknown_hear_id_volume_db.iter().copied())
            .chain(self.wearing_tone.bytes())
            .chain(self.unknown_tail.iter().copied())
            .chain(self.device_colour)
            .chain(
                self.press_sensitivity
                    .into_iter()
                    .flat_map(|value| value.bytes()),
            )
            .chain(self.unknown_suffix.iter().copied())
            .collect()
    }
}

state_update_packet_module!(A3953State, A3953StateUpdatePacket);

#[cfg(test)]
mod tests {
    use nom_language::error::VerboseError;

    use crate::devices::soundcore::common::packet::inbound::TryToPacket;

    use super::*;

    #[test]
    fn serialize_and_deserialize() {
        let bytes = A3953StateUpdatePacket::default()
            .to_packet()
            .bytes_with_checksum();
        let (_, packet) = packet::Inbound::take_with_checksum::<VerboseError<_>>(&bytes).unwrap();
        let _: A3953StateUpdatePacket = packet.try_to_packet().unwrap();
    }
}
