pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns true when CLI args were handled and the process should exit.
pub fn handle_cli_args() -> bool {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("{APP_VERSION}");
                return true;
            }
            _ => {}
        }
    }
    false
}
