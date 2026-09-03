pub mod keymap;
pub mod sink;
pub mod uinput;

pub use sink::OutputSink;
pub use uinput::{OutputError, VirtualKeyboard, VirtualMouse};
