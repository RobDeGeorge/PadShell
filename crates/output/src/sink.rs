//! Wires padshell_core::OutputEvent onto the virtual keyboard/mouse
//! devices - the single place the daemon calls into per emitted event.

use padshell_core::OutputEvent;

use crate::uinput::{OutputError, VirtualKeyboard, VirtualMouse};

pub struct OutputSink {
    keyboard: VirtualKeyboard,
    mouse: VirtualMouse,
}

impl OutputSink {
    pub fn create() -> Result<Self, OutputError> {
        Ok(OutputSink {
            keyboard: VirtualKeyboard::create()?,
            mouse: VirtualMouse::create()?,
        })
    }

    pub fn apply(&self, event: &OutputEvent) -> Result<(), OutputError> {
        match event {
            OutputEvent::KeyDown(name) => {
                let key = self.keyboard.key(name)?;
                self.keyboard.key_down(key)
            }
            OutputEvent::KeyUp(name) => {
                let key = self.keyboard.key(name)?;
                self.keyboard.key_up(key)
            }
            OutputEvent::KeyTap(names) => {
                let keys: Result<Vec<_>, _> =
                    names.iter().map(|n| self.keyboard.key(n)).collect();
                self.keyboard.tap(&keys?)
            }
            OutputEvent::MouseButtonDown(button) => {
                let key = self.mouse.button(*button);
                self.mouse.button_down(key)
            }
            OutputEvent::MouseButtonUp(button) => {
                let key = self.mouse.button(*button);
                self.mouse.button_up(key)
            }
            OutputEvent::MouseMove { dx, dy } => self.mouse.move_relative(*dx, *dy),
            OutputEvent::Scroll { dx, dy } => self.mouse.scroll(*dx, *dy),
        }
    }
}
