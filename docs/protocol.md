# EPOMAKER x AULA F75 Max HID protocol

This file describes what `crates/aula` sends. `[hw]` marks what was confirmed on a keyboard;
the rest is implemented but untested. `[?]` marks a byte whose meaning is unknown.

## Devices

| Connection | USB ID | Interface | in / out / feature (bytes) | Role |
|---|---|---|---|---|
| USB | `0C45:800A` | 0 | 8 / 1 / 0 | boot keyboard |
| | | 1 | 15 / 0 / 0 | consumer, system, NKRO |
| | | 2 | 64 / **4096** / 0 | screen data |
| | | 3 | 64 / 64 / **64** | control (the only interface with feature reports) |
| 2.4G dongle | `05AC:024F` | 3 | 32 / 32 / 0 | control |
| | | 4 | 64 / 64 / 0 | not used by the driver |

The dongle reports an Apple keyboard's ID. `SET_FEATURE` on its interface 3 fails with
`EPIPE`; it takes output reports instead, see "2.4G dongle" below.

## Transport (USB)

Commands go out with `HIDIOCSFEATURE`, 65 bytes: report ID `0x00` and a 64-byte payload.
Replies come back with `HIDIOCGFEATURE` in the same layout. The screen data is the exception:
4097-byte `write()` calls (report ID 0 and 4096 data bytes) on interface 2, each followed by a
`read()` of the acknowledgement with a 300 ms timeout.

Pacing: 35 ms before every report of a stored command. The real-time stream (`20`) uses 5 ms
before each control report and its reply, and 2 ms between data reports.

Reply byte 3 [?]: `0xFF` looks like "busy, send again", `0x01` like "more data follows".

## Framing

Payload bytes below are the 64 bytes **after** the report ID.

Control report: `04 <cmd> 00 00 00 00 00 00 <n> 00 ...`, byte 8 = number of 64-byte data
reports that follow (where applicable).

```
04 18                 begin
04 <cmd> ... [8]=n    command header
<n data reports>      most single-report payloads end with AA 55, position varies
04 02                 end / apply
04 F0                 after keymap and lighting [?] probably save
```

## Commands

| cmd | Purpose | Data |
|---|---|---|
| `18` | begin transaction | |
| `02` | end / apply | |
| `F0` | [?] probably save to flash | |
| `13` | lighting mode [hw] | 1 report, see below |
| `13` | switch to per-key mode [hw] | `[0]=0x80, [9]=brightness`, `AA 55` @14 |
| `23` | per-key static colours [hw] | header `[8]=9`; 576 B: slot *i* is `[i, R, G, B]` at `4*i`; `AA 55` @574 |
| `20` | real-time per-key colours [hw] | header `[8]=8`; 512 B of `[slot, R, G, B]` x 128; then `04 02` |
| `F5` | read per-key colours [hw] | header `[8]=9`; reply 576 B `[slot, R, G, B]` |
| `11` / `27` | keymap base / Fn layer [hw] | header `[8]=9`; 576 B = 144 slots x 4 B (below) |
| `19`+`15` | macros [hw] | header `[8]=n`; n x 64 B (below) |
| `17` | settings [hw for disable Win] | header `[2]=1, [8]=1`; data below |
| `28` | clock [hw] | header `[8]=1`; data below |
| `72` | screen GIF [hw] | header + 4 KiB bulk chunks (below) |

### Lighting mode (`04 13`, data report)

```
[0] mode   [1] R  [2] G  [3] B  [4..7] 0
[8] colorful  [9] brightness  [10] speed  [11] direction  [12..13] 0  [14..15] AA 55
```

Brightness and speed `0..=5`. Mode `0` is off; modes 1-3 checked on hardware:

| mode | effect | mode | effect | mode | effect | mode | effect |
|---|---|---|---|---|---|---|---|
| 1 | Static | 6 | Colourful | 11 | Rolling | 16 | Flowing |
| 2 | SingleOn | 7 | Breath | 12 | Rotating | 17 | Pulsating |
| 3 | SingleOff | 8 | Spectrum | 13 | Explode | 18 | Tilt |
| 4 | Glittering | 9 | Outward | 14 | Launch | 19 | Shuttle |
| 5 | Falling | 10 | Scrolling | 15 | Ripples | | |

The keyboard's screen labels them differently [hw]: modes 11-19 show "Effect 1" to "Effect 9",
modes 1-10 show "Effect 10" to "Effect 19". Which `direction` values exist is unknown; the
driver sends 0.

### Key slots

Main block: slot = row x 18 + column + 1. Then Backspace 103, Up 101, PgUp 118, Del 119,
End 120, PgDn 121. `crates/aula/src/layout.rs` has every key. Per-key buffers are indexed by
slot (`4 * slot`).

### Keymap (`04 11` base layer, `04 27` Fn layer)

`04 18`, header `[8]=9`, 576-byte per-slot block (`AA 55` @574), `04 02`, `04 F0`.
A zero entry is the key's default function.

| entry | action |
|---|---|
| `00 00 00 00` | default [hw] |
| `05 03 00 00` | disabled |
| `02 mods usage 00` | key + HID modifier bits; a lone modifier usage E0-E7 is sent as its bit in `mods` |
| `06 index mode count` | macro `index`; mode 0 once, 1 `count` times (1-255), 2 until pressed again, 3 while held, 4 "click to play" [?] |
| `01 01 btn 00` | mouse buttons: left 01, right 02, middle 04, 08, 10; double click `01 01 03` |
| `01 03 dir 00` | wheel: up `01`, down `FF` |
| `03 lo hi 00` | consumer usage (u16 LE): `CD` play/pause, `B7` stop, `B6` prev, `B5` next, `E9` vol+, `EA` vol-, `E2` mute |
| `07 a b c` | three keys pressed together (HID usages) |
| `05 02 xx 00` | [?] |

