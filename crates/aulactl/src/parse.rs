//! Command-line value parsers.

use aula::layout::{self, Key};
use aula::protocol::{
    Debounce, Effect, KeyAction, Level, MEDIA_KEYS, MIN_DELAY_MS, Macro, MacroEvent, MacroPlay,
    MacroStep, MouseAction, MouseButton, Rgb, Sleep, modifier,
};

pub fn rgb(s: &str) -> Result<Rgb, String> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("expected RRGGBB, got {s:?}"));
    }
    let [_, r, g, b] = u32::from_str_radix(hex, 16).unwrap().to_be_bytes();
    Ok(Rgb { r, g, b })
}

pub fn debounce(s: &str) -> Result<Debounce, String> {
    s.parse::<u8>().map_err(|e| e.to_string())?.try_into()
}

pub fn sleep(s: &str) -> Result<Sleep, String> {
    Ok(match s {
        "never" => Sleep::Never,
        "1m" => Sleep::OneMinute,
        "5m" => Sleep::FiveMinutes,
        "30m" => Sleep::ThirtyMinutes,
        _ => return Err(format!("expected never, 1m, 5m or 30m, got {s:?}")),
    })
}

pub fn level(s: &str) -> Result<Level, String> {
    s.parse::<u8>().map_err(|e| e.to_string())?.try_into()
}

/// `None` is "off". A bare `Option` would make clap treat the argument as optional.
#[derive(Clone)]
pub struct EffectArg(pub Option<Effect>);

pub fn effect(s: &str) -> Result<EffectArg, String> {
    if s.eq_ignore_ascii_case("off") {
        return Ok(EffectArg(None));
    }
    Effect::ALL
        .into_iter()
        .find(|effect| effect.to_string().eq_ignore_ascii_case(s))
        .map(|effect| EffectArg(Some(effect)))
        .ok_or_else(|| {
            let names: Vec<_> = Effect::ALL.iter().map(Effect::to_string).collect();
            format!(
                "unknown effect {s:?}; known effects: off {}",
                names.join(" ")
            )
        })
}

/// `KEY=RRGGBB`, see [`slot`].
pub fn key_color(s: &str) -> Result<(u8, Rgb), String> {
    let (name, color) = s.split_once('=').ok_or("expected KEY=RRGGBB")?;
    Ok((slot(name)?, rgb(color)?))
}

/// `KEY=ACTION`, see [`slot`] and [`action`].
pub fn remap(s: &str) -> Result<(u8, KeyAction), String> {
    let (name, action_spec) = s.split_once('=').ok_or("expected KEY=ACTION")?;
    Ok((slot(name)?, action(action_spec)?))
}

/// A key name, or a raw slot number for slots outside the known layout. Names
/// win, so `1` is the 1 key, not slot 1.
fn slot(s: &str) -> Result<u8, String> {
    key(s)
        .map(|key| key.slot)
        .or_else(|e| s.parse().map_err(|_| e))
}

/// One of:
/// - a key with optional modifiers: `a`, `ctrl+c`, `ctrl+shift+0x46`
/// - `mouse:NAME`, NAME in left right middle button4 button5 double scrollup scrolldown
/// - `media:NAME`, NAME in play stop prev next volup voldown mute
/// - `consumer:0xUSAGE`, any HID consumer usage
/// - `multi:KEY+KEY[+KEY]`, keys pressed together
/// - `macro:N[:MODE]`, MODE in once (default) xCOUNT toggle hold click
/// - `disabled`
pub fn action(s: &str) -> Result<KeyAction, String> {
    if s == "disabled" {
        return Ok(KeyAction::Disabled);
    }
    if let Some(keys) = s.strip_prefix("multi:") {
        let usages = keys.split('+').map(usage).collect::<Result<Vec<_>, _>>()?;
        let mut three = [0; 3];
        if usages.len() > three.len() {
            return Err(format!("at most 3 keys in {s:?}"));
        }
        three[..usages.len()].copy_from_slice(&usages);
        return Ok(KeyAction::MultiKey(three));
    }
    if let Some(spec) = s.strip_prefix("macro:") {
        let (index, mode) = spec.split_once(':').unwrap_or((spec, "once"));
        let index = index
            .parse()
            .map_err(|_| format!("bad macro index in {s:?}"))?;
        return Ok(KeyAction::Macro {
            index,
            play: macro_play(mode)?,
        });
    }
    if let Some(name) = s.strip_prefix("mouse:") {
        return mouse_action(name).map(KeyAction::Mouse);
    }
    if let Some(name) = s.strip_prefix("media:") {
        return media_usage(name).map(KeyAction::Consumer);
    }
    if let Some(usage) = s.strip_prefix("consumer:") {
        return hex(usage)
            .and_then(|usage| u16::try_from(usage).ok())
            .map(KeyAction::Consumer)
            .ok_or_else(|| format!("expected consumer:0xUSAGE, got {s:?}"));
    }

    let mut parts: Vec<_> = s.split('+').collect();
    let last = parts.pop().unwrap_or_default();
    let modifiers = parts
        .into_iter()
        .map(modifier_bit)
        .try_fold(0, |bits, bit| bit.map(|bit| bits | bit))?;
    Ok(KeyAction::Key {
        modifiers,
        usage: usage(last)?,
    })
}

