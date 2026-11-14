use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;
use thiserror::Error;

pub use hidapi;

pub mod keymap;
pub use keymap::*;
pub mod macros;
pub use macros::*;
pub mod matrix;
pub use matrix::*;

/// EVision Semiconductor (Fabricante do controlador USB do teclado físico AK820)
pub const AK820_VID: u16 = 0x320F;
pub const AK820_PID: u16 = 0x505B;
/// UsagePage proprietária para relatórios de controle e iluminação do AK820
pub const AK820_VENDOR_USAGE_PAGE: u16 = 0xFF1C;
pub const AK820_VENDOR_USAGE: u16 = 0x0092;

/// AJAZZ NearLink Mouse (Mouse sem fio NearLink detectado)
pub const MOUSE_NEARLINK_VID: u16 = 0x7768;
pub const MOUSE_NEARLINK_PID: u16 = 0x642E;

/// Sonix Technology Co., Ltd. (perfil de fallback presente nos arquivos padrão)
pub const SONIX_FALLBACK_VID: u16 = 0x0C45;
pub const SONIX_FALLBACK_PID: u16 = 0x1007;

/// Tamanho padrão do buffer de dados de um HID Report do teclado.
pub const HID_REPORT_PAYLOAD_SIZE: usize = 64;

/// Report ID fixo do canal proprietário de comandos
pub const REPORT_ID_COMMAND: u8 = 0x04;

/// Opcodes do protocolo USB HID do AK820
pub const OPCODE_BEGIN_TRANSACTION: u8 = 0x01;
pub const OPCODE_COMMIT_TRANSACTION: u8 = 0x02;
pub const OPCODE_GET_DEVICE_INFO: u8 = 0x03;
pub const OPCODE_GET_CONFIG: u8 = 0x05;
pub const OPCODE_SET_LIGHTING: u8 = 0x06;
pub const OPCODE_GET_KEY_MATRIX: u8 = 0x07;
pub const OPCODE_KEY_MATRIX_MAPPING: u8 = OPCODE_GET_KEY_MATRIX;
pub const OPCODE_SET_KEY_MATRIX: u8 = 0x09;
pub const OPCODE_SET_CUSTOM_RGB_MATRIX: u8 = 0x0B;
pub const OPCODE_SET_LIVE_MATRIX: u8 = 0x12;
pub const OPCODE_SET_MACROS: u8 = 0x15;
pub const OPCODE_SET_SHOOT_MACROS: u8 = 0x17;
pub const OPCODE_HEARTBEAT_POLL: u8 = 0x1A;
pub const OPCODE_GET_LED_MAPPING: u8 = 0x1B;

/// Bytes de matriz RGB enviados por quadros voláteis (126 LEDs * 3 = 378 bytes, `sub_4b4310`).
pub const LIVE_MATRIX_BYTES: usize = 126 * 3;

/// Tabela oficial de escala de brilho para quadros em tempo real (`.rdata 0x682ec0`).
pub const BRIGHTNESS_LUT: [u8; 5] = [0, 63, 126, 190, 255];

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Falha na camada USB HID: {0}")]
    Hid(#[from] hidapi::HidError),

    #[error("Dispositivo não encontrado: VID=0x{vid:04X}, PID=0x{pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("Tamanho de payload inválido: esperado {expected}, recebido {received}")]
    InvalidPayloadSize { expected: usize, received: usize },

    #[error("Firmware rejeitou comando 0x{opcode:02X} com status 0x{status:02X}")]
    CommandRejected { opcode: u8, status: u8 },

    #[error("Timeout aguardando resposta HID para opcode 0x{opcode:02X}")]
    ResponseTimeout { opcode: u8 },

    #[error("Dados inválidos no protocolo: {0}")]
    InvalidProtocolData(String),
}

/// Taxa de Polling USB (`PollingRate` em `Opcode 0x05`/`0x06`, byte 30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum PollingRate {
    #[default]
    Hz1000 = 0,
    Hz500 = 1,
    Hz250 = 2,
    Hz125 = 3,
}

impl PollingRate {
    pub const ALL: [PollingRate; 4] = [Self::Hz1000, Self::Hz500, Self::Hz250, Self::Hz125];

    pub const fn from_wire(val: u8) -> Self {
        match val {
            1 => Self::Hz500,
            2 => Self::Hz250,
            3 => Self::Hz125,
            _ => Self::Hz1000,
        }
    }

    pub const fn wire_id(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hz1000 => "1000 Hz",
            Self::Hz500 => "500 Hz",
            Self::Hz250 => "250 Hz",
            Self::Hz125 => "125 Hz",
        }
    }
}

/// Flags de sistema e taxa de polling armazenados junto à iluminação em `Opcode 0x05`/`0x06` (`sub_490e80`/`sub_492b30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SystemStages {
    /// Bloqueio da tecla Windows (`HID[29]`: `WinFlag`).
    pub win_lock: bool,
    /// Inversão WASD <-> Setas (`HID[27]`: `WFlag`).
    pub wasd_swap: bool,
    /// Taxa de polling USB (`HID[30]`: `0=1000Hz, 1=500Hz, 2=250Hz, 3=125Hz`).
    pub polling_rate: PollingRate,
    /// Flag de modo 6-Key / NKRO (`HID[28]`: `Key6Flag`, padrão `0xFF`).
    pub key6_flag: u8,
    /// Flag de modo Mac (`HID[35]`: `MacMode`, padrão `0`).
    pub mac_mode: u8,
    /// Modo de roda/knob (`HID[36]`: `Wheel`, padrão `0`).
    pub wheel: u8,
}

impl Default for SystemStages {
    fn default() -> Self {
        Self {
            win_lock: false,
            wasd_swap: false,
            polling_rate: PollingRate::Hz1000,
            key6_flag: 0xFF,
            mac_mode: 0,
            wheel: 0,
        }
    }
}

/// Capacidades lidas do firmware via `Opcode 0x03` (`GET_DEVICE_INFO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapabilities {
    pub total_keys: usize,
    pub macro_128b_blocks: usize,
}

impl DeviceCapabilities {
    /// Tamanho máximo em bytes disponível na flash de macros (`macro_128b_blocks * 128`).
    pub const fn max_macro_bytes(self) -> usize {
        self.macro_128b_blocks.saturating_mul(128)
    }
}

