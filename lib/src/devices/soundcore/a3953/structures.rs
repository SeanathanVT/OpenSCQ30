use nom::{
    IResult, Parser,
    combinator::map,
    error::{ContextError, ParseError, context},
    number::complete::le_u8,
};

use crate::devices::soundcore::common::{
    modules::sound_modes_v2::ToPacketBody,
    packet::{self, inbound::FromPacketBody, parsing::take_bool},
    structures::{AmbientSoundMode, Flag, WindNoise},
};

// [0x06,0x81] outbound (A3953CmdService.G1) / A3953AnalysisService.T2 inbound; anc_option_manual
// clamped 1-6, trans_option 0-2 (write only), other fields raw; wind_noise.is_detected never sent back
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

// Wire IDs/names from DeviceInfoUtil.t(cmdName), cross-checked against a real capture (PlayPause=6,
// Next/AmbientSoundModeCycle=3/4). 10 and 14 are each shared by two differently-named cmdNames
// (TakePhotoOrTranslate, StartSleepOrColorfulLight), so those pairs can't be told apart from the wire
// value alone; 7/9/12 are unused. Read-only: no write command found for any device family.
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

// Two nibble-packed bytes (A3953AnalysisService.G0); untws_* apply when used solo, unprefixed apply
// when TWS-connected. See ButtonAction for what action/untws_action mean.
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

// bArr[113..129] (A3953AnalysisService.R0/G0); field order is single/double/long/triple, unlike
// this project's other Soundcore devices (single/double/triple/long).
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

// [0x04,0x85] (Cmm2CmdData.L0/CmmBtCmdService.O); 0-4, no known display labels (string table wasn't decoded)
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

// A3953SpatialAudioVM.SPATIAL_MODE_FIXED/HEAD_TRACKING = 1/2
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

// A3953SpatialAudioVM.SOUND_MODE_MUSIC/MOVIE = 0/1
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

// [0x10,0x81] (Cmm2CmdData.u2/CmmBtCmdService.b4); byte order matches A3953AnalysisService.R0's read order
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

// [0x10,0x83] (Cmm2CmdData.w2/CmmBtCmdService.e1)
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

impl Flag for AmbientSoundPrompt {
    fn get_bool(&self) -> bool {
        self.0
    }

    fn set_bool(&mut self, value: bool) {
        self.0 = value;
    }
}

// [0x0B,0x84] (Cmm2CmdData.E1/CmmBtCmdService.B0); single bit, not this project's full DualConnections
// device-list feature
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

impl Flag for SupportTwoConnections {
    fn get_bool(&self) -> bool {
        self.0
    }

    fn set_bool(&mut self, value: bool) {
        self.0 = value;
    }
}

// bArr[63] (A3953AnalysisService.R0's m3); 255 or 254 = uninitialized (unlike a3955, which only
// checks one sentinel). Determines whether the equalizer write path sends the "uninitialized"
// sentinel bytes or the real hear id already stored on the device.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IsHearIdInitialized(pub bool);
