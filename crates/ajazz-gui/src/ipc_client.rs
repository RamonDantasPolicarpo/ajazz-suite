use ajazz_protocol::mouse::{
    ChatteringConfig, MouseButtonSlot, MouseConfig, MouseProfile, MouseStatus,
    SwitchChatteringSnapshot,
};
use ajazz_protocol::{
    GifCrop, KeyAssignment, KeyboardChatteringConfig, KeyboardChatteringSnapshot, LightingConfig,
    MacroDefinition, RgbColor, SystemStages, UserProfile,
};
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
        config: LightingConfig,
    },
    SetCustomBank {
        bank: u8,
        colors: Vec<RgbColor>,
    },
    SelectCustomBank {
        bank: u8,
    },
    SetSystemStages {
        system: SystemStages,
    },
    SetKeyAssignment {
        phys_led: u16,
        assignment: Option<KeyAssignment>,
    },
    ResetKeymap,
    UpsertMacro {
        macro_def: MacroDefinition,
    },
    DeleteMacro {
        id: String,
    },
    SyncFromDevice,
    PlayGif {
        path: String,
        fps: Option<u8>,
        crop: GifCrop,
    },
    StopGif,
    GetStatus,
    GetMouseStatus,
    SetMouseConfig {
        config: MouseConfig,
    },
    SetMouseButton {
        slot: u8,
        assignment: MouseButtonSlot,
    },
    ResetMouseButtons,
    SyncMouseFromDevice,
    StartMousePairing,
    UploadMouseMacros,
    SetChatteringConfig {
        config: ChatteringConfig,
    },
    ResetChatteringStats,
    SetKeyboardChatteringConfig {
        config: KeyboardChatteringConfig,
    },
    ResetKeyboardChatteringStats,
    SetAutostart {
        enabled: bool,
    },
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
        profile: UserProfile,
        is_playing_gif: bool,
        #[serde(default)]
        keyboard_chattering: KeyboardChatteringSnapshot,
        #[serde(default)]
        autostart_enabled: bool,
    },
    MouseStatus {
        mouse_connected: bool,
        mouse_status: Option<MouseStatus>,
        profile: MouseProfile,
        #[serde(default)]
        chattering: SwitchChatteringSnapshot,
    },
    Pong,
}

fn timeout_for_request(req: &IpcRequest) -> Duration {
    match req {
        IpcRequest::SetMouseConfig { .. }
        | IpcRequest::SetMouseButton { .. }
        | IpcRequest::ResetMouseButtons
        | IpcRequest::SyncMouseFromDevice
        | IpcRequest::StartMousePairing
        | IpcRequest::UploadMouseMacros
        | IpcRequest::SetSystemStages { .. }
        | IpcRequest::SetKeyAssignment { .. }
        | IpcRequest::ResetKeymap
        | IpcRequest::UpsertMacro { .. }
        | IpcRequest::DeleteMacro { .. }
        | IpcRequest::SyncFromDevice
        | IpcRequest::SetStaticColor { .. }
        | IpcRequest::SetLighting { .. }
        | IpcRequest::SetCustomBank { .. }
        | IpcRequest::SelectCustomBank { .. }
        | IpcRequest::PlayGif { .. } => Duration::from_millis(10_000),
        _ => Duration::from_millis(4000),
    }
}

/// Envia uma requisição única para o Daemon e aguarda a resposta.
pub async fn send_request(req: IpcRequest) -> Result<IpcResponse, String> {
    let wait_timeout = timeout_for_request(&req);
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

    match tokio::time::timeout(wait_timeout, lines.next_line()).await {
        Ok(Ok(Some(line))) => {
            serde_json::from_str::<IpcResponse>(&line)
                .map_err(|e| format!("Resposta IPC inválida: {e}"))
        }
        Ok(Ok(None)) => Err("Conexão fechada pelo Daemon".into()),
        Ok(Err(e)) => Err(format!("Erro ao ler resposta do pipe: {e}")),
        Err(_) => Err("Timeout aguardando resposta do Daemon".into()),
    }
}
