// Windows: no terminal window when the application is started (the tray icon replaces it).
#![cfg_attr(all(windows, has_tray), windows_subsystem = "windows")]

mod app;
mod browser;
mod commands;
mod config;
mod log;
mod platform;
mod providers;
mod search;
mod server;
#[cfg(has_tray)]
mod tray;
mod update;

use anyhow::Result;
use clap::Parser;
use commands::{Cli, Cmd, Ctx};
use config::Config;
use log::log_error;
use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    platform::attach_console();
    log::init();
    log::install_panic_hook();
    if let Err(e) = real_main() {
        log_error!("{e:#}");
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load();
    browser::set_choice(cfg.browser.clone());

    match cli.cmd.unwrap_or(Cmd::Serve { open: true, no_tray: false, foreground: false }) {
        Cmd::Serve { open, no_tray, foreground } => app::run(open, !no_tray, foreground),
        cmd => {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            let ctx = Ctx { cfg: Arc::new(Mutex::new(cfg)), in_app: false, host_local: true };
            let result = rt.block_on(async {
                let mut out = String::new();
                let r = commands::execute(cmd, &ctx, &mut out).await;
                let _ = std::io::Write::write_all(&mut std::io::stdout(), out.as_bytes()); // without a panic if the output is closed
                r
            });
            browser::shutdown(); // no background browser left behind by a command
            result
        }
    }
}