/// Cache em memória da topologia física/matriz do dispositivo conectado (`0x03` + `0x1B` + `0x07`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareLayoutCache {
    pub capabilities: DeviceCapabilities,
    /// Mapeamento `led_map[hw_key_idx] == phys_led` obtido via `Opcode 0x1B`.
    pub led_map: Vec<u8>,
    /// Matriz base lida via `Opcode 0x07` (`total_keys * 3` bytes) para preservar slots não visíveis.
    pub baseline_matrix: Vec<u8>,
}

/// Direção da animação de iluminação (`Fx` no protocolo oficial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum LightingDirection {
    /// Esquerda→Direita / Anti-horário / Cima→Baixo / Dentro→Fora (`deng_FXR_check`).
    #[default]
    Primary = 0,
    /// Direita→Esquerda / Horário / Baixo→Cima / Fora→Dentro (`deng_FXL_check`).
    Reverse = 1,
}

/// Tipo de rótulo de direção exibido na interface conforme o efeito ativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionKind {
    None,
    Horizontal,
    Circular,
    Vertical,
    Radial,
}

/// Capacidades e parâmetros aceitos por um Lighting Effect do AK820 (`sub_415010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeCapabilities {
    pub supports_color: bool,
    pub supports_multicolor: bool,
    pub supports_speed: bool,
    pub direction_kind: DirectionKind,
    pub supports_zzcc_mode: bool,
    pub is_custom_per_key: bool,
}

/// Todos os 19 Lighting Effects nativos de firmware + Modo 20 Custom do AK820.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum LightingMode {
    Corrugated = 0x01,
    Cloud = 0x02,
    Serpentine = 0x03,
    Spectrum = 0x04,
    Breath = 0x05,
    #[default]
    Static = 0x06,
    Reaction = 0x07,
    Ripples = 0x08,
    Traverse = 0x09,
    Stars = 0x0A,
    Flowers = 0x0B,
    Roll = 0x0C,
    Wave = 0x0D,
    Cartoon = 0x0E,
    Rain = 0x0F,
    Scan = 0x10,
    Surmount = 0x11,
    Speed = 0x12,
    Custom = 0x14,
    Capture = 0x1E,
}

impl LightingMode {
    /// Ordem canônica dos efeitos expostos para seleção do usuário.
    pub const ALL: [LightingMode; 20] = [
        Self::Static,
        Self::Corrugated,
        Self::Cloud,
        Self::Serpentine,
        Self::Spectrum,
        Self::Breath,
        Self::Reaction,
        Self::Ripples,
        Self::Traverse,
        Self::Stars,
        Self::Flowers,
        Self::Roll,
        Self::Wave,
        Self::Cartoon,
        Self::Rain,
        Self::Scan,
        Self::Surmount,
        Self::Speed,
        Self::Capture,
        Self::Custom,
    ];

    /// Byte enviado em `payload[9]` no pacote `Opcode 0x06` (`Capture` usa `0xFE`).
    pub const fn wire_id(self) -> u8 {
        match self {
            Self::Capture => 0xFE,
            other => other as u8,
        }
    }

    /// Reconstrói `LightingMode` a partir do byte recebido em `Opcode 0x05` (`GET_CONFIG`).
    pub const fn from_wire(wire: u8) -> Self {
        match wire {
            0x01 => Self::Corrugated,
            0x02 => Self::Cloud,
            0x03 => Self::Serpentine,
            0x04 => Self::Spectrum,
            0x05 => Self::Breath,
            0x06 => Self::Static,
            0x07 => Self::Reaction,
            0x08 => Self::Ripples,
            0x09 => Self::Traverse,
            0x0A => Self::Stars,
            0x0B => Self::Flowers,
            0x0C => Self::Roll,
            0x0D => Self::Wave,
            0x0E => Self::Cartoon,
            0x0F => Self::Rain,
            0x10 => Self::Scan,
            0x11 => Self::Surmount,
            0x12 => Self::Speed,
            0x14 => Self::Custom,
            0x1E | 0xFE => Self::Capture,
            _ => Self::Static,
        }
    }

