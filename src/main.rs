#![cfg_attr(all(not(debug_assertions), not(test)), windows_subsystem = "windows")]

mod analysis;
mod app;
mod backup;
mod cli;
mod engine;
mod logging;
mod model;
mod network;
mod presets;
mod profiles;
mod registry;
mod system;
mod theme;
mod tweaks;
mod ui;
mod util;
mod views;

fn main() {
    logging::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if cli::run(&args) {
        return;
    }
    if let Err(error) = app::run() {
        logging::error(format!("GUI failed to start: {error}"));
        eprintln!("Clutchcore failed to start: {error}");
    }
}
