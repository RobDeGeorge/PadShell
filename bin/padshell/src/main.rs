mod config_watch;
mod daemon;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use padshell_hid::device::{self};
use padshell_hid::ds4::{Ds4BtParser, Ds4UsbParser, DS4_PRODUCT_ID_V1, DS4_PRODUCT_ID_V2, DS4_VENDOR_ID};
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
        #[arg(long, default_value = "~/.config/padshell/config.toml")]
        config: String,
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
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config, dump_only, changes_only } => {
            let config_path = expand_tilde(&config);
            daemon::run(daemon::RunOptions {
                config_path,
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
    let mut candidates = device::find_all(DS4_VENDOR_ID, DS4_PRODUCT_ID_V1)?;
    candidates.extend(device::find_all(DS4_VENDOR_ID, DS4_PRODUCT_ID_V2)?);
    let info = candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no DS4 controller found"))?;
    Ok(HapticsController::discover(&info.hid_syspath))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
