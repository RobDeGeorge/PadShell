mod config_watch;
mod daemon;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use padshell_hid::device;
use padshell_hid::ds4::Ds4UsbParser;
use padshell_hid::ControllerReportParser;

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
    },
    /// List hidraw devices and their USB vendor/product IDs.
    Devices,
    /// Decode a raw HID report from a fixture file (raw bytes) and print
    /// the resulting controller state - no hardware required.
    TestReport {
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config, dump_only } => {
            let config_path = expand_tilde(&config);
            daemon::run(daemon::RunOptions {
                config_path,
                dump_only,
            })
        }
        Command::Devices => {
            for info in device::enumerate()? {
                println!(
                    "{}  vendor={:04x} product={:04x}",
                    info.path.display(),
                    info.vendor_id,
                    info.product_id
                );
            }
            Ok(())
        }
        Command::TestReport { path } => {
            let raw = std::fs::read(&path)?;
            let parser = Ds4UsbParser::new();
            match parser.parse(&raw) {
                Some(state) => {
                    println!("{state:#?}");
                    Ok(())
                }
                None => anyhow::bail!("report at {path:?} could not be parsed (too short or wrong report ID)"),
            }
        }
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
