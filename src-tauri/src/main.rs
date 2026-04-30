// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // CLI dispatch — runs BEFORE Tauri so transport commands don't spawn a
    // second GUI instance. Deep links (tidal://...) and unknown args fall
    // through to the regular GUI startup.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && tauri_app_lib::cli::is_cli_command(&args[1]) {
        std::process::exit(tauri_app_lib::cli::run(&args));
    }

    tauri_app_lib::run()
}
