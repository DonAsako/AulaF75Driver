//! Just enough HID report-descriptor parsing to size reports.

use std::collections::HashMap;

/// Largest report of each kind in bytes, excluding the report ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReportSizes {
    pub input: usize,
    pub output: usize,
    pub feature: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Input,
    Output,
    Feature,
}

#[derive(Clone, Copy, Default)]
struct Globals {
    report_size: u64,
    report_count: u64,
    report_id: u32,
}

pub fn report_sizes(descriptor: &[u8]) -> ReportSizes {
    let mut globals = Globals::default();
    let mut stack = Vec::new();
    let mut bits: HashMap<(Kind, u32), u64> = HashMap::new();

    let mut rest = descriptor;
    while let Some((&prefix, tail)) = rest.split_first() {
        if prefix == 0xFE {
            let len = tail.first().map_or(0, |&n| usize::from(n));
            rest = tail.get(2 + len..).unwrap_or_default();
            continue;
        }
        let len = [0, 1, 2, 4][usize::from(prefix & 0b11)];
        let Some(data) = tail.get(..len) else { break };
        rest = &tail[len..];
        let value = data
            .iter()
            .rev()
            .fold(0u32, |acc, &b| acc << 8 | u32::from(b));

        let kind = match prefix & 0b1111_1100 {
            0x80 => Some(Kind::Input),
            0x90 => Some(Kind::Output),
            0xB0 => Some(Kind::Feature),
            0x74 => {
                globals.report_size = value.into();
                None
            }
            0x94 => {
                globals.report_count = value.into();
                None
            }
            0x84 => {
                globals.report_id = value;
                None
            }
            0xA4 => {
                stack.push(globals);
                None
            }
            0xB4 => {
                globals = stack.pop().unwrap_or_default();
                None
            }
            _ => None,
        };
        if let Some(kind) = kind {
            let total = bits.entry((kind, globals.report_id)).or_default();
            *total = total.saturating_add(globals.report_size.saturating_mul(globals.report_count));
        }
    }

    let largest = |kind| {
        bits.iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|(_, &b)| b.div_ceil(8) as usize)
            .max()
            .unwrap_or(0)
    };
    ReportSizes {
        input: largest(Kind::Input),
        output: largest(Kind::Output),
        feature: largest(Kind::Feature),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // Descriptors read from the 2.4G dongle (05AC:024F), interfaces 0, 3 and 4.

    #[test]
    fn boot_keyboard() {
        let desc = hex(
            "05010906a101050719e029e7150025017501950881029501750881019503750105081901290391029505750191019505750826ff000507190029ff810005ff0903750895018102c0",
        );
        assert_eq!(
            report_sizes(&desc),
            ReportSizes {
                input: 8,
                output: 1,
                feature: 0
            }
        );
    }

    #[test]
    fn vendor_32_byte() {
        let desc = hex("0660ff0961a1010962150026ff009520750881020963150026ff00952075089102c0");
        assert_eq!(
            report_sizes(&desc),
            ReportSizes {
                input: 32,
                output: 32,
                feature: 0
            }
        );
    }

    #[test]
    fn vendor_64_byte() {
        let desc = hex("0659ff0961a1010962150026ff009540750881020963150026ff00954075089102c0");
        assert_eq!(
            report_sizes(&desc),
            ReportSizes {
                input: 64,
                output: 64,
                feature: 0
            }
        );
    }

    #[test]
    fn report_ids_are_sized_separately() {
        // ID 1: 4-byte input and feature. Push, ID 2: 4-byte feature. Pop back
        // to ID 1: another 4 feature bytes.
        let desc = hex("8501750895048102a4b1028502b102b4b102");
        assert_eq!(
            report_sizes(&desc),
            ReportSizes {
                input: 4,
                output: 0,
                feature: 8
            }
        );
    }

    #[test]
    fn truncated_item_stops_parsing() {
        assert_eq!(
            report_sizes(&[0x75, 0x08, 0x96, 0x01]),
            ReportSizes::default()
        );
    }
}