    /// Nome do efeito alinhado ao software oficial (`lan_en.xml`).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Corrugated => "Corrugated",
            Self::Cloud => "Cloud",
            Self::Serpentine => "Serpentine",
            Self::Spectrum => "Spectrum",
            Self::Breath => "Breath",
            Self::Static => "Normal",
            Self::Reaction => "Reaction",
            Self::Ripples => "Ripples",
            Self::Traverse => "Traverse",
            Self::Stars => "Stars",
            Self::Flowers => "Flowers",
            Self::Roll => "Roll",
            Self::Wave => "Wave",
            Self::Cartoon => "Cartoon",
            Self::Rain => "Rain",
            Self::Scan => "Scan",
            Self::Surmount => "Surmount",
            Self::Speed => "Speed",
            Self::Custom => "Custom",
            Self::Capture => "Capture",
        }
    }

    /// Capacidades de parâmetros suportadas pelo modo no firmware do AK820.
    pub const fn capabilities(self) -> ModeCapabilities {
        use DirectionKind::*;
        match self {
            Self::Corrugated | Self::Cloud => ModeCapabilities {
                supports_color: true,
                supports_multicolor: true,
                supports_speed: true,
                direction_kind: Horizontal,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Serpentine => ModeCapabilities {
                supports_color: true,
                supports_multicolor: true,
                supports_speed: true,
                direction_kind: Circular,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Spectrum | Self::Flowers => ModeCapabilities {
                supports_color: false,
                supports_multicolor: false,
                supports_speed: true,
                direction_kind: None,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Breath
            | Self::Reaction
            | Self::Ripples
            | Self::Traverse
            | Self::Stars
            | Self::Wave
            | Self::Cartoon
            | Self::Rain => ModeCapabilities {
                supports_color: true,
                supports_multicolor: true,
                supports_speed: true,
                direction_kind: None,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Static | Self::Scan => ModeCapabilities {
                supports_color: true,
                supports_multicolor: true,
                supports_speed: false,
                direction_kind: None,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Roll => ModeCapabilities {
                supports_color: false,
                supports_multicolor: false,
                supports_speed: true,
                direction_kind: Vertical,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Surmount => ModeCapabilities {
                supports_color: false,
                supports_multicolor: false,
                supports_speed: false,
                direction_kind: None,
                supports_zzcc_mode: true,
                is_custom_per_key: false,
            },
            Self::Speed => ModeCapabilities {
                supports_color: false,
                supports_multicolor: false,
                supports_speed: false,
                direction_kind: Radial,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
            Self::Custom => ModeCapabilities {
                supports_color: true,
                supports_multicolor: false,
                supports_speed: false,
                direction_kind: None,
                supports_zzcc_mode: false,
                is_custom_per_key: true,
            },
            Self::Capture => ModeCapabilities {
                supports_color: false,
                supports_multicolor: false,
                supports_speed: false,
                direction_kind: None,
                supports_zzcc_mode: false,
                is_custom_per_key: false,
            },
        }
    }
}

/// Parâmetros completos de um Lighting Effect do AK820 (`Opcode 0x06`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LightingConfig {
    pub mode: LightingMode,
    /// Nível de brilho: `0..=4`.
    pub brightness: u8,
    /// Nível de velocidade: `0..=4` (serializado como `4 - speed` no fio).
    pub speed: u8,
    pub direction: LightingDirection,
    pub multicolor: bool,
    pub color: RgbColor,
    /// Sub-modo do efeito Surmount (`ZZCCLedMode`, `0..=3`).
    pub zzcc_mode: u8,
    /// Banco ativo do modo Custom (`0..=2` -> Custom 1..3).
    pub custom_group: u8,
}

impl LightingConfig {
    /// Garante limites válidos e desativa flags incompatíveis com as capacidades do modo.
    pub fn normalize(&mut self) {
        self.brightness = self.brightness.min(4);
        self.speed = self.speed.min(4);
        self.zzcc_mode = self.zzcc_mode.min(3);
        self.custom_group = self.custom_group.min(2);
        if !self.mode.capabilities().supports_multicolor {
            self.multicolor = false;
        }
    }
}

impl Default for LightingConfig {
    fn default() -> Self {
        Self {
            mode: LightingMode::Static,
            brightness: 4,
            speed: 2,
            direction: LightingDirection::Primary,
            multicolor: false,
            color: RgbColor::new(0xFF, 0x00, 0x00),
            zzcc_mode: 0,
            custom_group: 0,
        }
    }
}

/// Perfil do usuário persistido pelo Daemon (`User Profile`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    pub lighting: LightingConfig,
    /// Três bancos de iluminação estática individual por tecla (128 LEDs físicos cada).
    pub custom_banks: [Vec<RgbColor>; 3],
    /// Configurações de sistema no bloco `0x05`/`0x06` (`WinFlag`, `WFlag`, `PollingRate`).
    #[serde(default)]
    pub system: SystemStages,
    /// Sobrescritas de remapeamento por `phys_led` (`AK820_KEY_LAYOUT`).
    #[serde(default)]
    pub keymap: BTreeMap<u16, KeyAssignment>,
    /// Macros definidas pelo usuário.
    #[serde(default)]
    pub macros: Vec<MacroDefinition>,
}

impl Default for UserProfile {
    fn default() -> Self {
        let empty = vec![RgbColor::default(); TOTAL_PHYSICAL_LEDS];
        Self {
            lighting: LightingConfig::default(),
            custom_banks: [empty.clone(), empty.clone(), empty],
            system: SystemStages::default(),
            keymap: BTreeMap::new(),
            macros: Vec::new(),
        }
    }
}

impl UserProfile {
    /// Normaliza os vetores dos bancos Custom para exatamente `TOTAL_PHYSICAL_LEDS`.
    pub fn normalize(&mut self) {
        self.lighting.normalize();
        for bank in &mut self.custom_banks {
            bank.resize(TOTAL_PHYSICAL_LEDS, RgbColor::default());
        }
    }

    /// Extrai o array fixo de 128 LEDs do banco Custom (`0..=2`).
    pub fn bank_array(&self, bank: u8) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
        let mut arr = [RgbColor::default(); TOTAL_PHYSICAL_LEDS];
        let slot = &self.custom_banks[bank.min(2) as usize];
        let count = slot.len().min(TOTAL_PHYSICAL_LEDS);
        arr[..count].copy_from_slice(&slot[..count]);
        arr
    }

    /// Atualiza um banco Custom (`0..=2`) a partir de uma lista de cores.
    pub fn set_bank_colors(&mut self, bank: u8, colors: &[RgbColor]) {
        let idx = bank.min(2) as usize;
        let bank_vec = &mut self.custom_banks[idx];
        bank_vec.resize(TOTAL_PHYSICAL_LEDS, RgbColor::default());
        let count = colors.len().min(TOTAL_PHYSICAL_LEDS);
        bank_vec[..count].copy_from_slice(&colors[..count]);
        for c in &mut bank_vec[count..] {
            *c = RgbColor::default();
        }
    }
}

/// Representa a cor de um LED individual na matriz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Buffer binário de tamanho fixo para comunicação USB com o teclado.
#[derive(Clone, Copy)]
pub struct HidReport {
    pub report_id: u8,
    pub payload: [u8; HID_REPORT_PAYLOAD_SIZE],
}

impl fmt::Debug for HidReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HidReport(id=0x{:02X}, len={}, sample={:02X?})",
            self.report_id,
            self.payload.len(),
            &self.payload[..8.min(self.payload.len())]
        )
    }
}

impl HidReport {
    pub fn new(report_id: u8) -> Self {
        Self {
            report_id,
            payload: [0u8; HID_REPORT_PAYLOAD_SIZE],
        }
    }
}

/// Calcula a soma de verificação de 16 bits (Little-Endian) sobre os bytes `payload[3..64]`.
#[inline]
pub fn calculate_checksum(payload: &[u8; HID_REPORT_PAYLOAD_SIZE]) -> u16 {
    payload[3..HID_REPORT_PAYLOAD_SIZE]
        .iter()
        .map(|&b| b as u16)
        .sum()
}

/// Constrói o pacote de início de transação (Opcode 0x01).
pub fn build_begin_transaction_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = OPCODE_BEGIN_TRANSACTION;
    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

