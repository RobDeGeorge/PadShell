//! A transport-independent "give me the next controller state" handle,
//! so the daemon and GUI don't care whether a controller is read as raw
//! HID reports (DS4 over hidraw) or as kernel input events (Xbox and
//! other gamepads over evdev).

use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use padshell_core::ControllerState;

use crate::device::{HidError, HidrawDevice};
use crate::report::ControllerReportParser;

pub trait ControllerSource: Send {
    /// Blocks up to `timeout` for the next decoded state. `Ok(None)`
    /// means the timeout elapsed with nothing new - callers use that
    /// gap to poll config reloads and battery without needing the
    /// controller to be chatty (a DS4 streams reports continuously; an
    /// Xbox pad only speaks when something changes). `Err` means the
    /// device is gone.
    fn next_state(&mut self, timeout: Duration) -> Result<Option<ControllerState>, HidError>;
}

/// Waits for the fd to become readable. Returns false on timeout.
pub(crate) fn wait_readable(fd: BorrowedFd<'_>, timeout: Duration) -> Result<bool, HidError> {
    let ms = u16::try_from(timeout.as_millis()).unwrap_or(u16::MAX);
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
    let n = poll(&mut fds, PollTimeout::from(ms)).map_err(std::io::Error::from)?;
    if n == 0 {
        return Ok(false);
    }
    if let Some(revents) = fds[0].revents() {
        if revents.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL) {
            return Err(HidError::Disconnected);
        }
    }
    Ok(true)
}

/// A hidraw node plus the parser for its report layout.
pub struct HidrawSource {
    device: HidrawDevice,
    parser: Box<dyn ControllerReportParser + Send>,
    buf: Vec<u8>,
}

impl HidrawSource {
    pub fn new(device: HidrawDevice, parser: Box<dyn ControllerReportParser + Send>) -> Self {
        let buf = vec![0u8; parser.report_len()];
        HidrawSource { device, parser, buf }
    }
}

impl ControllerSource for HidrawSource {
    fn next_state(&mut self, timeout: Duration) -> Result<Option<ControllerState>, HidError> {
        loop {
            if !wait_readable(self.device.as_fd(), timeout)? {
                return Ok(None);
            }
            let n = self.device.read_report(&mut self.buf)?;
            if let Some(state) = self.parser.parse(&self.buf[..n]) {
                return Ok(Some(state));
            }
            tracing::trace!(len = n, "discarded unparseable report");
        }
    }
}
