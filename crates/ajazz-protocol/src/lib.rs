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

/// Modos de iluminação conhecidos do teclado AK820
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum LightingMode {
    Static = 0x06,
    Rain = 0x0F,
    Custom = 0x14, // Modo 20: Matriz Customizada / Individual por Tecla
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

/// Constrói o pacote de configuração de modo e cor de iluminação (Opcode 0x06).
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
    buf[9] = mode as u8;
    buf[10] = brightness;
    buf[11] = speed;
    buf[14] = color.r;
    buf[15] = color.g;
    buf[16] = color.b;
    // Slots de cor secundária/backup identificados na captura USB
    buf[28] = 0xFF;
    buf[29] = 0x00;
    buf[30] = 0x00;
    buf[33] = 0x01; // Flag de cor sólida

    let sum = calculate_checksum(&buf);
    buf[1] = (sum & 0xFF) as u8;
    buf[2] = ((sum >> 8) & 0xFF) as u8;
    buf
}

/// Constrói os 7 pacotes HID (Opcode 0x0B) contendo o array serializado de cores RGB da matriz.
/// Total de 384 bytes (128 posições x 3 bytes R, G, B).
pub fn build_custom_matrix_packets(
    colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
) -> [[u8; HID_REPORT_PAYLOAD_SIZE]; 7] {
    let mut raw_rgb = [0u8; TOTAL_PHYSICAL_LEDS * 3];
    for (i, c) in colors.iter().enumerate() {
        raw_rgb[i * 3] = c.r;
        raw_rgb[i * 3 + 1] = c.g;
        raw_rgb[i * 3 + 2] = c.b;
    }

    let mut packets = [[0u8; HID_REPORT_PAYLOAD_SIZE]; 7];
    const CHUNK_SIZE: usize = 56;
    let mut offset = 0usize;

    for pkt in packets.iter_mut() {
        let chunk_len = (raw_rgb.len() - offset).min(CHUNK_SIZE);
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = OPCODE_SET_CUSTOM_RGB_MATRIX;
        pkt[4] = chunk_len as u8;
        pkt[5] = (offset & 0xFF) as u8;
        pkt[6] = ((offset >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;
        pkt[8..8 + chunk_len].copy_from_slice(&raw_rgb[offset..offset + chunk_len]);

        let sum = calculate_checksum(pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;

        offset += chunk_len;
    }

    packets
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

    /// Envia uma sequência transacional completa de alteração de iluminação.
    pub fn set_lighting(
        &mut self,
        mode: LightingMode,
        brightness: u8,
        speed: u8,
        color: RgbColor,
    ) -> Result<(), ProtocolError> {
        let p_begin = build_begin_transaction_packet();
        self.device.write(&p_begin)?;

        let p_light = build_lighting_packet(mode, brightness, speed, color);
        self.device.write(&p_light)?;

        let p_commit = build_commit_transaction_packet();
        self.device.write(&p_commit)?;

        Ok(())
    }

    /// Define a cor estática global do teclado (Modo Normal, brilho e velocidade máximos).
    pub fn set_static_color(&mut self, color: RgbColor) -> Result<(), ProtocolError> {
        self.set_lighting(LightingMode::Static, 4, 4, color)
    }

    /// Ativa o modo de matriz customizada (Modo 20 / 0x14) no microcontrolador.
    pub fn enable_custom_lighting_mode(&mut self) -> Result<(), ProtocolError> {
        let p_begin = build_begin_transaction_packet();
        self.device.write(&p_begin)?;

        let mut p_mode = [0u8; HID_REPORT_PAYLOAD_SIZE];
        p_mode[0] = REPORT_ID_COMMAND;
        p_mode[3] = OPCODE_SET_LIGHTING;
        p_mode[4] = 0x1D;
        p_mode[9] = LightingMode::Custom as u8;
        p_mode[10] = 4; // Brilho máximo
        p_mode[11] = 4; // Velocidade
        let sum = calculate_checksum(&p_mode);
        p_mode[1] = (sum & 0xFF) as u8;
        p_mode[2] = ((sum >> 8) & 0xFF) as u8;
        self.device.write(&p_mode)?;
        Ok(())
    }

    /// Envia um quadro completo de cores para a matriz de 128 LEDs via Opcode 0x0B.
    ///
    /// OBSERVAÇÃO CRÍTICA (Prevenção de desgaste de Flash):
    /// Não emite Opcode 0x02 (Commit), operando diretamente no buffer volátil de RAM
    /// do controlador do teclado para permitir projeções a 25 FPS com vida útil infinita.
    pub fn set_custom_matrix(
        &mut self,
        colors: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    ) -> Result<(), ProtocolError> {
        let packets = build_custom_matrix_packets(colors);
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
}
