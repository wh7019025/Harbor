// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn wants_core() -> bool {
    let mut args = std::env::args();
    let name = args
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    if name == "harbor_core" {
        return true;
    }
    args.any(|arg| arg == "--localhost-only" || arg == "--workspace")
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        if wants_core() {
            let args = match harbor_core::CoreArgs::from_env() {
                Ok(args) => args,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            };
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("tokio runtime failed: {error}");
                    std::process::exit(1);
                }
            };
            if let Err(error) = runtime.block_on(harbor_core::run_async(args)) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
    }
    if app_lib::handle_cli_args() {
        return;
    }
    app_lib::run()
}
