use ajazz_protocol::{
    KeyAssignment, LightingConfig, MacroDefinition, RgbColor, SystemStages, UserProfile,
};
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
    },
    StopGif,
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
        profile: UserProfile,
        is_playing_gif: bool,
    },
    Pong,
}
