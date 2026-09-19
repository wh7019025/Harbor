#[tokio::main]
async fn main() {
    let args = match harbor_core::CoreArgs::from_env() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = harbor_core::run_async(args).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
