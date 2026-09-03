//! Virtual keyboard and mouse devices via /dev/uinput.

use std::fs::{File, OpenOptions};

use input_linux::{
    EventKind, InputId, Key, KeyEvent, KeyState, RelativeAxis, RelativeEvent,
    SynchronizeEvent, UInputHandle,
};
use padshell_core::MouseButton;
use thiserror::Error;

use crate::keymap;

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("failed to open /dev/uinput: {0}")]
    OpenUinput(std::io::Error),
    #[error("uinput ioctl failed: {0}")]
    Ioctl(std::io::Error),
    #[error("unknown key name: {0}")]
    UnknownKey(String),
}

fn open_uinput() -> Result<File, OutputError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .map_err(OutputError::OpenUinput)
}

pub struct VirtualKeyboard {
    handle: UInputHandle<File>,
}

impl VirtualKeyboard {
    pub fn create() -> Result<Self, OutputError> {
        let handle = UInputHandle::new(open_uinput()?);
        handle
            .set_evbit(EventKind::Key)
            .map_err(OutputError::Ioctl)?;
        for key in keymap::all_supported_keys() {
            handle.set_keybit(key).map_err(OutputError::Ioctl)?;
        }

        let id = InputId {
            bustype: input_linux::sys::BUS_USB,
            vendor: 0x1209,  // pid.codes shared VID for hobbyist projects
            product: 0x0001,
            version: 1,
        };
        handle
            .create(&id, b"padshell virtual keyboard", 0, &[])
            .map_err(OutputError::Ioctl)?;

        Ok(VirtualKeyboard { handle })
    }

    pub fn key(&self, name: &str) -> Result<Key, OutputError> {
        keymap::lookup(name).ok_or_else(|| OutputError::UnknownKey(name.to_string()))
    }

    pub fn key_down(&self, key: Key) -> Result<(), OutputError> {
        self.emit_key(key, KeyState::PRESSED)
    }

    pub fn key_up(&self, key: Key) -> Result<(), OutputError> {
        self.emit_key(key, KeyState::RELEASED)
    }

    /// Presses all keys in order, then releases them in reverse - used
    /// for both single-key taps (one-element slice) and chords/macro
    /// steps.
    pub fn tap(&self, keys: &[Key]) -> Result<(), OutputError> {
        for &key in keys {
            self.key_down(key)?;
        }
        for &key in keys.iter().rev() {
            self.key_up(key)?;
        }
        Ok(())
    }

    fn emit_key(&self, key: Key, state: KeyState) -> Result<(), OutputError> {
        let events = [
            *KeyEvent::new(input_linux::EventTime::default(), key, state).as_ref(),
            *SynchronizeEvent::report(input_linux::EventTime::default()).as_ref(),
        ];
        self.handle.write(&events).map_err(OutputError::Ioctl)?;
        Ok(())
    }
}

pub struct VirtualMouse {
    handle: UInputHandle<File>,
}

impl VirtualMouse {
    pub fn create() -> Result<Self, OutputError> {
        let handle = UInputHandle::new(open_uinput()?);
        handle
            .set_evbit(EventKind::Key)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_keybit(Key::ButtonLeft)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_keybit(Key::ButtonRight)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_keybit(Key::ButtonMiddle)
            .map_err(OutputError::Ioctl)?;

        handle
            .set_evbit(EventKind::Relative)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_relbit(RelativeAxis::X)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_relbit(RelativeAxis::Y)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_relbit(RelativeAxis::Wheel)
            .map_err(OutputError::Ioctl)?;
        handle
            .set_relbit(RelativeAxis::HorizontalWheel)
            .map_err(OutputError::Ioctl)?;

        let id = InputId {
            bustype: input_linux::sys::BUS_USB,
            vendor: 0x1209,
            product: 0x0002,
            version: 1,
        };
        handle
            .create(&id, b"padshell virtual mouse", 0, &[])
            .map_err(OutputError::Ioctl)?;

        Ok(VirtualMouse { handle })
    }

    pub fn move_relative(&self, dx: f32, dy: f32) -> Result<(), OutputError> {
        let mut events = Vec::new();
        let dx = dx.round() as i32;
        let dy = dy.round() as i32;
        if dx != 0 {
            events.push(
                *RelativeEvent::new(input_linux::EventTime::default(), RelativeAxis::X, dx)
                    .as_ref(),
            );
        }
        if dy != 0 {
            events.push(
                *RelativeEvent::new(input_linux::EventTime::default(), RelativeAxis::Y, dy)
                    .as_ref(),
            );
        }
        if events.is_empty() {
            return Ok(());
        }
        events.push(*SynchronizeEvent::report(input_linux::EventTime::default()).as_ref());
        self.handle.write(&events).map_err(OutputError::Ioctl)?;
        Ok(())
    }

    pub fn scroll(&self, dx: f32, dy: f32) -> Result<(), OutputError> {
        let mut events = Vec::new();
        let dx = dx.round() as i32;
        let dy = dy.round() as i32;
        if dx != 0 {
            events.push(
                *RelativeEvent::new(
                    input_linux::EventTime::default(),
                    RelativeAxis::HorizontalWheel,
                    dx,
                )
                .as_ref(),
            );
        }
        if dy != 0 {
            events.push(
                *RelativeEvent::new(
                    input_linux::EventTime::default(),
                    RelativeAxis::Wheel,
                    -dy,
                )
                .as_ref(),
            );
        }
        if events.is_empty() {
            return Ok(());
        }
        events.push(*SynchronizeEvent::report(input_linux::EventTime::default()).as_ref());
        self.handle.write(&events).map_err(OutputError::Ioctl)?;
        Ok(())
    }

    pub fn button(&self, button: MouseButton) -> Key {
        match button {
            MouseButton::Left => Key::ButtonLeft,
            MouseButton::Right => Key::ButtonRight,
            MouseButton::Middle => Key::ButtonMiddle,
        }
    }

    pub fn button_down(&self, key: Key) -> Result<(), OutputError> {
        self.emit_key(key, KeyState::PRESSED)
    }

    pub fn button_up(&self, key: Key) -> Result<(), OutputError> {
        self.emit_key(key, KeyState::RELEASED)
    }

    fn emit_key(&self, key: Key, state: KeyState) -> Result<(), OutputError> {
        let events = [
            *KeyEvent::new(input_linux::EventTime::default(), key, state).as_ref(),
            *SynchronizeEvent::report(input_linux::EventTime::default()).as_ref(),
        ];
        self.handle.write(&events).map_err(OutputError::Ioctl)?;
        Ok(())
    }
}
