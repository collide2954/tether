#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod gpu;
mod render;
mod sound;

fn main() {
    let options = app::Options::parse(std::env::args().skip(1));
    let result = if let Some(path) = &options.catalog {
        app::catalog(path)
    } else if let Some(listen) = &options.listen {
        app::listen(listen)
    } else if let Some(capture) = &options.capture {
        app::capture(capture)
    } else {
        app::run(options).map_err(|e| e.to_string())
    };
    if let Err(error) = result {
        eprintln!("tether: {error}");
        std::process::exit(1);
    }
}
