use super::{
    Command, Connection, DATA_MARKER, PAYLOAD_LEN, Packet, begin, control, dongle, end, feature,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum FnMode {
    /// Fn layer active while Fn is held.
    #[default]
    Momentary,
    /// Fn toggles the Fn layer.
    Toggle,
}

/// Idle time before the keyboard sleeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Sleep {
    #[default]
    Never,
    OneMinute,
    FiveMinutes,
    ThirtyMinutes,
}

/// Debounce step, `1..=5`, 2 by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
pub struct Debounce(u8);

impl Debounce {
    pub const MAX: u8 = 5;

    pub fn new(value: u8) -> Option<Self> {
        (1..=Self::MAX).contains(&value).then_some(Self(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Debounce {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, String> {
        Self::new(value).ok_or_else(|| format!("expected 1..={}", Self::MAX))
    }
}

impl From<Debounce> for u8 {
    fn from(debounce: Debounce) -> u8 {
        debounce.0
    }
}

impl Default for Debounce {
    fn default() -> Self {
        Self(2)
    }
}

/// Firmware settings. Every field is sent; there is no partial update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Settings {
    pub disable_alt_tab: bool,
    pub disable_alt_f4: bool,
    pub disable_win: bool,
    pub fn_mode: FnMode,
    pub sleep: Sleep,
    pub debounce: Debounce,
}

/// The USB header's byte 2 is always 1 (meaning unknown).
pub fn set_settings(connection: Connection, settings: &Settings) -> Vec<Packet> {
    let mut data = [0; PAYLOAD_LEN];
    data[1] = 1;
    data[2] = settings.disable_alt_tab.into();
    data[3] = settings.disable_alt_f4.into();
    data[4] = settings.disable_win.into();
    data[5] = settings.fn_mode as u8;
    data[6] = settings.sleep as u8;
    data[8] = settings.debounce.get();
    data[PAYLOAD_LEN - 2..].copy_from_slice(&DATA_MARKER);

    match connection {
        Connection::Usb => {
            let mut header = control(Command::Settings, 1);
            header[2] = 1;
            vec![begin(), feature(header, true), feature(data, false), end()]
        }
        Connection::Dongle => vec![dongle::short(dongle::command::SETTINGS, &data)],
    }
}

#[cfg(test)]
mod tests {
    use super::super::frame;
    use super::*;

    #[test]
    fn settings() {
        let settings = Settings {
            disable_win: true,
            fn_mode: FnMode::Toggle,
            sleep: Sleep::FiveMinutes,
            debounce: Debounce::new(5).unwrap(),
            ..Settings::default()
        };
        let mut data = frame(&[0, 1, 0, 0, 1, 1, 2, 0, 5]);
        data[62..].copy_from_slice(&DATA_MARKER);

        assert_eq!(
            set_settings(Connection::Usb, &settings),
            [
                feature(frame(&[0x04, 0x18]), true),
                feature(frame(&[0x04, 0x17, 1, 0, 0, 0, 0, 0, 1]), true),
                feature(data, false),
                feature(frame(&[0x04, 0x02]), true),
            ]
        );
    }

    #[test]
    fn debounce_bounds() {
        assert_eq!(Debounce::new(0), None);
        assert_eq!(Debounce::new(6), None);
    }
}
