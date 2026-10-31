use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

mod parse;

use anyhow::{Context, Result};
use aula::protocol::{
    self, ClockTime, Debounce, FnMode, KeyAction, KeyColors, Keymap, Layer, Level, Lighting,
    Packet, Rgb, ScreenFrame, Settings, Sleep,
};
use aula::{Connection, Keyboard, hid, layout};
use chrono::{Datelike, Local, Timelike};
use clap::{Parser, Subcommand};

/// Configure an EPOMAKER x AULA F75 Max keyboard.
#[derive(Parser)]
struct Cli {
    /// Send to the keyboard. Without this, print the reports instead.
    #[arg(long, global = true)]
    send: bool,
    /// Print the dongle's wire format instead of USB's (without --send).
    #[arg(long, global = true)]
    dongle: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List hidraw devices.
    List,
    /// Print the stored per-key colours (USB only, always sent).
    ReadKeys,
    /// Stream a moving rainbow without storing it, until interrupted (USB
    /// only, always sent).
    Rainbow,
    /// Print the battery level (dongle only, always sent).
    Battery,
    #[command(flatten)]
    Write(Write),
}

/// Commands that change the keyboard's configuration.
#[derive(Subcommand)]
enum Write {
    /// Set a global lighting effect.
    Lighting {
        /// Effect name, or "off".
        #[arg(value_parser = parse::effect)]
        effect: parse::EffectArg,
        #[arg(long, value_parser = parse::rgb, default_value = "ffffff")]
        color: Rgb,
        #[arg(long)]
        colorful: bool,
        #[arg(long, value_parser = parse::level, default_value = "5")]
        brightness: Level,
        #[arg(long, value_parser = parse::level, default_value = "3")]
        speed: Level,
        #[arg(long, default_value_t = 0)]
        direction: u8,
    },
    /// Set static per-key colours.
    Keys {
        #[arg(long, value_parser = parse::level, default_value = "5")]
        brightness: Level,
        /// Colour for every key, applied before --key.
        #[arg(long, value_parser = parse::rgb)]
        all: Option<Rgb>,
        /// KEY=RRGGBB, repeatable.
        #[arg(long = "key", value_parser = parse::key_color)]
        keys: Vec<(u8, Rgb)>,
    },
    /// Remap keys. Keys not listed get their default function back.
    Remap {
        /// Remap the Fn layer instead of the base layer.
        #[arg(long = "fn")]
        fn_layer: bool,
        /// KEY=ACTION, KEY being a key name or, for keys without one, a slot
        /// number. ACTION is a key with optional modifiers (`a`, `ctrl+c`,
        /// `win+0x07`), `mouse:NAME` (left right middle button4 button5
        /// double scrollup scrolldown),
        /// `media:NAME` (play stop prev next volup voldown mute),
        /// `consumer:0xUSAGE`, `multi:KEY+KEY[+KEY]`, `macro:N[:MODE]` (once
        /// xCOUNT toggle hold click) or `disabled`.
        #[arg(value_parser = parse::remap)]
        keys: Vec<(u8, KeyAction)>,
    },
    /// Store macros, replacing those on the keyboard. A key plays macro N
    /// (counting from 0) once it is remapped to it.
    Macros {
        /// One macro per argument, as steps: `a` taps a key, `+a` presses it,
        /// `-a` releases it, `mouse:left` (or right, middle) clicks, and `50ms`
        /// sets the pause after the previous step. Example: "+lshift a 30ms -lshift".
        macros: Vec<String>,
    },
    /// Set firmware options. Unspecified options are reset to their defaults.
    Settings {
        #[arg(long)]
        disable_alt_tab: bool,
        #[arg(long)]
        disable_alt_f4: bool,
        #[arg(long)]
        disable_win: bool,
        /// Fn toggles the Fn layer instead of acting while held.
        #[arg(long)]
        fn_toggle: bool,
        /// Idle time before sleeping: never, 1m, 5m or 30m.
        #[arg(long, value_parser = parse::sleep, default_value = "never")]
        sleep: Sleep,
        /// Key debounce level, 1..=5.
        #[arg(long, value_parser = parse::debounce, default_value = "2")]
        debounce: Debounce,
    },
    /// Set the screen clock to local time.
    Clock {
        #[arg(long, default_value_t = 1)]
        slot: u8,
    },
    /// Show a GIF on the screen (USB only). Frames are centre-cropped to a
    /// square and scaled to 128×128.
    Screen {
        gif: PathBuf,
        #[arg(long, default_value_t = 1)]
        slot: u8,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let write = match cli.command {
        Command::List => return list(),
        Command::ReadKeys => return read_keys(),
        Command::Rainbow => return rainbow(),
        Command::Battery => {
            println!("{}%", Keyboard::open()?.battery()?);
            return Ok(());
        }
        Command::Write(write) => write,
    };

    let keyboard = cli.send.then(Keyboard::open).transpose()?;
    let connection = match &keyboard {
        Some(keyboard) => keyboard.connection(),
        None if cli.dongle => Connection::Dongle,
        None => Connection::Usb,
    };
    let packets = encode(write, connection)?;
    match keyboard {
        Some(mut keyboard) => send(&mut keyboard, &packets),
        None => {
            print(&packets);
            Ok(())
        }
    }
}

fn encode(write: Write, connection: Connection) -> Result<Vec<Packet>> {
    Ok(match write {
        Write::Lighting {
            effect,
            color,
            colorful,
            brightness,
            speed,
            direction,
        } => protocol::set_lighting(
            connection,
            &Lighting {
                effect: effect.0,
                color,
                colorful,
                brightness,
                speed,
                direction,
            },
        ),
        Write::Keys {
            brightness,
            all,
            keys,
        } => {
            let fill = all
                .into_iter()
                .flat_map(|color| layout::KEYS.map(|key| (key.slot, color)));
            let colors = KeyColors::from_slots(fill.chain(keys))?;
            protocol::set_key_colors(connection, brightness, &colors)
        }
        Write::Remap { fn_layer, keys } => {
            let keymap = Keymap::from_slots(keys)?;
            let layer = if fn_layer { Layer::Fn } else { Layer::Base };
            protocol::set_keymap(connection, layer, &keymap)
        }
        Write::Macros { macros } => {
            let macros = macros
                .iter()
                .map(|steps| parse::macro_steps(steps))
                .collect::<Result<Vec<_>, _>>()
                .map_err(anyhow::Error::msg)?;
            protocol::set_macros(connection, &macros)?
        }
        Write::Settings {
            disable_alt_tab,
            disable_alt_f4,
            disable_win,
            fn_toggle,
            sleep,
            debounce,
        } => protocol::set_settings(
            connection,
            &Settings {
                disable_alt_tab,
                disable_alt_f4,
                disable_win,
                fn_mode: if fn_toggle {
                    FnMode::Toggle
                } else {
                    FnMode::Momentary
                },
                sleep,
                debounce,
            },
        ),
        Write::Clock { slot } => protocol::sync_clock(connection, slot, &now()),
        Write::Screen { gif, slot } => protocol::upload_screen(slot, &load_gif(&gif)?)?,
    })
}

/// Shows progress on stderr for transfers long enough to need it.
fn send(keyboard: &mut Keyboard, packets: &[Packet]) -> Result<()> {
    const SHOW_PROGRESS_FROM: usize = 50;
    if packets.len() < SHOW_PROGRESS_FROM {
        return Ok(keyboard.send(packets)?);
    }
    let start = Instant::now();
    keyboard.send_with_progress(packets, |sent| {
        eprint!("\r{sent}/{} packets", packets.len());
    })?;
    let elapsed = start.elapsed();
    eprintln!(
        " in {:.1} s ({} ms per packet)",
        elapsed.as_secs_f64(),
        elapsed.as_millis() / packets.len() as u128
    );
    Ok(())
}

fn read_keys() -> Result<()> {
    let colors = Keyboard::open()?.query(&protocol::read_key_colors())?;
    for key in layout::KEYS {
        let Rgb { r, g, b } = colors.get(key.slot).unwrap_or_default();
        println!("{:<10} {r:02x}{g:02x}{b:02x}", key.name);
    }
    Ok(())
}

/// Hue follows the column, `(slot - 1) % 18` in the main block.
fn rainbow() -> Result<()> {
    let mut keyboard = Keyboard::open()?;
    for frame in 0u32.. {
        let mut colors = KeyColors::default();
        for key in layout::KEYS {
            let column = u32::from(key.slot - 1) % 18;
            colors.set(key.slot, hue((column * 20 + frame * 6) % 360))?;
        }
        keyboard.stream(&protocol::stream_key_colors(&colors)?)?;
    }
    Ok(())
}

/// Fully saturated colour at `degrees` on the colour wheel.
fn hue(degrees: u32) -> Rgb {
    let x = ((degrees % 60) * 255 / 60) as u8;
    let (r, g, b) = match degrees / 60 {
        0 => (255, x, 0),
        1 => (255 - x, 255, 0),
        2 => (0, 255, x),
        3 => (0, 255 - x, 255),
        4 => (x, 0, 255),
        _ => (255, 0, 255 - x),
    };
    Rgb { r, g, b }
}

fn list() -> Result<()> {
    for device in hid::enumerate()? {
        let sizes = device.report_sizes;
        let interface = device.interface.map_or("-".into(), |i| i.to_string());
        println!(
            "{}  {:04x}:{:04x}  if {interface:>2}  in {:>4}  out {:>4}  feature {:>4}  {}",
            device.path.display(),
            device.vendor_id,
            device.product_id,
            sizes.input,
            sizes.output,
            sizes.feature,
            device.name,
        );
    }
    Ok(())
}

fn print(packets: &[Packet]) {
    let same_kind =
        |a: &Packet, b: &Packet| matches!((a, b), (Packet::ScreenChunk(_), Packet::ScreenChunk(_)));
    for run in packets.chunk_by(same_kind) {
        match run {
            [Packet::ScreenChunk(_), ..] => println!("{} × {}", run[0], run.len()),
            _ => run.iter().for_each(|packet| println!("{packet}")),
        }
    }
}

fn now() -> ClockTime {
    let now = Local::now();
    ClockTime {
        year: now.year() as u16,
        month: now.month() as u8,
        day: now.day() as u8,
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
        weekday: now.weekday().num_days_from_sunday() as u8,
    }
}

fn load_gif(path: &Path) -> Result<Vec<ScreenFrame>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(aula::gif::decode(BufReader::new(file))?)
}
