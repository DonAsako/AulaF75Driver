//! Desktop interface for the EPOMAKER x AULA F75 Max. The webview in `ui/` calls the
//! commands below. Each opens the keyboard anew, so it can be replugged or
//! switched between USB and the dongle while the app runs.

use std::io::Cursor;
use std::sync::{Mutex, PoisonError};

use aula::layout::{self, Key};
use aula::protocol::{
    self, ClockTime, Debounce, Effect, KeyAction, KeyColors, Keymap, Layer, Level, Lighting,
    MAX_MACROS, MEDIA_KEYS, MIN_DELAY_MS, Macro, Packet, Rgb, Settings,
};
use aula::{Connection, Keyboard};
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter};

/// The CLI's default; what other slots do is unknown.
const SCREEN_SLOT: u8 = 1;

/// One transaction at a time: the keyboard does not interleave them.
static DEVICE: Mutex<()> = Mutex::new(());

type Reply<T> = Result<T, String>;

/// Runs `f` on a blocking thread with the keyboard opened.
async fn with_keyboard<T: Send + 'static>(
    f: impl FnOnce(&mut Keyboard) -> anyhow::Result<T> + Send + 'static,
) -> Reply<T> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _device = DEVICE.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut Keyboard::open()?)
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map_err(|e| format!("{e:#}"))
}

/// Encodes for the keyboard's connection and sends.
async fn write(
    encode: impl FnOnce(Connection) -> anyhow::Result<Vec<Packet>> + Send + 'static,
) -> Reply<()> {
    with_keyboard(move |keyboard| {
        let packets = encode(keyboard.connection())?;
        Ok(keyboard.send(&packets)?)
    })
    .await
}

/// Constants the frontend builds its pages from.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    keys: &'static [Key],
    effects: [Effect; 19],
    media_keys: [(&'static str, u16); 7],
    max_level: u8,
    max_debounce: u8,
    max_macros: usize,
    min_delay_ms: u16,
}

#[tauri::command]
fn info() -> Info {
    Info {
        keys: &layout::KEYS,
        effects: Effect::ALL,
        media_keys: MEDIA_KEYS,
        max_level: Level::MAX,
        max_debounce: Debounce::MAX,
        max_macros: MAX_MACROS,
        min_delay_ms: MIN_DELAY_MS,
    }
}

#[derive(Serialize)]
struct Status {
    connection: Connection,
    /// Percent; only the dongle reports it.
    battery: Option<u8>,
}

#[tauri::command]
async fn status() -> Reply<Status> {
    with_keyboard(|keyboard| {
        let connection = keyboard.connection();
        let battery = match connection {
            Connection::Usb => None,
            Connection::Dongle => Some(keyboard.battery()?),
        };
        Ok(Status {
            connection,
            battery,
        })
    })
    .await
}

#[tauri::command]
async fn set_lighting(lighting: Lighting) -> Reply<()> {
    write(move |connection| Ok(protocol::set_lighting(connection, &lighting))).await
}

fn key_colors(
    connection: Connection,
    brightness: Level,
    colors: Vec<(u8, Rgb)>,
) -> anyhow::Result<Vec<Packet>> {
    let colors = KeyColors::from_slots(colors)?;
    Ok(protocol::set_key_colors(connection, brightness, &colors))
}

fn keymap(
    connection: Connection,
    layer: Layer,
    keys: Vec<(u8, KeyAction)>,
) -> anyhow::Result<Vec<Packet>> {
    let keymap = Keymap::from_slots(keys)?;
    Ok(protocol::set_keymap(connection, layer, &keymap))
}

#[tauri::command]
async fn set_key_colors(brightness: Level, colors: Vec<(u8, Rgb)>) -> Reply<()> {
    write(move |connection| key_colors(connection, brightness, colors)).await
}

#[tauri::command]
async fn read_key_colors() -> Reply<Vec<(u8, Rgb)>> {
    with_keyboard(|keyboard| {
        let colors = keyboard.query(&protocol::read_key_colors())?;
        Ok(colors.iter().collect())
    })
    .await
}

#[tauri::command]
async fn set_keymap(layer: Layer, keys: Vec<(u8, KeyAction)>) -> Reply<()> {
    write(move |connection| keymap(connection, layer, keys)).await
}

#[tauri::command]
async fn set_macros(macros: Vec<Macro>) -> Reply<()> {
    write(move |connection| Ok(protocol::set_macros(connection, &macros)?)).await
}

#[tauri::command]
async fn set_settings(settings: Settings) -> Reply<()> {
    write(move |connection| Ok(protocol::set_settings(connection, &settings))).await
}

/// The frontend sends its local time.
#[tauri::command]
async fn sync_clock(time: ClockTime) -> Reply<()> {
    write(move |connection| Ok(protocol::sync_clock(connection, SCREEN_SLOT, &time))).await
}

/// The lighting last applied: an effect, or per-key colours.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Light {
    Effect {
        lighting: Lighting,
    },
    Keys {
        brightness: Level,
        colors: Vec<(u8, Rgb)>,
    },
}

/// Everything the app last applied, sent again in one go, for a keyboard
/// that was reset or configured elsewhere. The screen animation is not kept.
#[derive(Deserialize)]
struct Snapshot {
    settings: Option<Settings>,
    macros: Option<Vec<Macro>>,
    keymaps: Vec<(Layer, Vec<(u8, KeyAction)>)>,
    light: Option<Light>,
    time: ClockTime,
}

#[tauri::command]
async fn resync(snapshot: Snapshot) -> Reply<()> {
    write(move |connection| {
        let Snapshot {
            settings,
            macros,
            keymaps,
            light,
            time,
        } = snapshot;
        let mut packets = Vec::new();
        if let Some(settings) = settings {
            packets.extend(protocol::set_settings(connection, &settings));
        }
        // Before the keymaps, which refer to macros by index.
        if let Some(macros) = macros {
            packets.extend(protocol::set_macros(connection, &macros)?);
        }
        for (layer, keys) in keymaps {
            packets.extend(keymap(connection, layer, keys)?);
        }
        match light {
            Some(Light::Effect { lighting }) => {
                packets.extend(protocol::set_lighting(connection, &lighting));
            }
            Some(Light::Keys { brightness, colors }) => {
                packets.extend(key_colors(connection, brightness, colors)?);
            }
            None => {}
        }
        packets.extend(protocol::sync_clock(connection, SCREEN_SLOT, &time));
        Ok(packets)
    })
    .await
}

/// The request body is the GIF file: raw bytes, or a JSON array of them when
/// the webview falls back to `postMessage` IPC, as WebKitGTK does. Emits
/// `screen-progress` as `[sent, total]` packets.
#[tauri::command]
async fn upload_screen(app: AppHandle, request: Request<'_>) -> Reply<()> {
    let gif = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(json) => {
            Vec::<u8>::deserialize(json.clone()).map_err(|e| format!("reading the GIF: {e}"))?
        }
    };
    with_keyboard(move |keyboard| {
        let frames = aula::gif::decode(Cursor::new(gif))?;
        let packets = protocol::upload_screen(SCREEN_SLOT, &frames)?;
        keyboard.send_with_progress(&packets, |sent| {
            let _ = app.emit("screen-progress", (sent, packets.len()));
        })?;
        Ok(())
    })
    .await
}

fn main() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            info,
            status,
            set_lighting,
            set_key_colors,
            read_key_colors,
            set_keymap,
            set_macros,
            set_settings,
            sync_clock,
            resync,
            upload_screen,
        ])
        .run(tauri::generate_context!())
}
