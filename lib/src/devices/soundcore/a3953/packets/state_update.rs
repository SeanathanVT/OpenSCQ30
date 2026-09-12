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
            AmbientSoundModeCycle, DualBattery, DualFirmwareVersion, SerialNumber, TwsStatus,
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
/// - `button_config` (`bArr[113..129]`, 16 bytes): parsed by `R0`'s `G0` into a `ControllerBtnModel`
///   (left/right x single/double/triple/long press, TWS and non-TWS). Not decoded into a typed
///   struct yet.
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
/// - `unknown_suffix`: everything after that (wear detection, LDAC, auto power off, spatial audio,
///   health-tracking fields, device colour, press sensitivity, per `R0`) is not implemented yet and
///   is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A3953StateUpdatePacket {
    pub tws_status: TwsStatus,
    pub battery: DualBattery,
    pub dual_firmware_version: DualFirmwareVersion,
    pub serial_number: SerialNumber,
    pub unknown_before_custom_length: Vec<u8>,
    pub custom_length: u8,
    pub button_config: Vec<u8>,
    pub unknown_gap: Vec<u8>,
    pub ambient_sound_mode_cycle: AmbientSoundModeCycle,
    pub sound_modes: a3953::structures::SoundModes,
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
            button_config: vec![0; 16],
            unknown_gap: Vec::new(),
            ambient_sound_mode_cycle: Default::default(),
            sound_modes: Default::default(),
            unknown_suffix: Vec::new(),
        }
    }
}

impl FromPacketBody for A3953StateUpdatePacket {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context("a3953 state update packet", |input| {
            let (input, tws_status) = TwsStatus::take(input)?;
            let (input, battery) = DualBattery::take(input)?;
            let (input, dual_firmware_version) = DualFirmwareVersion::take(input)?;
            let (input, serial_number) = SerialNumber::take(input)?;
            let (input, unknown_before_custom_length) = take(71usize)(input)?;
            let (input, custom_length) = le_u8(input)?;
            let (input, button_config) = take(16usize)(input)?;
            let gap_len = (custom_length as usize).saturating_sub(18);
            let (input, unknown_gap) = take(gap_len)(input)?;
            let (input, ambient_sound_mode_cycle) = AmbientSoundModeCycle::take(input)?;
            let (input, sound_modes) = a3953::structures::SoundModes::take(input)?;
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
                    button_config: button_config.to_vec(),
                    unknown_gap: unknown_gap.to_vec(),
                    ambient_sound_mode_cycle,
                    sound_modes,
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
            .chain(self.button_config.iter().copied())
            .chain(self.unknown_gap.iter().copied())
            .chain(self.ambient_sound_mode_cycle.bytes())
            .chain(self.sound_modes.bytes())
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
