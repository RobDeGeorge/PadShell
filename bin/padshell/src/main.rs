mod config_watch;
mod daemon;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use padshell_hid::device::{self};
use padshell_hid::ds4::{Ds4BtParser, Ds4UsbParser};
use padshell_hid::{ControllerReportParser, HapticsController};

#[derive(Parser)]
#[command(name = "padshell", about = "Drive a Linux desktop with a game controller")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon: read the controller, apply bindings, drive the
    /// virtual keyboard/mouse.
    Run {
        /// Root config directory. Each connected controller gets its own
        /// profile under `<config_dir>/devices/`, seeded from
        /// `<config_dir>/default.toml` the first time it's seen.
        #[arg(long, default_value = "~/.config/padshell")]
        config_dir: String,
        /// Log decoded controller state instead of injecting output -
        /// useful for verifying HID parsing against real hardware.
        #[arg(long)]
        dump_only: bool,
        /// With --dump-only, print only when the button set changes
        /// instead of every report - readable while pressing buttons live.
        #[arg(long, requires = "dump_only")]
        changes_only: bool,
    },
    /// List hidraw devices and their USB vendor/product IDs.
    Devices,
    /// Decode a raw HID report from a fixture file (raw bytes) and print
    /// the resulting controller state - no hardware required.
    TestReport {
        path: PathBuf,
        /// Parse as a Bluetooth (report ID 0x11) report instead of USB.
        #[arg(long)]
        bt: bool,
    },
    /// Rumble the controller for a duration, then stop. Goes through the
    /// kernel's evdev force-feedback API, not a raw HID output report.
    Rumble {
        /// Weak/high-frequency motor magnitude (0-65535).
        #[arg(default_value = "32000")]
        weak: u16,
        /// Strong/low-frequency motor magnitude (0-65535).
        #[arg(default_value = "32000")]
        strong: u16,
        #[arg(long, default_value = "500")]
        duration_ms: u16,
    },
    /// Set the lightbar color (0-255 each) via the kernel LED subsystem.
    /// Stays set until changed again or the controller reconnects.
    Lightbar {
        r: u8,
        g: u8,
        b: u8,
    },
    /// Print the connected controller's battery level. Plain text
    /// (`85% discharging`) by default; `--json` prints a waybar-friendly
    /// object with text/tooltip/percentage/class.
    Battery {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config_dir, dump_only, changes_only } => {
            let config_dir = expand_tilde(&config_dir);
            daemon::run(daemon::RunOptions {
                config_dir,
                dump_only,
                changes_only,
            })
        }
        Command::Devices => {
            for info in device::enumerate()? {
                println!(
                    "{}  vendor={:04x} product={:04x} bus={:?}",
                    info.path.display(),
                    info.vendor_id,
                    info.product_id,
                    info.bus
                );
            }
            Ok(())
        }
        Command::TestReport { path, bt } => {
            let raw = std::fs::read(&path)?;
            let state = if bt {
                Ds4BtParser::new().parse(&raw)
            } else {
                Ds4UsbParser::new().parse(&raw)
            };
            match state {
                Some(state) => {
                    println!("{state:#?}");
                    Ok(())
                }
                None => anyhow::bail!("report at {path:?} could not be parsed (too short or wrong report ID)"),
            }
        }
        Command::Rumble { weak, strong, duration_ms } => {
            let haptics = find_haptics()?;
            if !haptics.has_rumble() {
                anyhow::bail!("no force-feedback capable device found for the connected controller");
            }
            haptics.rumble(weak, strong, duration_ms)?;
            Ok(())
        }
        Command::Battery { json } => {
            let mut infos = device::find_all(padshell_hid::ds4::DS4_VENDOR_ID, padshell_hid::ds4::DS4_PRODUCT_ID_V1)?;
            infos.extend(device::find_all(padshell_hid::ds4::DS4_VENDOR_ID, padshell_hid::ds4::DS4_PRODUCT_ID_V2)?);
            let status = infos.iter().find_map(|i| padshell_hid::battery::read(&i.hid_syspath));
            match (status, json) {
                (Some(s), false) => {
                    println!("{}% {}", s.percent, if s.charging { "charging" } else { "discharging" });
                }
                (Some(s), true) => {
                    let class = if s.charging { "charging" } else if s.percent <= 20 { "low" } else { "ok" };
                    println!(
                        "{{\"text\": \"{}%\", \"tooltip\": \"Controller battery {}% ({})\", \"percentage\": {}, \"class\": \"{}\"}}",
                        s.percent,
                        s.percent,
                        if s.charging { "charging" } else { "discharging" },
                        s.percent,
                        class
                    );
                }
                (None, false) => anyhow::bail!("no controller with a battery found"),
                (None, true) => println!("{{\"text\": \"\", \"tooltip\": \"No controller\", \"class\": \"disconnected\"}}"),
            }
            Ok(())
        }
        Command::Lightbar { r, g, b } => {
            let haptics = find_haptics()?;
            if !haptics.has_lightbar() {
                anyhow::bail!("no lightbar LEDs found for the connected controller");
            }
            haptics.set_lightbar(r, g, b)?;
            Ok(())
        }
    }
}

fn find_haptics() -> anyhow::Result<HapticsController> {
    HapticsController::discover_ds4().ok_or_else(|| anyhow::anyhow!("no DS4 controller found"))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