/// Constrói o pacote de confirmação/aplicação de transação (Opcode 0x02).
pub fn build_commit_transaction_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = OPCODE_COMMIT_TRANSACTION;
    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

/// Escala um `RgbColor` pela tabela de brilho oficial `[0, 63, 126, 190, 255]` (`sub_4b4310`).
#[inline]
pub fn scale_color_by_brightness(color: RgbColor, brightness: u8) -> RgbColor {
    let alpha = BRIGHTNESS_LUT[brightness.min(4) as usize] as u16;
    RgbColor::new(
        (((color.r as u16) * alpha) >> 8) as u8,
        (((color.g as u16) * alpha) >> 8) as u8,
        (((color.b as u16) * alpha) >> 8) as u8,
    )
}

/// Constrói o pacote completo de configuração (`Opcode 0x06`, bloco `0x1D` bytes)
/// preservando tanto `LightingConfig` quanto `SystemStages` (`sub_492b30` / `sub_44ab70`).
pub fn build_config_packet(
    config: &LightingConfig,
    system: &SystemStages,
) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut cfg = *config;
    cfg.normalize();

    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = OPCODE_SET_LIGHTING;
    buf[4] = 0x1D; // 29 bytes
    buf[9] = cfg.mode.wire_id();
    buf[10] = cfg.brightness;
    buf[11] = 4u8.saturating_sub(cfg.speed); // sub_492b30: 4 - Speed
    buf[12] = cfg.direction as u8;
    buf[13] = u8::from(cfg.multicolor);
    buf[14] = cfg.color.r;
    buf[15] = cfg.color.g;
    buf[16] = cfg.color.b;
    buf[26] = cfg.zzcc_mode;
    buf[27] = u8::from(system.wasd_swap); // WFlag
    buf[28] = system.key6_flag; // Key6Flag
    buf[29] = u8::from(system.win_lock); // WinFlag
    buf[30] = system.polling_rate.wire_id(); // PollingRate (0=1000..3=125Hz)
    buf[33] = cfg.custom_group;
    buf[35] = system.mac_mode;
    buf[36] = system.wheel;

    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

/// Constrói o pacote completo de configuração de iluminação (`Opcode 0x06`, bloco `0x1D` bytes)
/// com `SystemStages::default()` (`WFlag=0`, `Key6Flag=0xFF`, `WinFlag=0`, `PollingRate=1000Hz`).
pub fn build_lighting_packet_config(config: &LightingConfig) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_config_packet(config, &SystemStages::default())
}

/// Constrói pacote de leitura genérico (`Opcode 0x03`, `0x05`, etc.) para `len` bytes em `offset`.
pub fn build_read_command_packet(
    opcode: u8,
    offset: u16,
    len: u8,
) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = opcode;
    buf[4] = len;
    buf[5] = (offset & 0xFF) as u8;
    buf[6] = (offset >> 8) as u8;
    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

/// Decodifica o payload de `0x1D` (29) bytes retornado por `Opcode 0x05` (`ack[8..37]`)
/// para `(LightingConfig, SystemStages)`.
pub fn parse_config_block(block: &[u8]) -> Result<(LightingConfig, SystemStages), ProtocolError> {
    if block.len() < 0x1D {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: 0x1D,
            received: block.len(),
        });
    }
    let mut cfg = LightingConfig {
        mode: LightingMode::from_wire(block[1]),
        brightness: block[2],
        speed: 4u8.saturating_sub(block[3]),
        direction: if block[4] == 1 {
            LightingDirection::Reverse
        } else {
            LightingDirection::Primary
        },
        multicolor: block[5] != 0,
        color: RgbColor::new(block[6], block[7], block[8]),
        zzcc_mode: block[18],
        custom_group: block[25],
    };
    cfg.normalize();

    let system = SystemStages {
        wasd_swap: block[19] != 0,
        key6_flag: block[20],
        win_lock: block[21] != 0,
        polling_rate: PollingRate::from_wire(block[22]),
        mac_mode: block[27],
        wheel: block[28],
    };
    Ok((cfg, system))
}

/// Decodifica os 34 bytes de `Opcode 0x03` (`GET_DEVICE_INFO`, `ack[8..42]`).
pub fn parse_device_info_block(info: &[u8]) -> Result<DeviceCapabilities, ProtocolError> {
    if info.len() < 34 {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: 34,
            received: info.len(),
        });
    }
    Ok(DeviceCapabilities {
        total_keys: info[5] as usize,
        macro_128b_blocks: info[6] as usize,
    })
}

/// Constrói o pacote legado de configuração de modo e cor de iluminação (Opcode 0x06).
pub fn build_lighting_packet(
    mode: LightingMode,
    brightness: u8,
    speed: u8,
    color: RgbColor,
) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = OPCODE_SET_LIGHTING;
    buf[4] = 0x1D; // Comprimento do bloco de dados (29 bytes)
    buf[9] = mode.wire_id();
    buf[10] = brightness;
    buf[11] = speed;
    buf[14] = color.r;
    buf[15] = color.g;
    buf[16] = color.b;
    // Valores observados na captura USB inicial (`ak820_red.pcapng` frame 117)
    buf[28] = 0xFF;
    buf[29] = 0x00;
    buf[30] = 0x00;
    buf[33] = 0x01;

    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

