# padshell

Drive a Linux desktop with a PS4 (DualShock 4) controller: bind buttons, chords,
and macros to keyboard keys, use the right stick as a mouse, and control the
controller's lightbar color/rumble — including animated effects.

Reads the controller via raw `hidraw` (no evdev/SDL2 — this keeps a path open to
touchpad/gyro access later) and injects output via Linux `uinput`, so it works
under Wayland/Hyprland with no XTest dependency.

## Status

DS4 over USB and Bluetooth are both implemented and verified against real
hardware (a CUH-ZCT2x DualShock 4). Other controllers (Xbox) are not
implemented yet — see `ControllerReportParser` for the extension point.

The daemon supports any number of controllers connected at once, each with
its own bindings and lightbar state: it identifies a controller by its
Bluetooth MAC (or, over USB where no such serial exists, the physical port
it's plugged into), loads a profile for that identity from
`~/.config/padshell/devices/`, and keeps watching for controllers appearing
or disappearing rather than exiting when one is unplugged. Reconnecting
(replugging, or a Bluetooth controller waking back up) reapplies that
controller's saved lightbar/animation state automatically.

A QML GUI (`padshell-ui`) is included: a Live Monitor tab (DS4-shaped diagram
showing live button/stick/trigger state), a Bindings tab (click a button on the
diagram to bind it, or edit layers/macros/combos), and a Haptics tab (lightbar
color, rumble, and animated effects — rainbow/fade/strobe — with a small
persisted library of named presets).

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

# List hidraw devices and their vendor/product IDs (find your controller)
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

# GUI
cargo run --release -p padshell-ui
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

Implement `padshell_hid::ControllerReportParser` for the new device's HID report
layout, and register its vendor/product ID alongside `DS4_VENDOR_ID` in
`crates/hid/src/device.rs` / the daemon's device lookup.

## systemd

```sh
mkdir -p ~/.config/systemd/user
cp systemd/padshell.service ~/.config/systemd/user/
systemctl --user enable --now padshell
```

Note: while actively editing bindings through the GUI, you may want to stop the
daemon first (`systemctl --user stop padshell`) so test button presses don't
also trigger already-configured keystrokes.
