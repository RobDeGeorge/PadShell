# padshell

Drive a Linux desktop with a PS4 (DualShock 4) controller: bind buttons, chords,
and macros to keyboard keys, and use the right stick as a mouse.

Reads the controller via raw `hidraw` (no evdev/SDL2 — this keeps a path open to
touchpad/gyro/lightbar access later) and injects output via Linux `uinput`, so it
works under Wayland/Hyprland with no XTest dependency.

## Status

DS4 over USB and Bluetooth are both implemented and verified against real
hardware (a CUH-ZCT2x DualShock 4). The daemon auto-detects whichever transport
is currently connected. Other controllers (Xbox) are not implemented yet — see
`ControllerReportParser` for the extension point.

## Requirements

- `/dev/hidraw*` access to the DS4's USB device
- `/dev/uinput` access to create virtual keyboard/mouse devices

Install `udev/99-padshell.rules` and add yourself to the `input` group (see the
comments in that file) so padshell doesn't need root.

## Usage

```sh
cargo build --release

# List hidraw devices and their vendor/product IDs (find your controller)
padshell devices

# Decode a raw HID report file without hardware (for testing/debugging the parser)
padshell test-report path/to/report.bin

# Run the daemon
cp config.example.toml ~/.config/padshell/config.toml
padshell run

# Log decoded controller state instead of injecting output - useful for
# verifying the HID parser against real hardware before trusting it
padshell run --dump-only
```

Config hot-reloads on save; no need to restart the daemon after editing bindings.

## Config

See `config.example.toml`. Bindings map a button (or `{ combo = [...] }` chord) to
a `key`, `key_combo`, `macro`, or `mouse_button` action. Layers activate on
hold/toggle of a trigger button and can override bindings and add stick-driven
mouse movement/scroll while active.

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
