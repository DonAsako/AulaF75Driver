//! Linux hidraw devices.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::descriptor::{ReportSizes, report_sizes};

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// USB interface number; `None` for non-USB devices.
    pub interface: Option<u8>,
    pub report_sizes: ReportSizes,
}

/// Devices that cannot be read or parsed are skipped.
pub fn enumerate() -> io::Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir("/sys/class/hidraw")? {
        let entry = entry?;
        if let Ok(device) = describe(&entry.path(), entry.file_name().to_string_lossy().as_ref()) {
            devices.push(device);
        }
    }
    devices.sort_by_key(|d| {
        let node = d.path.to_str()?.strip_prefix("/dev/hidraw")?;
        node.parse::<u32>().ok()
    });
    Ok(devices)
}

fn describe(sys_node: &Path, node_name: &str) -> io::Result<DeviceInfo> {
    let hid_dir = fs::canonicalize(sys_node.join("device"))?;
    let uevent = fs::read_to_string(hid_dir.join("uevent"))?;
    let field = |key: &str| {
        uevent
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or_default()
    };
    // HID_ID is "bus:vendor:product" in hex.
    let mut id = field("HID_ID")
        .split(':')
        .skip(1)
        .map(|part| u16::from_str_radix(part, 16));
    let (Some(Ok(vendor_id)), Some(Ok(product_id))) = (id.next(), id.next()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad HID_ID in {}", hid_dir.display()),
        ));
    };

    let interface = hid_dir
        .parent()
        .and_then(|dir| fs::read_to_string(dir.join("bInterfaceNumber")).ok())
        .and_then(|n| u8::from_str_radix(n.trim(), 16).ok());

    Ok(DeviceInfo {
        path: Path::new("/dev").join(node_name),
        name: field("HID_NAME").to_owned(),
        vendor_id,
        product_id,
        interface,
        report_sizes: report_sizes(&fs::read(hid_dir.join("report_descriptor"))?),
    })
}

/// An open hidraw node. Report buffers start with the report ID.
pub struct Hidraw(File);

impl Hidraw {
    pub fn open(path: &Path) -> io::Result<Self> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(Self)
    }

    pub fn set_feature(&self, report: &[u8]) -> io::Result<()> {
        self.ioctl(HIDIOCSFEATURE, report.as_ptr().cast_mut(), report.len())
    }

    pub fn get_feature(&self, report: &mut [u8]) -> io::Result<()> {
        self.ioctl(HIDIOCGFEATURE, report.as_mut_ptr(), report.len())
    }

    pub fn write(&mut self, report: &[u8]) -> io::Result<()> {
        self.0.write_all(report)
    }

    /// Returns `Ok(None)` if no report arrives within `timeout`.
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
        let mut poll = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = timeout.as_millis().try_into().unwrap_or(libc::c_int::MAX);
        // SAFETY: `poll` is a valid pollfd and we pass a count of 1.
        match unsafe { libc::poll(&mut poll, 1, millis) } {
            -1 => Err(io::Error::last_os_error()),
            0 => Ok(None),
            _ => self.0.read(buf).map(Some),
        }
    }

    fn ioctl(&self, nr: u8, buf: *mut u8, len: usize) -> io::Result<()> {
        // SAFETY: the request encodes `len`, and `buf` is valid for `len` bytes.
        let ret = unsafe { libc::ioctl(self.0.as_raw_fd(), hidioc(nr, len), buf) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

const HIDIOCSFEATURE: u8 = 0x06;
const HIDIOCGFEATURE: u8 = 0x07;

/// `_IOC(_IOC_READ | _IOC_WRITE, 'H', nr, len)` from `linux/hidraw.h`.
fn hidioc(nr: u8, len: usize) -> libc::Ioctl {
    const READ_WRITE: usize = 3;
    (READ_WRITE << 30 | len << 16 | usize::from(b'H') << 8 | usize::from(nr)) as libc::Ioctl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_numbers_match_kernel_headers() {
        assert_eq!(hidioc(HIDIOCSFEATURE, 65) as u64, 0xC041_4806);
        assert_eq!(hidioc(HIDIOCGFEATURE, 65) as u64, 0xC041_4807);
    }
}
