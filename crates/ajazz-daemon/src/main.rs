use ajazz_protocol::PROFILE_AK820;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

pub const IPC_PIPE_NAME: &str = r"\\.\pipe\ajazz-daemon";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!(
        "Iniciando Daemon para {} (VID=0x{:04X}, PID=0x{:04X})",
        PROFILE_AK820.name, PROFILE_AK820.vid, PROFILE_AK820.pid
    );
    info!("Canal IPC configurado em: {}", IPC_PIPE_NAME);

    // Loop leve de segundo plano (0% CPU em idle)
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