fn build_chunked_matrix_packets(
    opcode: u8,
    wire_base: usize,
    raw_rgb: &[u8],
) -> [[u8; HID_REPORT_PAYLOAD_SIZE]; 7] {
    let mut packets = [[0u8; HID_REPORT_PAYLOAD_SIZE]; 7];
    const CHUNK_SIZE: usize = 56;
    let mut chunk_offset = 0usize;

    for pkt in packets.iter_mut() {
        let chunk_len = raw_rgb.len().saturating_sub(chunk_offset).min(CHUNK_SIZE);
        let wire_offset = wire_base + chunk_offset;
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = opcode;
        pkt[4] = chunk_len as u8;
        pkt[5] = (wire_offset & 0xFF) as u8;
        pkt[6] = ((wire_offset >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;
        if chunk_len > 0 {
            pkt[8..8 + chunk_len].copy_from_slice(&raw_rgb[chunk_offset..chunk_offset + chunk_len]);
        }

        let sum = calculate_checksum(pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;

        chunk_offset += chunk_len;
    }

    packets
}

/// Constrói os 7 pacotes HID (Opcode 0x0B) para um banco Custom específico (`custom_group` `0..=2`).
/// Cada banco tem stride de `0x200` (512 bytes), gravando 384 bytes (128 LEDs x RGB).
pub fn build_custom_matrix_packets_for_bank(
    custom_group: u8,
    colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
) -> [[u8; HID_REPORT_PAYLOAD_SIZE]; 7] {
    let mut raw_rgb = [0u8; TOTAL_PHYSICAL_LEDS * 3];
    for (i, c) in colors.iter().enumerate() {
        raw_rgb[i * 3] = c.r;
        raw_rgb[i * 3 + 1] = c.g;
        raw_rgb[i * 3 + 2] = c.b;
    }

    let bank_base = (custom_group.min(2) as usize) * 0x200;
    build_chunked_matrix_packets(OPCODE_SET_CUSTOM_RGB_MATRIX, bank_base, &raw_rgb)
}

/// Constrói os 7 pacotes HID voláteis (Opcode 0x12) para streaming em tempo real (378 bytes / 126 LEDs),
/// sem transações de gravação em flash (`0x01`/`0x02`), escalados por brilho conforme `sub_4b4310`.
pub fn build_live_matrix_packets(
    colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    brightness: u8,
) -> [[u8; HID_REPORT_PAYLOAD_SIZE]; 7] {
    let mut raw_rgb = [0u8; LIVE_MATRIX_BYTES];
    for (i, &c) in colors.iter().take(126).enumerate() {
        let scaled = scale_color_by_brightness(c, brightness);
        raw_rgb[i * 3] = scaled.r;
        raw_rgb[i * 3 + 1] = scaled.g;
        raw_rgb[i * 3 + 2] = scaled.b;
    }
    build_chunked_matrix_packets(OPCODE_SET_LIVE_MATRIX, 0, &raw_rgb)
}

/// Constrói os 7 pacotes HID (Opcode 0x0B) para o banco Custom 0 (384 bytes / offset 0).
pub fn build_custom_matrix_packets(
    colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
) -> [[u8; HID_REPORT_PAYLOAD_SIZE]; 7] {
    build_custom_matrix_packets_for_bank(0, colors)
}

/// Handle conectado ao teclado AK820 na interface proprietária de controle.
pub struct Ak820Device {
    device: hidapi::HidDevice,
    cached_layout: Option<HardwareLayoutCache>,
}

impl Ak820Device {
    /// Procura e abre o teclado AK820 na UsagePage proprietária (0xFF1C:0x0092).
    pub fn open(api: &hidapi::HidApi) -> Result<Self, ProtocolError> {
        for d in api.device_list() {
            if d.vendor_id() == AK820_VID
                && d.product_id() == AK820_PID
                && d.usage_page() == AK820_VENDOR_USAGE_PAGE
                && d.usage() == AK820_VENDOR_USAGE
            {
                let device = d.open_device(api)?;
                return Ok(Self {
                    device,
                    cached_layout: None,
                });
            }
        }
        Err(ProtocolError::DeviceNotFound {
            vid: AK820_VID,
            pid: AK820_PID,
        })
    }

    /// Retorna uma referência mutável ao handle de dispositivo HID subjacente.
    pub fn device_handle(&mut self) -> &mut hidapi::HidDevice {
        &mut self.device
    }

    /// Escreve um pacote bruto diretamente no dispositivo HID.
    pub fn write_raw(&mut self, data: &[u8]) -> Result<usize, ProtocolError> {
        self.device.write(data).map_err(ProtocolError::from)
    }

    /// Escreve um pacote de 64 bytes e retorna o relatório ACK completo (`sub_4781c0`).
    fn write_packet_with_ack_response(
        &mut self,
        pkt: &[u8; HID_REPORT_PAYLOAD_SIZE],
        timeout_ms: i32,
    ) -> Result<[u8; HID_REPORT_PAYLOAD_SIZE], ProtocolError> {
        self.device.write(pkt)?;
        let expected_opcode = pkt[3];
        let mut ack = [0u8; HID_REPORT_PAYLOAD_SIZE];
        for _ in 0..3 {
            let read = self.device.read_timeout(&mut ack, timeout_ms)?;
            if read == 0 {
                break;
            }
            if read >= 8 && ack[3] == expected_opcode {
                if ack[7] == 0xFF || ack[7] == 0xFE {
                    return Err(ProtocolError::CommandRejected {
                        opcode: expected_opcode,
                        status: ack[7],
                    });
                }
                return Ok(ack);
            }
        }
        Err(ProtocolError::ResponseTimeout {
            opcode: expected_opcode,
        })
    }

    /// Escreve um pacote de 64 bytes e lê o ACK correspondente na interface HID (`sub_4781c0`).
    fn write_packet_with_ack(
        &mut self,
        pkt: &[u8; HID_REPORT_PAYLOAD_SIZE],
        timeout_ms: i32,
    ) -> Result<(), ProtocolError> {
        match self.write_packet_with_ack_response(pkt, timeout_ms) {
            Ok(_) | Err(ProtocolError::ResponseTimeout { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Lê um buffer em chunks de até 56 bytes via `build_chunked_read_packets`.
    fn read_chunked(
        &mut self,
        opcode: u8,
        base_offset: u16,
        total_len: usize,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut out = Vec::with_capacity(total_len);
        for (_off, pkt) in build_chunked_read_packets(opcode, base_offset, total_len) {
            let chunk_len = pkt[4] as usize;
            let ack = self.write_packet_with_ack_response(&pkt, 300)?;
            out.extend_from_slice(&ack[8..8 + chunk_len]);
        }
        Ok(out)
    }

    /// Escreve um buffer em chunks de até 56 bytes aguardando ACK em cada pacote.
    fn write_chunked(
        &mut self,
        opcode: u8,
        base_offset: u16,
        data: &[u8],
    ) -> Result<(), ProtocolError> {
        for pkt in build_chunked_transfer_packets(opcode, base_offset, data) {
            self.write_packet_with_ack(&pkt, 300)?;
        }
        Ok(())
    }

    /// Executa transação `0x01 -> f() -> sleep(10ms) -> 0x02` (`sub_479200`).
    fn with_transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ProtocolError>,
    ) -> Result<T, ProtocolError> {
        let p_begin = build_begin_transaction_packet();
        self.write_packet_with_ack(&p_begin, 300)?;
        let res = f(self)?;
        std::thread::sleep(Duration::from_millis(10));
        let p_commit = build_commit_transaction_packet();
        self.write_packet_with_ack(&p_commit, 300)?;
        Ok(res)
    }

    /// Lê capacidades (`Opcode 0x03`: `total_keys` e blocos de macro).
    pub fn read_device_capabilities(&mut self) -> Result<DeviceCapabilities, ProtocolError> {
        let pkt = build_read_command_packet(OPCODE_GET_DEVICE_INFO, 0, 34);
        let ack = self.write_packet_with_ack_response(&pkt, 300)?;
        parse_device_info_block(&ack[8..42])
    }

    /// Lê a configuração atual (`Opcode 0x05`: `LightingConfig` e `SystemStages`).
    pub fn read_config(&mut self) -> Result<(LightingConfig, SystemStages), ProtocolError> {
        self.with_transaction(|dev| {
            let p_prof = build_read_command_packet(OPCODE_GET_CONFIG, 0, 1);
            let ack_prof = dev.write_packet_with_ack_response(&p_prof, 300)?;
            let active_profile = ack_prof[8] as u16;
            let p_cfg =
                build_read_command_packet(OPCODE_GET_CONFIG, active_profile.saturating_mul(0x1D), 0x1D);
            let ack_cfg = dev.write_packet_with_ack_response(&p_cfg, 300)?;
            parse_config_block(&ack_cfg[8..37])
        })
    }

    /// Obtém e memoriza `HardwareLayoutCache` (`0x03`, `0x1B`, `0x07`) em sessão.
    pub fn ensure_hardware_layout(&mut self) -> Result<HardwareLayoutCache, ProtocolError> {
        if let Some(cache) = &self.cached_layout {
            return Ok(cache.clone());
        }
        let cache = self.with_transaction(|dev| {
            let capabilities = dev.read_device_capabilities()?;
            let total_keys = capabilities.total_keys;
            let led_map = dev.read_chunked(OPCODE_GET_LED_MAPPING, 0, total_keys)?;
            let baseline_matrix = dev.read_chunked(OPCODE_GET_KEY_MATRIX, 0, total_keys * 3)?;
            Ok(HardwareLayoutCache {
                capabilities,
                led_map,
                baseline_matrix,
            })
        })?;
        self.cached_layout = Some(cache.clone());
        Ok(cache)
    }

    /// Envia sequência transacional `0x01 -> 0x06 -> 0x02` com `LightingConfig` e `SystemStages`.
    pub fn apply_config(
        &mut self,
        config: &LightingConfig,
        system: &SystemStages,
    ) -> Result<(), ProtocolError> {
        self.with_transaction(|dev| {
            let p_cfg = build_config_packet(config, system);
            dev.write_packet_with_ack(&p_cfg, 300)
        })
    }

    /// Envia uma sequência transacional completa de alteração de iluminação usando `LightingConfig`.
    pub fn apply_lighting(&mut self, config: &LightingConfig) -> Result<(), ProtocolError> {
        self.apply_config(config, &SystemStages::default())
    }

    /// Envia uma sequência transacional completa de alteração de iluminação (API simplificada).
    pub fn set_lighting(
        &mut self,
        mode: LightingMode,
        brightness: u8,
        speed: u8,
        color: RgbColor,
    ) -> Result<(), ProtocolError> {
        self.apply_lighting(&LightingConfig {
            mode,
            brightness,
            speed,
            color,
            ..LightingConfig::default()
        })
    }

    /// Define a cor estática global do teclado (Modo Normal, brilho máximo).
    pub fn set_static_color(&mut self, color: RgbColor) -> Result<(), ProtocolError> {
        self.apply_lighting(&LightingConfig {
            mode: LightingMode::Static,
            brightness: 4,
            speed: 0,
            color,
            ..LightingConfig::default()
        })
    }

    /// Ativa o modo de matriz customizada (Modo 20 / 0x14) no microcontrolador.
    pub fn enable_custom_lighting_mode(&mut self) -> Result<(), ProtocolError> {
        self.apply_lighting(&LightingConfig {
            mode: LightingMode::Custom,
            ..LightingConfig::default()
        })
    }

    /// Grava um banco Custom (`0..=2`) preservando `SystemStages` no pacote `0x06`.
    pub fn apply_custom_bank_with_system(
        &mut self,
        config: &LightingConfig,
        system: &SystemStages,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        let mut custom_cfg = *config;
        custom_cfg.mode = LightingMode::Custom;
        custom_cfg.normalize();

        self.apply_config(&custom_cfg, system)?;

        let packets = build_custom_matrix_packets_for_bank(custom_cfg.custom_group, colors);
        for pkt in &packets {
            self.write_packet_with_ack(pkt, 300)?;
        }
        Ok(())
    }

    /// Grava um banco Custom (`0..=2`) em modo estático por tecla conforme `sub_492b30` + `sub_49f280`:
    /// primeiro fecha a transação `0x01 -> 0x06 -> 0x02` com `mode = Custom` (`multicolor = false`) e em seguida
    /// envia os 7 pacotes `Opcode 0x0B` aguardando ACK HID por pacote (`sub_4781c0`).
    pub fn apply_custom_bank(
        &mut self,
        config: &LightingConfig,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        self.apply_custom_bank_with_system(config, &SystemStages::default(), colors)
    }

    /// Compila e grava macros (`0x15`) e matriz de remapeamento (`0x09`) no teclado via transações `0x01..0x02`.
    pub fn apply_keymap_and_macros(
        &mut self,
        overrides: &BTreeMap<u16, KeyAssignment>,
        macros: &[MacroDefinition],
    ) -> Result<(), ProtocolError> {
        let layout = self.ensure_hardware_layout()?;
        let max_macro_bytes = layout.capabilities.max_macro_bytes();
        let (macro_blob_opt, matrix_bytes) = compile_keymap_and_macros(
            &layout.baseline_matrix,
            &layout.led_map,
            overrides,
            macros,
            max_macro_bytes,
        )?;

        if let Some(blob) = macro_blob_opt {
            self.with_transaction(|dev| dev.write_chunked(OPCODE_SET_MACROS, 0, &blob))?;
        }

        self.with_transaction(|dev| dev.write_chunked(OPCODE_SET_KEY_MATRIX, 0, &matrix_bytes))?;

        if let Some(cache) = &mut self.cached_layout {
            cache.baseline_matrix = matrix_bytes;
        }
        Ok(())
    }

    /// Envia um quadro volátil em tempo real via `Opcode 0x12` (Modo Capture / Screen Sync), sem gravar na flash.
    pub fn send_live_matrix(
        &mut self,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
        brightness: u8,
    ) -> Result<(), ProtocolError> {
        let packets = build_live_matrix_packets(colors, brightness);
        for pkt in &packets {
            self.write_packet_with_ack(pkt, 80)?;
        }
        Ok(())
    }

    /// Envia um quadro completo de cores para o banco 0 via Opcode 0x0B sem Commit (uso reservado).
    pub fn set_custom_matrix(
        &mut self,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        let packets = build_custom_matrix_packets_for_bank(0, colors);
        for pkt in &packets {
            self.write_packet_with_ack(pkt, 300)?;
        }
        Ok(())
    }
}

/// Quadro de cores mapeado sobre a disposição física das teclas do dispositivo.
#[derive(Debug, Clone)]
pub struct KeyMatrixFrame {
    pub rows: usize,
    pub cols: usize,
    pub pixels: Vec<RgbColor>,
}

impl KeyMatrixFrame {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            pixels: vec![RgbColor::default(); rows * cols],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, row: usize, col: usize, color: RgbColor) {
        if row < self.rows && col < self.cols {
            self.pixels[row * self.cols + col] = color;
        }
    }
}

/// Definição estática de periférico suportado.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub name: &'static str,
    pub vid: u16,
    pub pid: u16,
    pub matrix_rows: usize,
    pub matrix_cols: usize,
}

pub const PROFILE_AK820: DeviceProfile = DeviceProfile {
    name: "AJAZZ AK820 (EVision)",
    vid: AK820_VID,
    pid: AK820_PID,
    matrix_rows: 6,
    matrix_cols: 16,
};

pub const PROFILE_MOUSE_NEARLINK: DeviceProfile = DeviceProfile {
    name: "AJAZZ NearLink Mouse",
    vid: MOUSE_NEARLINK_VID,
    pid: MOUSE_NEARLINK_PID,
    matrix_rows: 1,
    matrix_cols: 1,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_begin_transaction_packet() {
        let p = build_begin_transaction_packet();
        assert_eq!(p[0], 0x04);
        assert_eq!(p[1], 0x01); // Checksum LSB
        assert_eq!(p[2], 0x00); // Checksum MSB
        assert_eq!(p[3], 0x01); // Opcode 0x01
    }

    #[test]
    fn test_commit_transaction_packet() {
        let p = build_commit_transaction_packet();
        assert_eq!(p[0], 0x04);
        assert_eq!(p[1], 0x02); // Checksum LSB
        assert_eq!(p[2], 0x00); // Checksum MSB
        assert_eq!(p[3], 0x02); // Opcode 0x02
    }

    #[test]
    fn test_set_static_red_packet_matches_capture() {
        let p = build_lighting_packet(LightingMode::Static, 4, 4, RgbColor::new(0xFF, 0x00, 0x00));
        assert_eq!(p[0], 0x04);
        assert_eq!(p[1], 0x30); // Frame 117 Checksum LSB
        assert_eq!(p[2], 0x02); // Frame 117 Checksum MSB
        assert_eq!(p[3], 0x06); // Opcode 0x06
        assert_eq!(p[4], 0x1D);
        assert_eq!(p[9], 0x06); // Mode Static
        assert_eq!(p[10], 0x04); // Brightness 4
        assert_eq!(p[11], 0x04); // Speed 4
        assert_eq!(p[14], 0xFF); // R
        assert_eq!(p[15], 0x00); // G
        assert_eq!(p[16], 0x00); // B
    }

    #[test]
    fn test_set_blue_packet_matches_capture() {
        let p = build_lighting_packet(LightingMode::Static, 4, 4, RgbColor::new(0x31, 0x53, 0xFF));
        assert_eq!(p[0], 0x04);
        assert_eq!(p[1], 0xB4); // Frame 145 Checksum LSB
        assert_eq!(p[2], 0x02); // Frame 145 Checksum MSB
        assert_eq!(p[14], 0x31); // R
        assert_eq!(p[15], 0x53); // G
        assert_eq!(p[16], 0xFF); // B
    }

    #[test]
    fn test_custom_matrix_packets_structure() {
        let colors = [RgbColor::new(0xFF, 0x00, 0x00); TOTAL_PHYSICAL_LEDS];
        let packets = build_custom_matrix_packets(&colors);
        assert_eq!(packets.len(), 7);

        // Verifica os offsets e tamanhos dos 7 chunks
        let expected_offsets = [0, 56, 112, 168, 224, 280, 336];
        let expected_lens = [56, 56, 56, 56, 56, 56, 48];

        for i in 0..7 {
            assert_eq!(packets[i][0], REPORT_ID_COMMAND);
            assert_eq!(packets[i][3], OPCODE_SET_CUSTOM_RGB_MATRIX);
            assert_eq!(packets[i][4], expected_lens[i]);

            let offset = (packets[i][5] as usize) | ((packets[i][6] as usize) << 8);
            assert_eq!(offset, expected_offsets[i]);

            // Valida que o checksum calculado bate com os bytes 1 e 2
            let expected_sum = calculate_checksum(&packets[i]);
            let packet_sum = (packets[i][1] as u16) | ((packets[i][2] as u16) << 8);
            assert_eq!(packet_sum, expected_sum);
        }
    }

    #[test]
    fn test_lighting_config_packet_full_fields() {
        let cfg = LightingConfig {
            mode: LightingMode::Rain,
            brightness: 3,
            speed: 4, // deve serializar como 4 - 4 = 0
            direction: LightingDirection::Reverse,
            multicolor: true,
            color: RgbColor::new(0x10, 0x20, 0x30),
            zzcc_mode: 2,
            custom_group: 2,
        };
        let p = build_lighting_packet_config(&cfg);
        assert_eq!(p[9], 0x0F);
        assert_eq!(p[10], 3);
        assert_eq!(p[11], 0);
        assert_eq!(p[12], 1);
        assert_eq!(p[13], 1);
        assert_eq!((p[14], p[15], p[16]), (0x10, 0x20, 0x30));
        assert_eq!(p[26], 2);
        assert_eq!(p[27], 0);
        assert_eq!(p[28], 0xFF);
        assert_eq!(p[33], 2);
        let expected_sum = calculate_checksum(&p);
        assert_eq!((p[1] as u16) | ((p[2] as u16) << 8), expected_sum);

        let cap = build_lighting_packet_config(&LightingConfig {
            mode: LightingMode::Capture,
            ..LightingConfig::default()
        });
        assert_eq!(cap[9], 0xFE);
    }

    #[test]
    fn test_custom_bank_offset_stride() {
        let colors = [RgbColor::new(0x00, 0xFF, 0x80); TOTAL_PHYSICAL_LEDS];
        let bank1 = build_custom_matrix_packets_for_bank(1, &colors);
        let bank2 = build_custom_matrix_packets_for_bank(2, &colors);
        assert_eq!((bank1[0][5] as usize) | ((bank1[0][6] as usize) << 8), 0x0200);
        assert_eq!((bank2[0][5] as usize) | ((bank2[0][6] as usize) << 8), 0x0400);
    }

    #[test]
    fn test_config_packet_system_stages_roundtrip() {
        let cfg = LightingConfig {
            mode: LightingMode::Wave,
            brightness: 2,
            speed: 3,
            direction: LightingDirection::Reverse,
            multicolor: false,
            color: RgbColor::new(0x11, 0x22, 0x33),
            zzcc_mode: 1,
            custom_group: 1,
        };
        let sys = SystemStages {
            win_lock: true,
            wasd_swap: true,
            polling_rate: PollingRate::Hz250,
            key6_flag: 0x01,
            mac_mode: 1,
            wheel: 2,
        };
        let pkt = build_config_packet(&cfg, &sys);
        assert_eq!(pkt[27], 1); // WFlag
        assert_eq!(pkt[28], 1); // Key6Flag
        assert_eq!(pkt[29], 1); // WinFlag
        assert_eq!(pkt[30], 2); // PollingRate 250Hz
        assert_eq!(pkt[35], 1); // MacMode
        assert_eq!(pkt[36], 2); // Wheel

        let (parsed_cfg, parsed_sys) = parse_config_block(&pkt[8..37]).unwrap();
        assert_eq!(parsed_cfg, cfg);
        assert_eq!(parsed_sys, sys);
    }

    #[test]
    fn test_keymap_assignment_roundtrips() {
        let samples = [
            KeyAssignment::Disabled,
            KeyAssignment::KeyCombo {
                modifiers: 0x03,
                hid_usage: 0x04,
            },
            KeyAssignment::Mouse(MouseButton::Back),
            KeyAssignment::Media(MediaKey::PlayPause),
            KeyAssignment::System(SystemKey::Sleep),
            KeyAssignment::Layer(1),
            KeyAssignment::Fn,
        ];
        for sample in samples {
            let wire = sample.to_wire_bytes_with_macro_index(None).unwrap();
            let decoded = KeyAssignment::from_wire_bytes(wire);
            assert_eq!(decoded, sample);
        }
        assert_eq!(AK820_FACTORY_ASSIGNMENTS.len(), VISIBLE_KEYS_COUNT);
    }

    #[test]
    fn test_macro_container_and_compile_keymap_roundtrip() {
        let m1 = MacroDefinition {
            id: "m1".to_string(),
            name: "Copy".to_string(),
            steps: vec![
                MacroStep {
                    delay_ms: 15,
                    action: MacroAction::Modifier {
                        bitmask: 0x01,
                        down: true,
                    },
                },
                MacroStep {
                    delay_ms: 20,
                    action: MacroAction::Key {
                        hid_usage: 0x06,
                        down: true,
                    },
                },
                MacroStep {
                    delay_ms: 25,
                    action: MacroAction::MouseMoveX { delta: -12 },
                },
            ],
        };
        let blob = build_macro_container_blob(&[&m1], 512).unwrap();
        assert_eq!(blob[0], 0xAA);
        assert_eq!(blob[1], 0x55);
        let parsed = parse_macro_container_blob(&blob).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], m1.steps);

        // Simula matriz de 2 slots físicos: slot 0 -> phys_led da 1ª tecla (Esc = 0), slot 1 -> slot oculto (0xFF)
        let esc_led = AK820_KEY_LAYOUT[0].phys_led as u8;
        let led_map = vec![esc_led, 0xFF];
        let baseline = vec![0x20, 0x00, 0x29, 0xAA, 0xBB, 0xCC];
        let mut overrides = BTreeMap::new();
        overrides.insert(
            esc_led as u16,
            KeyAssignment::Macro {
                macro_id: "m1".to_string(),
                mode: MacroTriggerMode::Repeat(3),
            },
        );
        let (macro_opt, out_matrix) = compile_keymap_and_macros(
            &baseline,
            &led_map,
            &overrides,
            std::slice::from_ref(&m1),
            512,
        )
        .unwrap();
        assert!(macro_opt.is_some());
        assert_eq!(&out_matrix[0..3], &[0x71, 0x00, 0x03]); // Macro 0, Repeat 3
        assert_eq!(&out_matrix[3..6], &[0xAA, 0xBB, 0xCC]); // Slot oculto preservado

        // Restaura padrão de fábrica quando overrides está vazio
        let (no_macro, restored) =
            compile_keymap_and_macros(&out_matrix, &led_map, &BTreeMap::new(), &[m1], 512).unwrap();
        assert!(no_macro.is_none());
        assert_eq!(&restored[0..3], &AK820_FACTORY_ASSIGNMENTS[0]);
    }
}
