use nom::{
    IResult, Parser,
    combinator::map,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::{
    modules::sound_modes_v2::ToPacketBody,
    packet::{self, inbound::FromPacketBody, parsing::take_bool},
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

/// The behavior a button press is mapped to. IDs and names come from
/// `com.soundcore.control.utils.DeviceInfoUtil.t(String)`, a `cmdName` (e.g.
/// `PushLogConstant.VALUAS_APP_CUSTOM_PLAY_PAUSE`) to wire-ID switch shared by this app's whole
/// button-customization UI (not device-specific), and cross-checked against this device's own real
/// capture: the single-press assignments decode to `PlayPause` (6) and the double-press assignments
/// decode to `Next`/`AmbientSoundModeCycle` (3/4), all of which are named by this same switch.
/// `TakePhotoOrTranslate` and `StartSleepOrColorfulLight` share one wire ID each between two
/// differently-named `cmdName` constants in the decompiled source (`custom_take_photo`/
/// `custom_translate` both map to 10; `custom_start_sleep`/`custom_color_ful_light` both fall
/// through to the same `return 14`), so this project can't tell those two pairs apart from the wire
/// value alone. IDs 7, 9, and 12 aren't produced by any `cmdName` in that switch, so they're left as
/// `Unknown` rather than guessed at. No outbound command that writes a `ButtonAssignment` back was
/// found anywhere in the decompiled source (confirmed absent from every `*AnalysisService`'s sibling
/// `*CmdService` across every device family that uses `ControllerBtnModel`, not just this one), so
/// this stays read-only.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    VolumeUp,
    VolumeDown,
    Previous,
    Next,
    AmbientSoundModeCycle,
    VoiceAssistant,
    PlayPause,
    AncSwitch,
    TakePhotoOrTranslate,
    AskAnswer,
    SwitchMode,
    StartSleepOrColorfulLight,
    #[default]
    None,
    Unknown(u8),
}

impl ButtonAction {
    fn from_nibble(nibble: u8) -> Self {
        match nibble {
            0 => Self::VolumeUp,
            1 => Self::VolumeDown,
            2 => Self::Previous,
            3 => Self::Next,
            4 => Self::AmbientSoundModeCycle,
            5 => Self::VoiceAssistant,
            6 => Self::PlayPause,
            8 => Self::AncSwitch,
            10 => Self::TakePhotoOrTranslate,
            11 => Self::AskAnswer,
            13 => Self::SwitchMode,
            14 => Self::StartSleepOrColorfulLight,
            15 => Self::None,
            other => Self::Unknown(other),
        }
    }

    fn to_nibble(self) -> u8 {
        match self {
            Self::VolumeUp => 0,
            Self::VolumeDown => 1,
            Self::Previous => 2,
            Self::Next => 3,
            Self::AmbientSoundModeCycle => 4,
            Self::VoiceAssistant => 5,
            Self::PlayPause => 6,
            Self::AncSwitch => 8,
            Self::TakePhotoOrTranslate => 10,
            Self::AskAnswer => 11,
            Self::SwitchMode => 13,
            Self::StartSleepOrColorfulLight => 14,
            Self::None => 15,
            Self::Unknown(nibble) => nibble,
        }
    }
}

/// One button press-type's (single/double/long/triple press, for one earbud) configuration, as
/// read by `com.oceanwing.devicecmd.manager.product.a3953.A3953AnalysisService.G0`: two bytes,
/// each nibble-packed as `(BytesUtil.G(byte), BytesUtil.K(byte))` = (high nibble, low nibble).
/// `untws_*` fields apply when the earbud is used alone (not connected as a TWS pair); the
/// unprefixed fields apply when TWS-connected. See `ButtonAction` for what `action`/`untws_action`
/// mean.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ButtonAssignment {
    pub untws_enabled: bool,
    pub enabled: bool,
    pub untws_action: ButtonAction,
    pub action: ButtonAction,
}

impl ButtonAssignment {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map((le_u8, le_u8), |(switch_byte, action_byte)| Self {
            untws_enabled: switch_byte >> 4 == 1,
            enabled: switch_byte & 0x0F == 1,
            untws_action: ButtonAction::from_nibble(action_byte >> 4),
            action: ButtonAction::from_nibble(action_byte & 0x0F),
        })
        .parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 2] {
        [
            (u8::from(self.untws_enabled) << 4) | u8::from(self.enabled),
            ((self.untws_action.to_nibble() & 0x0F) << 4) | (self.action.to_nibble() & 0x0F),
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

/// Whether the spatial-audio "effect" is fixed in place or head-tracked, cited from the literal
/// constants `A3953SpatialAudioVM.SPATIAL_MODE_FIXED = 1` / `SPATIAL_MODE_HEAD_TRACKING = 2`
/// (confirmed against `A3953SpatialActivity`'s toggle handlers, which call
/// `effectMode.set(1)`/`effectMode.set(2)`).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    ::strum::FromRepr,
    ::strum::Display,
    ::strum::IntoStaticStr,
    ::strum::EnumString,
    ::strum::EnumIter,
    ::strum::VariantArray,
    ::openscq30_i18n_macros::Translate,
)]
#[repr(u8)]
pub enum SpatialMode {
    #[default]
    Fixed = 1,
    HeadTracking = 2,
}

