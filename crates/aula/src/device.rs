use std::io;
use std::thread;
use std::time::Duration;

use crate::descriptor::ReportSizes;
use crate::hid::{self, DeviceInfo, Hidraw};
use crate::protocol::{Connection, PAYLOAD_LEN, Packet, Query, SCREEN_CHUNK_LEN, dongle};

impl Connection {
    /// Vendor and product ID.
    const fn ids(self) -> (u16, u16) {
        match self {
            Connection::Usb => (0x0C45, 0x800A),
            Connection::Dongle => (0x05AC, 0x024F),
        }
    }

    /// HID name. Both ID pairs are shared with other devices (the dongle's is
    /// an Apple keyboard's), so the name is checked too.
    const fn name(self) -> &'static str {
        match self {
            Connection::Usb => "SONiX AULA F75Max",
            Connection::Dongle => "2.4G Dongle",
        }
    }

    fn matches(self, device: &DeviceInfo) -> bool {
        (device.vendor_id, device.product_id) == self.ids() && device.name == self.name()
    }
}

/// Sleeps before each feature report: `control` for reports whose reply is
/// read, `data` for the others, `reply` before reading a reply.
struct Pacing {
    control: Duration,
    data: Duration,
    reply: Duration,
}

/// Slept before every report of a stored command.
const STORED: Pacing = Pacing {
    control: Duration::from_millis(35),
    data: Duration::from_millis(35),
    reply: Duration::from_millis(35),
};

/// The real-time stream's shorter delays.
const REALTIME: Pacing = Pacing {
    control: Duration::from_millis(5),
    data: Duration::from_millis(2),
    reply: Duration::from_millis(5),
};

const SCREEN_ACK_TIMEOUT: Duration = Duration::from_millis(300);

