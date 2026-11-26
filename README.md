# padshell

Drive a Linux desktop with a game controller (DualShock 4, Xbox, or any pad
the kernel has a driver for): bind buttons, chords, and macros to keyboard
keys, use the right stick as a mouse, and control the DS4's lightbar
color/rumble — including animated effects.

A DS4 is read via raw `hidraw` (keeping a path open to touchpad/gyro access
later); Xbox and other gamepads are read via the kernel's `evdev` nodes.
Output is injected via Linux `uinput`, so it works under Wayland/Hyprland
with no XTest dependency.

## Status

DS4 over USB and Bluetooth are both implemented and verified against real
hardware (a CUH-ZCT2x DualShock 4).

Xbox controllers (and any other gamepad with a kernel driver — 8BitDo,
Logitech, Stadia, ...) are read through `/dev/input/event*`. That's how
Xbox pads reach userspace on Linux anyway: over USB the `xpad` driver owns
the device and exposes no hidraw node at all, and over Bluetooth
(`hid-microsoft` or `xpadneo`) evdev is the well-trodden path. Xbox pads
get rumble but no lightbar. The evdev decoding was verified against a
virtual Xbox-shaped uinput device; a physical pad hasn't been tried yet on
this machine, so report anything that decodes oddly (`padshell run
--dump-only --changes-only` shows exactly what's being read).

Every pad uses one shared button vocabulary in profiles, so bindings are
portable between controllers:

| Profile name     | DS4            | Xbox                      |
|------------------|----------------|---------------------------|
| `cross`          | ✕              | A                         |
| `circle`         | ○              | B                         |
| `square`         | ◻              | X                         |
| `triangle`       | △              | Y                         |
| `l1` / `r1`      | L1 / R1        | LB / RB                   |
| `l2` / `r2`      | L2 / R2        | LT / RT                   |
| `l3` / `r3`      | L3 / R3        | LS / RS (stick click)     |
| `share`          | Share          | View / Back               |
| `options`        | Options        | Menu / Start              |
| `ps`             | PS             | Xbox / Guide              |
| `touchpad_click` | Touchpad click | Share (Series pads)       |

The Xbox names (`a`, `lb`, `view`, `guide`, ...) are accepted as aliases
when hand-editing a profile. The GUI labels buttons the way the connected
pad prints them, based on the `kind` the daemon records in each profile.

### Coexisting with RetroArch, Steam and games

padshell never grabs (`EVIOCGRAB`) a controller. It just reads the same
node RetroArch's `udev` joypad driver, SDL, and Steam read, so every
connected pad keeps working in games exactly as before, with padshell's
bindings layered on top. Each controller — two Xbox pads, a DS4 and an
Xbox pad, whatever — gets its own profile and its own worker in the
daemon, so bindings and lightbar state are per-pad.

The one thing to keep in mind: bindings still fire while a game is in the
foreground, so a button bound to a key will also send that key to the game.
Keep desktop-only bindings behind a layer (hold L1 / LB, say), or stop the
daemon while playing, if that gets in the way.

The daemon supports any number of controllers connected at once, each with
its own bindings and lightbar state: it identifies a controller by its
Bluetooth MAC (or, over USB where no such serial exists — DS4 and Xbox
alike — the physical port it's plugged into), loads a profile for that identity from
`~/.config/padshell/devices/`, and keeps watching for controllers appearing
or disappearing rather than exiting when one is unplugged. Reconnecting
(replugging, or a Bluetooth controller waking back up) reapplies that
controller's saved lightbar/animation state automatically.

A QML GUI (`padshell-ui`) is included: a Live Monitor tab (DS4-shaped diagram
showing live button/stick/trigger state), a Bindings tab, and a Haptics tab (lightbar
color, rumble, and animated effects — rainbow/fade/strobe — with a small
persisted library of named presets).

The Bindings tab works like a game's controls screen: the left column lists
every `bind` from your Hyprland config (`~/.config/hypr/hyprland.lua` if it
exists, since Hyprland prefers it, otherwise `hyprland.conf`; the TOML-style
file is parsed directly following `source =` includes and `$variables`, the
Lua file is evaluated with the system `lua` under a stub `hl` API that records
`hl.bind()` calls), grouped by the section comments in that file and given friendly names like "Close window" or
"Go to workspace 3"), every desktop app the wofi launcher shows (bound as a
`launch` action that runs the app's Exec line), plus raw keyboard keys and
mouse buttons. Pick an
action, then press a button on the controller (or click one on the diagram)
to bind it; or click a button first and pick its action. Bindings are stored
as the `key_combo` that triggers the Hyprland bind, so the daemon needs no
Hyprland integration. A layer picker switches which layer you're editing;
chords, layer management and macros live under "Advanced".

The window's top-right corner shows the controller's battery (red and
pulsing at or below the lowest alert threshold, blue while charging). Battery
alerts, edited on the Haptics tab and stored in the controller's profile as
`[[battery_alerts]]`, tell the daemon to switch the lightbar to a chosen
color/animation (and optionally rumble once) when the battery drops to a
threshold while not charging; the profile's normal lightbar state returns once
it's charged back above it.

## Requirements

- `/dev/hidraw*` access to the DS4 (USB or Bluetooth)
- `/dev/uinput` access to create virtual keyboard/mouse devices
- `input` group membership for `/dev/uinput`, the DS4's lightbar LED sysfs
  files, and its force-feedback evdev node

Install `udev/99-padshell.rules` and add yourself to the `input` group (see the
comments in that file) so padshell doesn't need root:

```sh
sudo cp udev/99-padshell.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG input $USER   # log out/in (or `newgrp input`) to take effect
```

## Usage

```sh
cargo build --release

# List detected controllers, how each is read (hidraw/evdev), and which
# profile file each maps to. --all also dumps every hidraw node.
padshell devices

# Decode a raw HID report file without hardware (for testing/debugging the parser)
padshell test-report path/to/report.bin

# Run the daemon
cp config.example.toml ~/.config/padshell/default.toml
padshell run
# Each controller connected gets its own profile, seeded from default.toml,
# at ~/.config/padshell/devices/<device-id>.toml the first time it's seen.

# Log decoded controller state instead of injecting output - useful for
# verifying the HID parser against real hardware before trusting it.
# --changes-only prints a live-updating line plus a log of button changes.
padshell run --dump-only [--changes-only]

# Rumble/lightbar from the CLI (see also the GUI's Haptics tab)
padshell rumble [weak] [strong] [--duration-ms 500]
padshell lightbar <r> <g> <b>

# Battery level (from the kernel's power_supply for the controller);
# --json prints a waybar-style {text, tooltip, percentage, class} object
padshell battery [--json]

# GUI
cargo run --release -p padshell-ui

# Optional: add the GUI to your app launcher (wofi/rofi/etc.). Adjust the
# Exec path if padshell-ui isn't on PATH for your launcher.
cp desktop/padshell-ui.desktop ~/.local/share/applications/
```

Config hot-reloads on save; no need to restart the daemon after editing bindings
(whether you edit the TOML by hand or through the GUI's Bindings tab, which
writes to the same file). A controller upgrading from a pre-multi-profile
setup has its old `~/.config/padshell/config.toml` copied to `default.toml`
automatically on first run.

## Config

See `config.example.toml`. Bindings map a button (or `{ combo = [...] }` chord) to
a `key`, `key_combo`, `macro`, or `mouse_button` action. Layers activate on
hold/toggle of a trigger button and can override bindings and add stick-driven
mouse movement/scroll while active. `mouse_button` bindings support press-and-hold
(drag), not just taps.

## Rumble and lightbar

DS4 output (rumble/lightbar) does **not** go through raw HID output reports on
this system — `hid-playstation` (the in-kernel driver) already owns the device's
output reports for its own bookkeeping, so writing report ID 0x05/0x11 directly
is silently ineffective (verified live). Instead padshell uses the kernel's own
interfaces:

- Lightbar: `/sys/class/leds/<inputNN>:{red,green,blue}/brightness`
- Rumble: the standard evdev force-feedback API on the controller's associated
  `/dev/input/eventN`

See `padshell_hid::HapticsController`. Animated lightbar effects
(`LightbarAnimation::{Rainbow,Fade,Strobe}`) run as a background thread updating
the LED files at ~33fps.

## Extending to a new controller

Anything the kernel exposes as a gamepad (`BTN_SOUTH` + `ABS_X`) already
works through the evdev backend (`crates/hid/src/evdev_pad.rs`); to label
it as its own family rather than "generic", add a `ControllerKind` and
teach `EvdevPadInfo::kind` to recognize it. For a controller that needs
raw HID access (touchpad, gyro), implement `padshell_hid::ControllerReportParser`
for its report layout and add it to `crates/hid/src/controller.rs`.

## systemd

```sh
mkdir -p ~/.config/systemd/user
cp systemd/padshell.service ~/.config/systemd/user/
systemctl --user enable --now padshell
```

Note: while actively editing bindings through the GUI, you may want to stop the
daemon first (`systemctl --user stop padshell`) so test button presses don't
also trigger already-configured keystrokes.