impl SpatialMode {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map(le_u8, |b| Self::from_repr(b).unwrap_or_default()).parse_complete(input)
    }
}

/// The spatial-audio content-type selector, cited from the literal constants
/// `A3953SpatialAudioVM.SOUND_MODE_MUSIC = 0` / `SOUND_MODE_MOVIE = 1` (confirmed against
/// `A3953SpatialActivity`'s toggle handlers, which call `soundMode.set(0)`/`soundMode.set(1)`).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    ::strum::FromRepr,
    ::strum::Display,
    ::strum::IntoStaticStr,
    ::strum::EnumString,
    ::strum::EnumIter,
    ::strum::VariantArray,
    ::openscq30_i18n_macros::Translate,
)]
#[repr(u8)]
pub enum SpatialContentMode {
    #[default]
    Music = 0,
    Movie = 1,
}

impl SpatialContentMode {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map(le_u8, |b| Self::from_repr(b).unwrap_or_default()).parse_complete(input)
    }
}

/// Spatial audio (switch, effect mode, content mode), sent together as 3 bytes with command
/// `[0x10, 0x81]` (decompiled constant `Cmm2CmdData.u2`, built by `CmmBtCmdService.b4`, called from
/// `Cmm2BtDeviceManager.T6`, called from `A3953SpatialAudioVM.setSpatialAudioFun`). Byte order
/// (switch, effect mode, content mode) matches `A3953AnalysisService.R0`'s read order exactly
/// (`spatialSwitch`, `spatialEffectMode`, `spatialSoundMode`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpatialAudio {
    pub is_enabled: bool,
    pub effect_mode: SpatialMode,
    pub sound_mode: SpatialContentMode,
}

impl SpatialAudio {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        context(
            "a3953 spatial audio",
            map(
                (take_bool, SpatialMode::take, SpatialContentMode::take),
                |(is_enabled, effect_mode, sound_mode)| Self {
                    is_enabled,
                    effect_mode,
                    sound_mode,
                },
            ),
        )
        .parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 3] {
        [
            u8::from(self.is_enabled),
            self.effect_mode as u8,
            self.sound_mode as u8,
        ]
    }
}

/// Sent with command `[0x10, 0x83]` (decompiled constant `Cmm2CmdData.w2`, built by
/// `CmmBtCmdService.e1`, called from `Cmm2BtDeviceManager.v4`, called from
/// `A3953PromptVM.setAmbientChangeSwitch`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AmbientSoundPrompt(pub bool);

impl AmbientSoundPrompt {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map(take_bool, Self).parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 1] {
        [u8::from(self.0)]
    }
}

/// Lets the earbuds stay connected to two phones/devices at once (labeled "SupportTwoCnnSwitch" in
/// the decompiled bean, toggled from the device list screen). Sent with command `[0x0B, 0x84]`
/// (decompiled constant `Cmm2CmdData.E1`, built by `CmmBtCmdService.B0`, called from
/// `Cmm2BtDeviceManager.a7`, called from `A3952DeviceListVM.sendDeviceListSwitchCmd`, the shared
/// device-list view model this device's `A3953DeviceListActivity` reuses).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SupportTwoConnections(pub bool);

impl SupportTwoConnections {
    pub fn take<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        input: &'a [u8],
    ) -> IResult<&'a [u8], Self, E> {
        map(take_bool, Self).parse_complete(input)
    }

    pub fn bytes(&self) -> [u8; 1] {
        [u8::from(self.0)]
    }
}

/// Whether the device has ever recorded a Hear ID (personalized hearing profile) result. Read from
/// the single byte immediately preceding the Hear ID block in the state update packet
/// (`A3953AnalysisService.R0`'s `m3`, `bArr[63]`), which `R0` treats as "no data" when it equals
/// either `Cmm2CmdData.x` (`-1`/255) or `Cmm2CmdData.y` (`-2`/254); this project's sibling A3955
/// device only checks the single-sentinel case, but A3953's own decompiled `R0` checks both, so both
/// are checked here. This project doesn't expose Hear ID for editing (see `HearId` on this device's
/// state), so the only use of this flag is deciding whether the equalizer write path needs to send
/// the "uninitialized" sentinel bytes (`CmmBtCmdService.v5`, called through `A3953CmdService.c5`'s
/// `z5`) or the real ones already stored on the device.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IsHearIdInitialized(pub bool);
