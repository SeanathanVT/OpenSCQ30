use nom::{
    IResult, Parser,
    combinator::{map, rest},
    error::{ContextError, ParseError, context},
};

use crate::devices::soundcore::{
    a3953::state::A3953State,
    common::{
        macros::state_update_packet_module,
        packet::{self, inbound::FromPacketBody, outbound::ToPacket},
        structures::{DualBattery, DualFirmwareVersion, SerialNumber, TwsStatus},
    },
};

/// Only `tws_status`, `battery`, `dual_firmware_version`, and `serial_number` have been
/// reverse-engineered so far (confirmed against a real capture from the device: the serial number
/// decodes to "3953" + the device's own MAC address, byte-reversed). Everything after that is
/// captured verbatim in `unknown_suffix` rather than guessed at, so round-tripping stays exact
/// until more fields (equalizer, sound modes, buttons, etc.) are identified from real captures.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct A3953StateUpdatePacket {
    pub tws_status: TwsStatus,
    pub battery: DualBattery,
    pub dual_firmware_version: DualFirmwareVersion,
    pub serial_number: SerialNumber,
    pub unknown_suffix: Vec<u8>,
}

impl FromPacketBody for A3953StateUpdatePacket {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context(
            "a3953 state update packet",
            map(
                (
                    TwsStatus::take,
                    DualBattery::take,
                    DualFirmwareVersion::take,
                    SerialNumber::take,
                    map(rest, |suffix: &[u8]| suffix.to_vec()),
                ),
                |(tws_status, battery, dual_firmware_version, serial_number, unknown_suffix)| {
                    Self {
                        tws_status,
                        battery,
                        dual_firmware_version,
                        serial_number,
                        unknown_suffix,
                    }
                },
            ),
        )
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