### Macros (`04 15`)

`04 19` (not `18`), header `04 15 [8]=n`, n data reports, `04 02`; no `F0`. 30 ms pause after
the header and after the data.

- Bytes `0..399`: 100 entries of 4 bytes: `u16 LE offset` of the macro, `FF FF FF FF` if it
  has no events, zero past the last macro.
- At `offset`: `u16 LE` record count, 6 zero bytes, then 4-byte records. Each event is followed
  by a delay record:
  - key down `[0,0,usage,B0]`, key up `[0,0,usage,30]`
  - mouse down `[0,0,btn,90]`, mouse up `[0,0,btn,10]`, btn = left 1, middle 4, right 2
  - delay `[lo,hi,0,50]`, at least 10 ms
- Next macro starts at `offset + 8 + 4 * count`.
- n = `len/64 + 1` if `len` is a multiple of 64, else `len/64 + 2` (always a spare report);
  `AA 55` ends the last report. At most `0xE00` bytes.

### Settings (`04 17`)

`04 18`, header `04 17` with `[2]=1` [?] and `[8]=1`, one data report, `04 02`; no `F0`.

```
[0]=00 [1]=01 [2] disable Alt+Tab [3] disable Alt+F4 [4] disable Win (0/1)
[5] Fn mode: 0 momentary, 1 toggle   [6] sleep: 0 never, 1 = 1 min, 2 = 5 min, 3 = 30 min
[7]=0 [8] debounce level 1-5 (default 2)   [62..63] AA 55
```

### Real-time per-key colours (`04 20`)

No `18`/`F0`. Header `04 20 [8]=8`, 512 bytes of `[slot, R, G, B]` packed in key order (not at
`4 * slot`), `04 02`. Meant to be sent repeatedly; `F5` reads the streamed colours back [hw],
so they land in the same per-key memory as `23`.

### Read per-key colours (`04 F5`)

Send `04 F5 [8]=9`, read a status report whose byte 3 must be `01`, then read 9 x 64 bytes
(`[?, R, G, B]` at `4 * slot`), then send `04 02`. USB only.
Colours read back scaled [hw]: `ff0000` written with brightness 5 reads `a80000`.

### Clock (`04 28` data)

```
[0]=00 [1]=screen slot (1-based) [2]=5A [3]=year-2000 [4]=month [5]=day [6]=hour [7]=min [8]=sec
[9]=0 [10]=day of week (0 = Sunday)   ... [62..63] AA 55
```

### Screen GIF (`04 72`), USB only

Screen 128x128.

1. `04 18`
2. `04 72 <slot> 00 00 00 00 00 <chunks lo> <chunks hi>`
3. *chunks* x 4096-byte **output** reports on interface 2, each followed by a read of the
   acknowledgement [hw: it arrives after ~140 ms per chunk, so a 61-frame GIF takes ~70 s]
4. `04 02`

Image blob, padded with `0xFF` to a 4 KiB multiple:
```
[0]          frame count (at most 255)
[1..n]       per-frame delay = max(1, delay / 2) in 2 ms ticks [hw]
[256 ...]      frames, RGB565 little-endian [hw], 128 x 128 x 2 = 32768 bytes each
```

## 2.4G dongle

The control interface is the one with 32-byte input and output reports (interface 3). A packet
is one 33-byte output report: report ID 0, then `d[1..31]`, then `d[32]` = sum of `d[0..31]`
mod 256. After the write: 5 ms pause, then up to 20 reads (5 ms timeout, 10 ms apart) for an
input report echoing `d[1..3]`. The echo does not always come (none for the first macro chunk
[hw]), so its absence is not an error. A failed write is retried once after 10 ms. Bulk uploads
pause 2 ms between packets. There are no begin/end/save packets.

Single-packet commands carry the first 14 bytes of the USB data report at `d[4..17]` and
`AA 55` at `d[18..19]`, after `d[1..3] = <cmd> 10 00`:

| cmd | USB equivalent |
|---|---|
| `05` | lighting `13` [hw]; also the per-key mode switch (mode `80`) before `14` |
| `07` | settings `17` |
| `0C` | clock `28` |

Bulk commands split a block into 28-byte chunks, `d[1..3] = <cmd> <length> <index>`:

| cmd | Block |
|---|---|
| `10` / `12` | keymap base / Fn layer, 576 bytes as over USB (last chunk 16 bytes, padded with zeros) |
| `14` | per-key colours, 576 bytes as over USB |
| `09` | macros: blob as over USB, padded with a spare chunk, `AA 55` at the end; the last chunk declares `len % 28` |

`20 01 00` reads the battery: reply byte 3 = percent [hw].
Not available over 2.4G: the screen, the real-time stream `20`, colour read-back `F5`.

## Open questions

1. `04 F0` (save to flash?), reply byte 3, `04 17` header byte 2.
2. Keymap entries `05 02 xx`, and what macro mode 4 ("click to play") changes.
3. Why colours read back scaled (`ff` reads `a8`).
4. The lighting `direction` values.
