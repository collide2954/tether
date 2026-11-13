#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod gpu;
mod render;

fn main() {
    let options = app::Options::parse(std::env::args().skip(1));
    if let Err(error) = app::run(options) {
        eprintln!("tether: {error}");
        std::process::exit(1);
    }
}
