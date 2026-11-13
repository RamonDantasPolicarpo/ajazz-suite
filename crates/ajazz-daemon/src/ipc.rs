use ajazz_protocol::{LightingMode, RgbColor};
use serde::{Deserialize, Serialize};

pub const IPC_PIPE_NAME: &str = r"\\.\pipe\ajazz-daemon";

/// Comandos recebidos de clientes (como a GUI Slint ou scripts CLI).
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
    GetStatus,
}

/// Respostas enviadas de volta ao cliente pelo Daemon.
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
    },
    Pong,
}
