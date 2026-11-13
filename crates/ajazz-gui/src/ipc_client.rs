use ajazz_protocol::{LightingMode, RgbColor};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

const PIPE_NAME: &str = r"\\.\pipe\ajazz-daemon";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    Ping,
    SetStaticColor {
        r: u8,
        g: u8,
        b: u8,
    },
    SetLighting {
        mode: LightingMode,
        brightness: u8,
        speed: u8,
        color: RgbColor,
    },
    SetKeyMatrix {
        colors: Vec<RgbColor>,
    },
    PlayGif {
        path: String,
        fps: Option<u8>,
    },
    StopGif,
    GetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcResponse {
    Ok {
        message: String,
    },
    Error {
        error: String,
    },
    Status {
        device_connected: bool,
        device_name: String,
        current_color: Option<RgbColor>,
        current_mode: Option<LightingMode>,
        is_playing_gif: bool,
    },
    Pong,
}

/// Envia uma requisição única para o Daemon e aguarda a resposta.
pub async fn send_request(req: IpcRequest) -> Result<IpcResponse, String> {
    let client = ClientOptions::new()
        .open(PIPE_NAME)
        .map_err(|e| format!("Não foi possível conectar ao Daemon: {e}"))?;

    let (reader, mut writer) = tokio::io::split(client);
    let mut lines = BufReader::new(reader).lines();

    let mut json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    json.push('\n');

    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|e| format!("Erro ao enviar requisição IPC: {e}"))?;

    match tokio::time::timeout(Duration::from_millis(2000), lines.next_line()).await {
        Ok(Ok(Some(line))) => {
            serde_json::from_str::<IpcResponse>(&line)
                .map_err(|e| format!("Resposta IPC inválida: {e}"))
        }
        Ok(Ok(None)) => Err("Conexão fechada pelo Daemon".into()),
        Ok(Err(e)) => Err(format!("Erro ao ler resposta do pipe: {e}")),
        Err(_) => Err("Timeout aguardando resposta do Daemon".into()),
    }
}