/// Whitespace-separated steps: `a` taps a key, `+a` presses it, `-a` releases
/// it; `mouse:left`, `+mouse:left`, `-mouse:left` do the same with a mouse
/// button; `50ms` sets the pause after the previous step (default
/// [`MIN_DELAY_MS`]).
pub fn macro_steps(s: &str) -> Result<Macro, String> {
    let mut steps: Macro = Vec::new();
    for token in s.split_whitespace() {
        if let Some(ms) = token.strip_suffix("ms") {
            let delay_ms = ms.parse().map_err(|_| format!("bad delay {token:?}"))?;
            let last = steps
                .last_mut()
                .ok_or_else(|| format!("delay {token:?} before any event"))?;
            last.delay_ms = delay_ms;
            continue;
        }
        let (press, release, name) = match token.as_bytes()[0] {
            b'+' => (true, false, &token[1..]),
            b'-' => (false, true, &token[1..]),
            _ => (true, true, token),
        };
        let (down, up) = match name.strip_prefix("mouse:") {
            Some(button) => {
                let button = mouse_button(button)?;
                (MacroEvent::MouseDown(button), MacroEvent::MouseUp(button))
            }
            None => {
                let usage = usage(name)?;
                (MacroEvent::KeyDown(usage), MacroEvent::KeyUp(usage))
            }
        };
        let events = [press.then_some(down), release.then_some(up)];
        steps.extend(events.into_iter().flatten().map(|event| MacroStep {
            event,
            delay_ms: MIN_DELAY_MS,
        }));
    }
    Ok(steps)
}

fn key(name: &str) -> Result<Key, String> {
    layout::by_name(name).ok_or_else(|| {
        let names: Vec<_> = layout::KEYS.iter().map(|k| k.name).collect();
        format!("unknown key {name:?}; known keys: {}", names.join(" "))
    })
}

/// A key name from the layout or a raw `0xUSAGE`.
fn usage(s: &str) -> Result<u8, String> {
    match hex(s) {
        Some(usage) => u8::try_from(usage).map_err(|_| format!("usage {s} is over 0xFF")),
        None => key(s).map(|key| key.usage),
    }
}

fn hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

fn modifier_bit(name: &str) -> Result<u8, String> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "ctrl" | "lctrl" => modifier::LEFT_CTRL,
        "shift" | "lshift" => modifier::LEFT_SHIFT,
        "alt" | "lalt" => modifier::LEFT_ALT,
        "win" | "lwin" => modifier::LEFT_GUI,
        "rctrl" => modifier::RIGHT_CTRL,
        "rshift" => modifier::RIGHT_SHIFT,
        "ralt" => modifier::RIGHT_ALT,
        "rwin" => modifier::RIGHT_GUI,
        _ => return Err(format!("unknown modifier {name:?}")),
    })
}

fn mouse_action(name: &str) -> Result<MouseAction, String> {
    Ok(match name {
        "left" => MouseAction::Left,
        "right" => MouseAction::Right,
        "middle" => MouseAction::Middle,
        "button4" => MouseAction::Button4,
        "button5" => MouseAction::Button5,
        "double" => MouseAction::DoubleClick,
        "scrollup" => MouseAction::ScrollUp,
        "scrolldown" => MouseAction::ScrollDown,
        _ => return Err(format!("unknown mouse action {name:?}")),
    })
}

fn macro_play(mode: &str) -> Result<MacroPlay, String> {
    Ok(match mode {
        "once" => MacroPlay::Once,
        "toggle" => MacroPlay::UntilPressedAgain,
        "hold" => MacroPlay::WhileHeld,
        "click" => MacroPlay::Click,
        _ => mode
            .strip_prefix('x')
            .and_then(|count| count.parse().ok())
            .map(MacroPlay::Times)
            .ok_or_else(|| format!("unknown macro mode {mode:?}"))?,
    })
}

fn mouse_button(name: &str) -> Result<MouseButton, String> {
    Ok(match name {
        "left" => MouseButton::Left,
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => return Err(format!("unknown mouse button {name:?}")),
    })
}

fn media_usage(name: &str) -> Result<u16, String> {
    MEDIA_KEYS
        .into_iter()
        .find_map(|(media, usage)| (media == name).then_some(usage))
        .ok_or_else(|| format!("unknown media key {name:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions() {
        assert_eq!(
            action("ctrl+shift+c"),
            Ok(KeyAction::Key {
                modifiers: modifier::LEFT_CTRL | modifier::LEFT_SHIFT,
                usage: 0x06
            })
        );
        assert_eq!(
            action("0x68"),
            Ok(KeyAction::Key {
                modifiers: 0,
                usage: 0x68
            })
        );
        assert_eq!(action("media:mute"), Ok(KeyAction::Consumer(0xE2)));
        assert_eq!(action("consumer:0x224"), Ok(KeyAction::Consumer(0x224)));
        assert!(action("hyper+a").is_err());
        assert_eq!(
            action("multi:a+b"),
            Ok(KeyAction::MultiKey([0x04, 0x05, 0]))
        );
        assert_eq!(
            action("macro:2:x3"),
            Ok(KeyAction::Macro {
                index: 2,
                play: MacroPlay::Times(3.try_into().unwrap())
            })
        );
        assert!(action("macro:2:x0").is_err());
    }

    #[test]
    fn slots() {
        assert_eq!(slot("esc"), Ok(1));
        assert_eq!(slot("1"), Ok(20));
        assert_eq!(slot("142"), Ok(142));
        assert!(slot("nope").is_err());
    }

    #[test]
    fn macros() {
        let steps = macro_steps("+lshift a 30ms -lshift").unwrap();
        let events: Vec<_> = steps.iter().map(|s| (s.event, s.delay_ms)).collect();
        assert_eq!(
            events,
            [
                (MacroEvent::KeyDown(0xE1), MIN_DELAY_MS),
                (MacroEvent::KeyDown(0x04), MIN_DELAY_MS),
                (MacroEvent::KeyUp(0x04), 30),
                (MacroEvent::KeyUp(0xE1), MIN_DELAY_MS),
            ]
        );
        assert!(macro_steps("10ms a").is_err());
    }
}
