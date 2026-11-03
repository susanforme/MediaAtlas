#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod notices;

fn main() {
    match notices::handle_args(std::env::args_os().skip(1)) {
        Ok(false) => mediaatlas_desktop::run(),
        Ok(true) => (),
        Err(error) => {
            eprintln!("MediaAtlas: {error}");
            std::process::exit(2);
        }
    }
}
