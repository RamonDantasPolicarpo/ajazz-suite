use std::fmt;
use thiserror::Error;

pub use hidapi;

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
pub const OPCODE_SET_LIGHTING: u8 = 0x06;
pub const OPCODE_KEY_MATRIX_MAPPING: u8 = 0x07;
pub const OPCODE_SET_CUSTOM_RGB_MATRIX: u8 = 0x0B;
pub const OPCODE_HEARTBEAT_POLL: u8 = 0x1A;
pub const OPCODE_GET_LED_MAPPING: u8 = 0x1B;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Falha na camada USB HID: {0}")]
    Hid(#[from] hidapi::HidError),

    #[error("Dispositivo não encontrado: VID=0x{vid:04X}, PID=0x{pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("Tamanho de payload inválido: esperado {expected}, recebido {received}")]
    InvalidPayloadSize { expected: usize, received: usize },
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
}

impl Default for UserProfile {
    fn default() -> Self {
        let empty = vec![RgbColor::default(); TOTAL_PHYSICAL_LEDS];
        Self {
            lighting: LightingConfig::default(),
            custom_banks: [empty.clone(), empty.clone(), empty],
        }
    }
}

impl UserProfile {
    /// Normaliza os vetores dos bancos Custom para exatamente `TOTAL_PHYSICAL_LEDS`.
    pub fn normalize(&mut self) {
        self.lighting.brightness = self.lighting.brightness.min(4);
        self.lighting.speed = self.lighting.speed.min(4);
        self.lighting.zzcc_mode = self.lighting.zzcc_mode.min(3);
        self.lighting.custom_group = self.lighting.custom_group.min(2);
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

/// Constrói o pacote completo de configuração de iluminação (`Opcode 0x06`, bloco `0x1D` bytes)
/// conforme extraído de `sub_492b30` / `LightInfo` / `SystemStages`.
pub fn build_lighting_packet_config(config: &LightingConfig) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut buf = [0u8; HID_REPORT_PAYLOAD_SIZE];
    buf[0] = REPORT_ID_COMMAND;
    buf[3] = OPCODE_SET_LIGHTING;
    buf[4] = 0x1D; // 29 bytes
    buf[9] = config.mode.wire_id();
    buf[10] = config.brightness.min(4);
    buf[11] = 4u8.saturating_sub(config.speed.min(4)); // sub_492b30: 4 - Speed
    buf[12] = config.direction as u8;
    buf[13] = u8::from(config.multicolor);
    buf[14] = config.color.r;
    buf[15] = config.color.g;
    buf[16] = config.color.b;
    buf[26] = config.zzcc_mode.min(3);
    // SystemStages seguros de `Keyboard.json` para não corromper taxa de polling / WinFlag
    buf[27] = 1; // Key6Flag
    buf[28] = 16; // Repeat (0x10)
    buf[33] = config.custom_group.min(2);

    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
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
    let mut packets = [[0u8; HID_REPORT_PAYLOAD_SIZE]; 7];
    const CHUNK_SIZE: usize = 56;
    let mut chunk_offset = 0usize;

    for pkt in packets.iter_mut() {
        let chunk_len = (raw_rgb.len() - chunk_offset).min(CHUNK_SIZE);
        let wire_offset = bank_base + chunk_offset;
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = OPCODE_SET_CUSTOM_RGB_MATRIX;
        pkt[4] = chunk_len as u8;
        pkt[5] = (wire_offset & 0xFF) as u8;
        pkt[6] = ((wire_offset >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;
        pkt[8..8 + chunk_len].copy_from_slice(&raw_rgb[chunk_offset..chunk_offset + chunk_len]);

        let sum = calculate_checksum(pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;

        chunk_offset += chunk_len;
    }

    packets
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
                return Ok(Self { device });
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

    /// Envia uma sequência transacional completa de alteração de iluminação usando `LightingConfig`.
    pub fn apply_lighting(&mut self, config: &LightingConfig) -> Result<(), ProtocolError> {
        let p_begin = build_begin_transaction_packet();
        self.device.write(&p_begin)?;

        let p_light = build_lighting_packet_config(config);
        self.device.write(&p_light)?;

        let p_commit = build_commit_transaction_packet();
        self.device.write(&p_commit)?;

        Ok(())
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

    /// Grava um banco Custom (`0..=2`) em modo estático por tecla via transação única fechada (`0x01` -> `0x06` -> `0x0B` -> `0x02`).
    pub fn apply_custom_bank(
        &mut self,
        config: &LightingConfig,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        let mut custom_cfg = *config;
        custom_cfg.mode = LightingMode::Custom;

        let p_begin = build_begin_transaction_packet();
        self.device.write(&p_begin)?;

        let p_light = build_lighting_packet_config(&custom_cfg);
        self.device.write(&p_light)?;

        let packets = build_custom_matrix_packets_for_bank(custom_cfg.custom_group, colors);
        for pkt in &packets {
            self.device.write(pkt)?;
        }

        let p_commit = build_commit_transaction_packet();
        self.device.write(&p_commit)?;
        Ok(())
    }

    /// Envia um quadro completo de cores para o banco 0 via Opcode 0x0B sem Commit (uso reservado).
    pub fn set_custom_matrix(
        &mut self,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        let packets = build_custom_matrix_packets_for_bank(0, colors);
        for pkt in &packets {
            self.device.write(pkt)?;
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
        assert_eq!(p[27], 1);
        assert_eq!(p[28], 16);
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
}
