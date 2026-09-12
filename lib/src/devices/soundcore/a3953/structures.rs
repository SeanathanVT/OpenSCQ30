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
