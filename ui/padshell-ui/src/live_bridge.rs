//! Bridges live controller state to QML: a background thread
//! independently reads the controller (hidraw for a DS4, evdev for an
//! Xbox or other pad; never touching uinput - the UI must never fight
//! the running daemon for the virtual devices) and marshals updates onto
//! the Qt thread via CxxQtThread::queue().

use cxx_qt_lib::{QString, QStringList};
use padshell_core::{ButtonId, ControllerState};
use padshell_hid::controller;
use padshell_hid::device::BusType;
use std::time::Duration;

#[derive(Default)]
pub struct LiveBridgeRust {
    connected: bool,
    bus_label: QString,
    /// `ds4` / `xbox` / `generic`.
    controller_kind: QString,
    device_name: QString,
    left_stick_x: f64,
    left_stick_y: f64,
    right_stick_x: f64,
    right_stick_y: f64,
    l2_analog: f64,
    r2_analog: f64,
    pressed_buttons: QStringList,
    /// -1 when unknown / no controller.
    battery_percent: i32,
    battery_charging: bool,
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
        #[qproperty(QString, controller_kind)]
        #[qproperty(QString, device_name)]
        #[qproperty(f64, left_stick_x)]
        #[qproperty(f64, left_stick_y)]
        #[qproperty(f64, right_stick_x)]
        #[qproperty(f64, right_stick_y)]
        #[qproperty(f64, l2_analog)]
        #[qproperty(f64, r2_analog)]
        #[qproperty(QStringList, pressed_buttons)]
        #[qproperty(i32, battery_percent)]
        #[qproperty(bool, battery_charging)]
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
        let Some(info) = controller::first() else {
            tracing::debug!("no controller found, retrying in 1s");
            let _ = qt_thread.queue(|mut qobject| {
                qobject.as_mut().set_connected(false);
                qobject.as_mut().set_bus_label(QString::from("Disconnected"));
                qobject.as_mut().set_battery_percent(-1);
            });
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        let bus_label = match info.bus {
            BusType::Bluetooth => "Bluetooth",
            BusType::Usb => "USB",
            BusType::Other(_) => "Unknown",
        };

        tracing::info!(kind = %info.kind.as_str(), name = %info.name, path = ?info.input_path(), bus = ?info.bus, "live_bridge opening controller");
        let mut source = match info.open() {
            Ok(source) => source,
            Err(err) => {
                tracing::warn!(path = ?info.input_path(), error = %err, "live_bridge failed to open controller");
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        {
            let kind = info.kind.as_str().to_string();
            let name = info.name.clone();
            let bus_label = bus_label.to_string();
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().set_connected(true);
                qobject.as_mut().set_bus_label(QString::from(&bus_label));
                qobject.as_mut().set_controller_kind(QString::from(&kind));
                qobject.as_mut().set_device_name(QString::from(&name));
            });
        }

        let mut logged_first = false;
        let mut next_battery = std::time::Instant::now();
        loop {
            if std::time::Instant::now() >= next_battery {
                next_battery = std::time::Instant::now() + Duration::from_secs(10);
                let status = info.battery();
                let _ = qt_thread.queue(move |mut qobject| {
                    qobject.as_mut().set_battery_percent(status.map(|s| s.percent as i32).unwrap_or(-1));
                    qobject.as_mut().set_battery_charging(status.map(|s| s.charging).unwrap_or(false));
                });
            }
            let state = match source.next_state(Duration::from_millis(500)) {
                Ok(Some(state)) => state,
                Ok(None) => continue, // idle pad; loop back to refresh battery
                Err(err) => {
                    tracing::warn!(error = %err, "live_bridge read failed, reconnecting");
                    break; // device gone - fall out to the reconnect loop
                }
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
