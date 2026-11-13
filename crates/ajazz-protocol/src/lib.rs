use std::fmt;
use thiserror::Error;

/// EVision Semiconductor (Fabricante do controlador USB do teclado físico AK820 detectado)
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

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Falha na camada USB HID: {0}")]
    Hid(#[from] hidapi::HidError),

    #[error("Dispositivo não encontrado: VID=0x{vid:04X}, PID=0x{pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("Tamanho de payload inválido: esperado {expected}, recebido {received}")]
    InvalidPayloadSize { expected: usize, received: usize },
}

/// Representa a cor de um LED individual na matriz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
