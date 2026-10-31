use super::{
    Command, Connection, DATA_MARKER, PAYLOAD_LEN, Packet, begin, dongle, end, feature, header,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClockTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// 0 is Sunday.
    pub weekday: u8,
}

/// Also carries the screen slot (1-based) and a constant `0x5A`; both purposes
/// are unknown.
pub fn sync_clock(connection: Connection, screen_slot: u8, time: &ClockTime) -> Vec<Packet> {
    let mut data = [0; PAYLOAD_LEN];
    data[1] = screen_slot;
    data[2] = 0x5A;
    data[3] = (time.year % 2000) as u8;
    data[4] = time.month;
    data[5] = time.day;
    data[6] = time.hour;
    data[7] = time.minute;
    data[8] = time.second;
    data[10] = time.weekday;
    data[PAYLOAD_LEN - 2..].copy_from_slice(&DATA_MARKER);

    match connection {
        Connection::Usb => vec![
            begin(),
            header(Command::Clock, 1),
            feature(data, true),
            end(),
        ],
        Connection::Dongle => vec![dongle::short(dongle::command::CLOCK, &data)],
    }
}

#[cfg(test)]
mod tests {
    use super::super::frame;
    use super::*;

    #[test]
    fn clock() {
        let time = ClockTime {
            year: 2026,
            month: 10,
            day: 31,
            hour: 23,
            minute: 59,
            second: 7,
            weekday: 6,
        };
        let mut data = frame(&[0, 1, 0x5A, 26, 10, 31, 23, 59, 7, 0, 6]);
        data[62..].copy_from_slice(&DATA_MARKER);

        assert_eq!(
            sync_clock(Connection::Usb, 1, &time),
            [
                feature(frame(&[0x04, 0x18]), true),
                feature(frame(&[0x04, 0x28, 0, 0, 0, 0, 0, 0, 1]), true),
                feature(data, true),
                feature(frame(&[0x04, 0x02]), true),
            ]
        );
    }
}
