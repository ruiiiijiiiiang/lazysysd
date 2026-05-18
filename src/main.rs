mod app;
mod models;
mod systemd;
mod ui;

use std::panic;

use crate::{app::runner::run_app, ui::utils::Tui};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let next = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = Tui::exit_terminal();
        next(info);
    }));

    if let Err(e) = run_app().await {
        let _ = Tui::exit_terminal();
        eprintln!("Application error: {e}");
        std::process::exit(1);
    }

    Ok(())
}
