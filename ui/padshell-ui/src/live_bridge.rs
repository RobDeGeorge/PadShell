//! Bridges live DS4 state to QML: a background thread independently
//! reads/decodes hidraw reports (never touching uinput - the UI must
//! never fight the running daemon for the virtual devices) and marshals
//! updates onto the Qt thread via CxxQtThread::queue().

use cxx_qt_lib::{QString, QStringList};
use padshell_core::{ButtonId, ControllerState};
use padshell_hid::device::{self, BusType};
use padshell_hid::ds4::{Ds4BtParser, Ds4UsbParser, DS4_PRODUCT_ID_V1, DS4_PRODUCT_ID_V2, DS4_VENDOR_ID};
use padshell_hid::ControllerReportParser;
use std::time::Duration;

#[derive(Default)]
pub struct LiveBridgeRust {
    connected: bool,
    bus_label: QString,
    left_stick_x: f64,
    left_stick_y: f64,
    right_stick_x: f64,
    right_stick_y: f64,
    l2_analog: f64,
    r2_analog: f64,
    pressed_buttons: QStringList,
    started: bool,
}

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, connected)]
        #[qproperty(QString, bus_label)]
        #[qproperty(f64, left_stick_x)]
        #[qproperty(f64, left_stick_y)]
        #[qproperty(f64, right_stick_x)]
        #[qproperty(f64, right_stick_y)]
        #[qproperty(f64, l2_analog)]
        #[qproperty(f64, r2_analog)]
        #[qproperty(QStringList, pressed_buttons)]
        type LiveBridge = super::LiveBridgeRust;

        /// Starts the background read thread. Safe to call more than
        /// once - only the first call has any effect.
        #[qinvokable]
        fn start(self: Pin<&mut LiveBridge>);
    }

    impl cxx_qt::Threading for LiveBridge {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use ffi::LiveBridge;

impl LiveBridge {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.started {
            return;
        }
        self.as_mut().rust_mut().started = true;

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || run_reader_loop(qt_thread));
    }
}

fn run_reader_loop(qt_thread: cxx_qt::CxxQtThread<LiveBridge>) {
    tracing::info!("live_bridge reader thread starting");
    loop {
        let Some(info) = find_ds4() else {
            tracing::debug!("no DS4 found, retrying in 1s");
            let _ = qt_thread.queue(|mut qobject| {
                qobject.as_mut().set_connected(false);
                qobject.as_mut().set_bus_label(QString::from("Disconnected"));
            });
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        let parser: Box<dyn ControllerReportParser + Send> = match info.bus {
            BusType::Bluetooth => Box::new(Ds4BtParser::new()),
            _ => Box::new(Ds4UsbParser::new()),
        };
        let bus_label = match info.bus {
            BusType::Bluetooth => "Bluetooth",
            BusType::Usb => "USB",
            BusType::Other(_) => "Unknown",
        };

        tracing::info!(path = ?info.path, bus = ?info.bus, "live_bridge opening DS4");
        let Ok(mut hidraw) = device::HidrawDevice::open(&info.path) else {
            tracing::warn!(path = ?info.path, "live_bridge failed to open hidraw device");
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        let mut buf = vec![0u8; parser.report_len()];
        let mut logged_first = false;
        loop {
            let Ok(n) = hidraw.read_report(&mut buf) else {
                tracing::warn!("live_bridge read_report failed, reconnecting");
                break; // device gone - fall out to the reconnect loop
            };
            let Some(state) = parser.parse(&buf[..n]) else {
                continue;
            };
            if !logged_first {
                tracing::info!(?state, "live_bridge first decoded state");
                logged_first = true;
            }

            let bus_label = bus_label.to_string();
            let result = qt_thread.queue(move |qobject| apply_state(qobject, &state, &bus_label));
            if let Err(err) = result {
                tracing::warn!(?err, "live_bridge qt_thread.queue failed, stopping reader thread");
                return;
            }
        }
    }
}

fn apply_state(mut qobject: Pin<&mut LiveBridge>, state: &ControllerState, bus_label: &str) {
    qobject.as_mut().set_connected(true);
    qobject.as_mut().set_bus_label(QString::from(bus_label));
    qobject.as_mut().set_left_stick_x(state.left_stick.x as f64);
    qobject.as_mut().set_left_stick_y(state.left_stick.y as f64);
    qobject.as_mut().set_right_stick_x(state.right_stick.x as f64);
    qobject.as_mut().set_right_stick_y(state.right_stick.y as f64);
    qobject.as_mut().set_l2_analog(state.l2_analog as f64);
    qobject.as_mut().set_r2_analog(state.r2_analog as f64);

    let pressed: QStringList = ButtonId::ALL
        .iter()
        .filter(|&&b| state.buttons.is_pressed(b))
        .map(|b| QString::from(b.as_str()))
        .collect();
    qobject.as_mut().set_pressed_buttons(pressed);
}

fn find_ds4() -> Option<device::HidrawInfo> {
    let mut candidates = device::find_all(DS4_VENDOR_ID, DS4_PRODUCT_ID_V1).ok()?;
    candidates.extend(device::find_all(DS4_VENDOR_ID, DS4_PRODUCT_ID_V2).unwrap_or_default());
    candidates.into_iter().next()
}
