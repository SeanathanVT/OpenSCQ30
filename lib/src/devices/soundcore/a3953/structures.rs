use nom::{
    IResult, Parser,
    combinator::map,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::{
    modules::sound_modes_v2::ToPacketBody,
    packet::{self, inbound::FromPacketBody},
    structures::{AmbientSoundMode, WindNoise},
};

/// Byte layout reverse-engineered from the official Soundcore Android app (com.oceanwing.soundcore
/// v6.4.0-17), decompiling `com.oceanwing.devicecmd.manager.product.a3953.A3953CmdService.G1`
/// (outbound, command `[0x06, 0x81]`) and `A3953AnalysisService.T2` (inbound, same byte range
/// within the state update packet). `ambient_sound_mode` is validated to exactly {0,1,2} by
/// `CmmBtCmdService.h()` before sending. `anc_option_manual` is clamped to 1-6 by
/// `A3953CmdService.f()` on both read and write. `anc_option_auto`, `trans_option`,
/// `anc_automation_mode`, and `anc_auto_sensitivity_level` are read as raw, unclamped bytes;
/// `trans_option` is additionally clamped to 0-2 by `A3953CmdService.i()`, but only when sending.
/// `wind_noise.is_detected` is read from the device but is never sent back (the app always writes
/// only the suppression-enabled bit).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SoundModes {
    pub ambient_sound_mode: AmbientSoundMode,
    pub anc_option_manual: u8,
    pub anc_option_auto: u8,
    pub trans_option: u8,
    pub anc_automation_mode: u8,
    pub wind_noise: WindNoise,
    pub anc_auto_sensitivity_level: u8,
}

impl SoundModes {
    pub fn bytes(&self) -> [u8; 6] {
        [
            self.ambient_sound_mode as u8,
            (self.anc_option_manual.clamp(1, 6) << 4) | (self.anc_option_auto & 0x0F),
            self.trans_option.clamp(0, 2),
            self.anc_automation_mode,
            u8::from(self.wind_noise.is_suppression_enabled),
            self.anc_auto_sensitivity_level,
        ]
    }
}

impl FromPacketBody for SoundModes {
    type DirectionMarker = packet::InboundMarker;

    fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context(
            "a3953 sound modes",
            map(
                (
                    AmbientSoundMode::take,
                    le_u8,
                    le_u8,
                    le_u8,
                    WindNoise::take,
                    le_u8,
                ),
                |(
                    ambient_sound_mode,
                    manual_auto_byte,
                    trans_option,
                    anc_automation_mode,
                    wind_noise,
                    anc_auto_sensitivity_level,
                )| Self {
                    ambient_sound_mode,
                    anc_option_manual: (manual_auto_byte >> 4).clamp(1, 6),
                    anc_option_auto: manual_auto_byte & 0x0F,
                    trans_option,
                    anc_automation_mode,
                    wind_noise,
                    anc_auto_sensitivity_level,
                },
            ),
        )
        .parse_complete(input)
    }
}

impl ToPacketBody for SoundModes {
    fn bytes(&self) -> Vec<u8> {
        self.bytes().to_vec()
    }
}

/// One button press-type's (single/double/long/triple press, for one earbud) configuration, as
/// read by `com.oceanwing.devicecmd.manager.product.a3953.A3953AnalysisService.G0`: two bytes,
/// each nibble-packed as `(BytesUtil.G(byte), BytesUtil.K(byte))` = (high nibble, low nibble).
/// `untws_*` fields apply when the earbud is used alone (not connected as a TWS pair); the
/// unprefixed fields apply when TWS-connected. `action`/`untws_action` are raw 0-15 IDs; no
/// enum mapping from ID to behavior (e.g. "volume up") was found anywhere in the decompiled
/// source, so they're left as plain numbers rather than guessed at. No outbound command that
/// writes this struct back was found either, so it's read-only for now.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ButtonAssignment {
    pub untws_enabled: bool,
    pub enabled: bool,
    pub untws_action: u8,
    pub action: u8,
}

impl ButtonAssignment {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map((le_u8, le_u8), |(switch_byte, action_byte)| Self {
            untws_enabled: switch_byte >> 4 == 1,
            enabled: switch_byte & 0x0F == 1,
            untws_action: action_byte >> 4,
            action: action_byte & 0x0F,
        })
        .parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 2] {
        [
            (u8::from(self.untws_enabled) << 4) | u8::from(self.enabled),
            ((self.untws_action & 0x0F) << 4) | (self.action & 0x0F),
        ]
    }
}

/// Full 16-byte button configuration block (`bArr[113..129]` in `A3953AnalysisService.R0`, i.e.
/// `bArr[113..129]`, passed to `G0` starting at `bArr[113]`). Field order (single, double, long,
/// triple; left before right for each) matches the order `G0` populates them in, which is not the
/// same order this project's other Soundcore devices use (they go single/double/triple/long).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ButtonConfig {
    pub left_single: ButtonAssignment,
    pub right_single: ButtonAssignment,
    pub left_double: ButtonAssignment,
    pub right_double: ButtonAssignment,
    pub left_long: ButtonAssignment,
    pub right_long: ButtonAssignment,
    pub left_triple: ButtonAssignment,
    pub right_triple: ButtonAssignment,
}

impl ButtonConfig {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context(
            "a3953 button config",
            map(
                (
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                    ButtonAssignment::take,
                ),
                |(
                    left_single,
                    right_single,
                    left_double,
                    right_double,
                    left_long,
                    right_long,
                    left_triple,
                    right_triple,
                )| Self {
                    left_single,
                    right_single,
                    left_double,
                    right_double,
                    left_long,
                    right_long,
                    left_triple,
                    right_triple,
                },
            ),
        )
        .parse_complete(input)
    }

    pub fn bytes(&self) -> impl Iterator<Item = u8> {
        [
            self.left_single,
            self.right_single,
            self.left_double,
            self.right_double,
            self.left_long,
            self.right_long,
            self.left_triple,
            self.right_triple,
        ]
        .into_iter()
        .flat_map(|button| button.bytes())
    }
}

/// A 0-4 selection with no known display labels (the app's own `A3953PressSensVM.initData`
/// pairs each value with a string resource ID rather than a literal name, and that string table
/// wasn't decoded), sent with command `[0x04, 0x85]` (decompiled constant `Cmm2CmdData.L0`, used
/// by the base `CmmBtCmdService.O(int)`, called from `Cmm2BtDeviceManager.g5(int)`, called from
/// `A3953PressSensVM.setPressSensItem`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PressSensitivity(pub u8);

impl PressSensitivity {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map(le_u8, |value: u8| Self(value.min(4))).parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 1] {
        [self.0.min(4)]
    }
}
