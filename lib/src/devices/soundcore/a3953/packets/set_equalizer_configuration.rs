use crate::devices::soundcore::common::{self, packet, structures::OptionalVolumeAdjustmentsExt};

/// Command `[0x03, 0x87]` (decompiled constant `Cmm2CmdData.p1`), built by
/// `CmmBtCmdService.v5`, reached from `A3953CmdService.c5()` returning `true`, which routes
/// `CmmBtCmdService.d4` to `z5` (which always applies DRC to the plain custom EQ values via `n5`,
/// unconditionally on Hear ID state) and then to `v5` itself. Byte layout cross-checked against this
/// project's own `a3955` device, which shares the identical wire format for the same command:
/// preset id, then the Hear ID "favorite music genre"/"hearIdEqIndex" slot (same two bytes, meaning
/// depends on `hear_id_type`), then the plain custom EQ values, then a 2-byte slot that's `0` when
/// the device already has Hear ID data or `255` when it doesn't (`Cmm2CmdData.x`), then the Hear ID
/// switch, Hear ID values, Hear ID time (big-endian), Hear ID type, custom Hear ID values, the DRC
/// (`apply_drc`) transform of the plain custom EQ values, and a trailing always-zero byte.
pub fn set_equalizer_configuration<
    const CHANNELS: usize,
    const BANDS: usize,
    const MIN_VOLUME: i16,
    const MAX_VOLUME: i16,
    const FRACTION_DIGITS: u8,
>(
    equalizer_configuration: &common::structures::EqualizerConfiguration<
        CHANNELS,
        BANDS,
        MIN_VOLUME,
        MAX_VOLUME,
        FRACTION_DIGITS,
    >,
    hear_id: &common::structures::CustomHearId<CHANNELS, BANDS>,
    is_hear_id_initialized: bool,
) -> packet::Outbound {
    let body = equalizer_configuration
        .preset_id()
        .to_le_bytes()
        .into_iter()
        .chain(hear_id.favorite_music_genre.bytes())
        .chain(equalizer_configuration.volume_adjustments_bytes())
        .chain(if is_hear_id_initialized {
            [0; 2]
        } else {
            [255; 2]
        })
        .chain(std::iter::once(u8::from(hear_id.is_enabled)))
        .chain(hear_id.volume_adjustments.iter().flat_map(|v| {
            let mut bytes = v.bytes();
            if v.is_none() {
                bytes[bytes.len() - 1] = 0;
            }
            bytes
        }))
        .chain(hear_id.time.to_be_bytes())
        .chain(std::iter::once(hear_id.hear_id_type as u8))
        .chain(hear_id.custom_volume_adjustments.iter().flat_map(|v| {
            let mut bytes = v.bytes();
            if v.is_none() {
                bytes[bytes.len() - 1] = 0;
            }
            bytes
        }))
        .chain(
            equalizer_configuration
                .volume_adjustments()
                .iter()
                .flat_map(|v| v.apply_drc().bytes()),
        )
        .chain(std::iter::once(0))
        .collect();

    packet::Outbound::new(packet::Command([3, 135]), body)
}
