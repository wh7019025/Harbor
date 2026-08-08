// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if app_lib::handle_cli_args() {
        return;
    }
    app_lib::run()
}
