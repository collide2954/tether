#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod gpu;
mod render;

fn main() {
    let options = app::Options::parse(std::env::args().skip(1));
    let result = match &options.capture {
        Some(capture) => app::capture(capture),
        None => app::run(options).map_err(|e| e.to_string()),
    };
    if let Err(error) = result {
        eprintln!("tether: {error}");
        std::process::exit(1);
    }
}