/// Dongle exchange timings: a pause after the write, then up to
/// `DONGLE_READS` short reads until the echo arrives, and a 2 ms gap between
/// bulk packets. A missing echo is not an error: the dongle sent none for the
/// first macro chunk on hardware.
const DONGLE_AFTER_WRITE: Duration = Duration::from_millis(5);
const DONGLE_READ_TIMEOUT: Duration = Duration::from_millis(5);
const DONGLE_READ_RETRY: Duration = Duration::from_millis(10);
const DONGLE_READS: usize = 20;
const DONGLE_GAP: Duration = Duration::from_millis(2);
/// Reply byte 3 of a query's status report when data frames follow.
const MORE_DATA: u8 = 0x01;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no AULA F75 Max found")]
    NotFound,
    #[error("no control interface found on the {0:?} connection")]
    NoControlInterface(Connection),
    #[error("no screen interface found")]
    NoScreenInterface,
    #[error("this needs the {0:?} connection")]
    WrongConnection(Connection),
    #[error("unexpected reply {0:02x?}")]
    UnexpectedReply(Vec<u8>),
    #[error("the dongle did not reply")]
    NoReply,
    #[error("packet {index} ({packet})")]
    Packet {
        index: usize,
        packet: String,
        source: io::Error,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct Keyboard {
    connection: Connection,
    control: Hidraw,
    screen: Option<Hidraw>,
}

impl Keyboard {
    /// Opens the first keyboard found, preferring USB.
    ///
    /// Interfaces are picked by the reports they declare: over USB, 64-byte
    /// feature reports for control (interface 3) and 4096-byte output reports
    /// for the screen; on the dongle, 32-byte input and output reports.
    pub fn open() -> Result<Self, Error> {
        let devices = hid::enumerate()?;
        for connection in [Connection::Usb, Connection::Dongle] {
            let interfaces: Vec<_> = devices.iter().filter(|d| connection.matches(d)).collect();
            if interfaces.is_empty() {
                continue;
            }
            let find = |declares: fn(&ReportSizes) -> bool| {
                interfaces.iter().find(|d| declares(&d.report_sizes))
            };
            let control = match connection {
                Connection::Usb => find(|s| s.feature >= PAYLOAD_LEN),
                Connection::Dongle => {
                    find(|s| s.input == dongle::REPORT_LEN && s.output == dongle::REPORT_LEN)
                }
            }
            .ok_or(Error::NoControlInterface(connection))?;
            let screen = find(|s| s.output == SCREEN_CHUNK_LEN);
            return Ok(Self {
                connection,
                control: Hidraw::open(&control.path)?,
                screen: screen.map(|d| Hidraw::open(&d.path)).transpose()?,
            });
        }
        Err(Error::NotFound)
    }

    pub fn connection(&self) -> Connection {
        self.connection
    }

    pub fn send(&mut self, packets: &[Packet]) -> Result<(), Error> {
        self.send_with_progress(packets, |_| {})
    }

    /// Calls `progress` with the number of packets sent so far after each one.
    pub fn send_with_progress(
        &mut self,
        packets: &[Packet],
        progress: impl FnMut(usize),
    ) -> Result<(), Error> {
        self.send_paced(packets, &STORED, progress)
    }

    /// Sends with the short real-time delays, for frames that are
    /// repeated many times a second such as [`stream_key_colors`](crate::protocol::stream_key_colors).
    pub fn stream(&mut self, packets: &[Packet]) -> Result<(), Error> {
        self.send_paced(packets, &REALTIME, |_| {})
    }

    /// Reads state over USB, such as [`read_key_colors`](crate::protocol::read_key_colors).
    pub fn query<T>(&mut self, query: &Query<T>) -> Result<T, Error> {
        self.require(Connection::Usb)?;
        let failed = |source| Error::Packet {
            index: 0,
            packet: Packet::Feature {
                payload: query.request,
                read_reply: true,
            }
            .to_string(),
            source,
        };
        let mut report = [0; 1 + PAYLOAD_LEN];
        report[1..].copy_from_slice(&query.request);
        thread::sleep(STORED.control);
        self.control.set_feature(&report).map_err(failed)?;
        thread::sleep(STORED.reply);
        self.control.get_feature(&mut report).map_err(failed)?;
        if report[1 + 3] != MORE_DATA {
            return Err(Error::UnexpectedReply(report[1..].to_vec()));
        }

        let mut block = Vec::with_capacity(query.reply_frames * PAYLOAD_LEN);
        for _ in 0..query.reply_frames {
            thread::sleep(STORED.reply);
            self.control.get_feature(&mut report).map_err(failed)?;
            block.extend_from_slice(&report[1..]);
        }
        self.send(std::slice::from_ref(&query.finish))?;
        Ok((query.parse)(&block))
    }

    /// Battery level in percent, over the dongle.
    pub fn battery(&mut self) -> Result<u8, Error> {
        self.require(Connection::Dongle)?;
        let Packet::Dongle(request) = dongle::read_battery() else {
            unreachable!("read_battery builds a dongle packet")
        };
        let reply = self.dongle_exchange(&request)?.ok_or(Error::NoReply)?;
        dongle::battery_percent(&reply).ok_or_else(|| Error::UnexpectedReply(reply.to_vec()))
    }

    fn require(&self, connection: Connection) -> Result<(), Error> {
        if self.connection == connection {
            Ok(())
        } else {
            Err(Error::WrongConnection(connection))
        }
    }

    fn send_paced(
        &mut self,
        packets: &[Packet],
        pacing: &Pacing,
        mut progress: impl FnMut(usize),
    ) -> Result<(), Error> {
        for packet in packets {
            self.require(packet.connection())?;
        }
        let needs_screen = packets.iter().any(|p| matches!(p, Packet::ScreenChunk(_)));
        if needs_screen && self.screen.is_none() {
            return Err(Error::NoScreenInterface);
        }

        for (index, packet) in packets.iter().enumerate() {
            self.send_packet(packet, pacing)
                .map_err(|source| Error::Packet {
                    index,
                    packet: packet.to_string(),
                    source,
                })?;
            progress(index + 1);
        }
        Ok(())
    }

    fn send_packet(&mut self, packet: &Packet, pacing: &Pacing) -> io::Result<()> {
        match packet {
            Packet::Feature {
                payload,
                read_reply,
            } => {
                let mut report = [0; 1 + PAYLOAD_LEN];
                report[1..].copy_from_slice(payload);
                thread::sleep(if *read_reply {
                    pacing.control
                } else {
                    pacing.data
                });
                self.control.set_feature(&report)?;
                if *read_reply {
                    thread::sleep(pacing.reply);
                    self.control.get_feature(&mut report)?;
                }
            }
            Packet::ScreenChunk(chunk) => {
                let screen = self.screen.as_mut().expect("checked in send");
                let mut report = vec![0; 1 + SCREEN_CHUNK_LEN];
                report[1..].copy_from_slice(&chunk[..]);
                screen.write(&report)?;
                screen.read_timeout(&mut report, SCREEN_ACK_TIMEOUT)?;
            }
            Packet::Dongle(report) => {
                self.dongle_exchange(report)?;
                thread::sleep(DONGLE_GAP);
            }
        }
        Ok(())
    }

    /// Writes one report and waits for the reply echoing its first 3 bytes.
    fn dongle_exchange(&mut self, report: &dongle::Report) -> io::Result<Option<dongle::Report>> {
        let mut buf = [0; 1 + dongle::REPORT_LEN];
        buf[1..].copy_from_slice(report);
        self.control.write(&buf)?;
        thread::sleep(DONGLE_AFTER_WRITE);

        for _ in 0..DONGLE_READS {
            if let Some(len) = self.control.read_timeout(&mut buf, DONGLE_READ_TIMEOUT)? {
                // hidraw omits report ID 0 unless the descriptor declares IDs.
                let reply = if len > dongle::REPORT_LEN {
                    &buf[1..]
                } else {
                    &buf[..len]
                };
                if reply.get(..3) == Some(&report[..3]) {
                    let mut out = [0; dongle::REPORT_LEN];
                    out[..reply.len()].copy_from_slice(reply);
                    return Ok(Some(out));
                }
            }
            thread::sleep(DONGLE_READ_RETRY);
        }
        Ok(None)
    }
}
