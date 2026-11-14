use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::keymap::{MouseButton, SystemKey};
use crate::macros::{MacroAction, MacroDefinition, MacroStep};
use crate::{HID_REPORT_PAYLOAD_SIZE, MOUSE_NEARLINK_VID, ProtocolError};

/// PIDs oficiais do mouse AJAZZ AJ179 NL P (`config.xml` / `aj179.xml`).
pub const MOUSE_NEARLINK_PID_24G: u16 = crate::MOUSE_NEARLINK_PID;
pub const MOUSE_NEARLINK_PID_USB: u16 = 0x6D2E;

/// Interface HID proprietária segura (`sub_449710`).
pub const MOUSE_VENDOR_USAGE_PAGE: u16 = 0xFFB2;
pub const MOUSE_VENDOR_USAGE: u16 = 0x01;

/// Interface DFU explicitamente bloqueada (ADR-0003).
pub const MOUSE_DFU_BLOCKED_USAGE_PAGE: u16 = 0xFFB1;

pub const MOUSE_REPORT_ID: u8 = 0x06;
pub const MOUSE_DPI_MIN: u16 = 50;
pub const MOUSE_DPI_MAX: u16 = 26_000;
pub const MOUSE_DPI_STEP: u16 = 50;
pub const MOUSE_MAX_DPI_STAGES: usize = 6;
pub const MOUSE_DEBOUNCE_MIN_MS: u8 = 1;
pub const MOUSE_DEBOUNCE_MAX_MS: u8 = 50;
pub const MOUSE_BUTTON_COUNT: usize = 5;
pub const MOUSE_MACRO_MAX_SLOTS: usize = 32;
pub const MOUSE_MACRO_HEADER_SIZE: usize = 0x40;
pub const MOUSE_MACRO_MAX_BYTES: usize = 0xFA0;
pub const MOUSE_MACRO_CHUNK_SIZE: usize = 59;
pub const MOUSE_MACRO_MAX_DELAY_MS: u16 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseConnectionMode {
    UsbWired,
    NearLink24G,
}

impl MouseConnectionMode {
    pub const fn pid(self) -> u16 {
        match self {
            Self::UsbWired => MOUSE_NEARLINK_PID_USB,
            Self::NearLink24G => MOUSE_NEARLINK_PID_24G,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::UsbWired => "USB Cabo",
            Self::NearLink24G => "2.4G NearLink",
        }
    }
}

/// Polling rate do AJ179 NL P (`0=125Hz .. 3=1000Hz` em `sub_41e1b0` e `sub_42f930`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum MousePollingRate {
    Hz125 = 0,
    Hz250 = 1,
    Hz500 = 2,
    #[default]
    Hz1000 = 3,
}

impl MousePollingRate {
    pub const ALL: [Self; 4] = [Self::Hz125, Self::Hz250, Self::Hz500, Self::Hz1000];

    pub const fn from_wire(val: u8) -> Self {
        match val {
            0 => Self::Hz125,
            1 => Self::Hz250,
            2 => Self::Hz500,
            _ => Self::Hz1000,
        }
    }

    pub const fn hz(self) -> u16 {
        match self {
            Self::Hz125 => 125,
            Self::Hz250 => 250,
            Self::Hz500 => 500,
            Self::Hz1000 => 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum LiftOffDistance {
    #[default]
    Low1Mm = 0,
    High2Mm = 1,
}

impl LiftOffDistance {
    pub const fn from_wire(val: u8) -> Self {
        if val == 0 { Self::Low1Mm } else { Self::High2Mm }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ScrollDirection {
    #[default]
    Forward = 0,
    Reverse = 1,
}

impl ScrollDirection {
    pub const fn from_wire(val: u8) -> Self {
        if val == 0 { Self::Forward } else { Self::Reverse }
    }
}

/// Bitmask `sensor_flag` (`rx[34]` / `06 13 07`): `bit0=Ripple, bit1=Linear, bit2=MotionSync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SensorFlags {
    pub ripple_control: bool,
    pub linear_correction: bool,
    pub motion_sync: bool,
}

impl SensorFlags {
    pub const fn from_wire(byte: u8) -> Self {
        Self {
            ripple_control: (byte & 0x01) != 0,
            linear_correction: (byte & 0x02) != 0,
            motion_sync: (byte & 0x04) != 0,
        }
    }

    pub const fn to_wire(self) -> u8 {
        (self.ripple_control as u8)
            | ((self.linear_correction as u8) << 1)
            | ((self.motion_sync as u8) << 2)
    }
}

pub const fn clamp_dpi(dpi: u16) -> u16 {
    let clamped = if dpi < MOUSE_DPI_MIN {
        MOUSE_DPI_MIN
    } else if dpi > MOUSE_DPI_MAX {
        MOUSE_DPI_MAX
    } else {
        dpi
    };
    let rem = clamped % MOUSE_DPI_STEP;
    if rem >= MOUSE_DPI_STEP / 2 {
        let up = clamped + (MOUSE_DPI_STEP - rem);
        if up > MOUSE_DPI_MAX { MOUSE_DPI_MAX } else { up }
    } else {
        clamped - rem
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseConfig {
    pub dpi_count: u8,
    pub dpi_index: u8,
    pub dpi_stages: [u16; MOUSE_MAX_DPI_STAGES],
    pub polling_rate: MousePollingRate,
    pub scroll_direction: ScrollDirection,
    pub lod: LiftOffDistance,
    pub sensor: SensorFlags,
    pub highspeed_mode: bool,
    pub debounce_ms: u8,
    pub light_sleep: u16,
    pub deep_sleep: u16,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            dpi_count: 4,
            dpi_index: 1,
            dpi_stages: [400, 800, 1600, 3200, 6400, 12800],
            polling_rate: MousePollingRate::Hz1000,
            scroll_direction: ScrollDirection::Forward,
            lod: LiftOffDistance::Low1Mm,
            sensor: SensorFlags {
                ripple_control: false,
                linear_correction: false,
                motion_sync: true,
            },
            highspeed_mode: true,
            debounce_ms: 8,
            light_sleep: 60,
            deep_sleep: 300,
        }
    }
}

impl MouseConfig {
    pub fn normalize(&mut self) {
        self.dpi_count = self.dpi_count.clamp(1, MOUSE_MAX_DPI_STAGES as u8);
        self.dpi_index = self.dpi_index.min(self.dpi_count - 1);
        for d in &mut self.dpi_stages {
            *d = clamp_dpi(*d);
        }
        self.debounce_ms = self
            .debounce_ms
            .clamp(MOUSE_DEBOUNCE_MIN_MS, MOUSE_DEBOUNCE_MAX_MS);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseStatus {
    pub connection_mode: MouseConnectionMode,
    pub online: bool,
    pub charging: bool,
    pub battery_percent: u8,
    pub mouse_fw: String,
    pub receiver_fw: Option<String>,
    pub raw_mode: u8,
    pub report_flag: u8,
    pub config: MouseConfig,
    /// Valores crus `u16_le` recebidos nos offsets `18..29` de `06 05` (para auditoria/diagnóstico).
    pub raw_dpi_words: [u16; MOUSE_MAX_DPI_STAGES],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseAsyncEvent {
    Reconnect,
    DpiStageChanged { index: u8 },
    BatteryUpdated { charging: bool, battery_percent: u8 },
    Special { code: u8, value: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButtonAction {
    Left,
    Right,
    Middle,
    DoubleClick,
    Back,
    Forward,
    WheelUp,
    WheelDown,
    DpiPlus,
    DpiMinus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MouseButtonSlot {
    Disable,
    Mouse(MouseButtonAction),
    Keyboard { modifiers: u8, key1: u8, key2: u8 },
    Media { usage: u16 },
    /// Ações ACPI de sistema (`40 <01|02|04> 00 00`: Power/Sleep/Wake via UsagePage 0x01 Usage 0x80).
    System(SystemKey),
    Macro { index: u8, loop_mode: u8, loop_count: u8 },
    Raw([u8; 4]),
}

impl<'de> Deserialize<'de> for MouseButtonSlot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        enum LegacyOrCurrent {
            Disable,
            Mouse(MouseButtonAction),
            Keyboard { modifiers: u8, key1: u8, key2: u8 },
            Media { usage: u16 },
            System(SystemKey),
            // Perfis antigos gravavam `SniperDpi` que na verdade emitia ACPI Power (0x40 01);
            // neutralizamos para `Disable` ao carregar para evitar hibernação acidental.
            SniperDpi {
                #[serde(rename = "level_mask")]
                _level_mask: u8,
            },
            Macro { index: u8, loop_mode: u8, loop_count: u8 },
            Raw([u8; 4]),
        }

        Ok(match LegacyOrCurrent::deserialize(deserializer)? {
            LegacyOrCurrent::Disable | LegacyOrCurrent::SniperDpi { .. } => Self::Disable,
            LegacyOrCurrent::Mouse(act) => Self::Mouse(act),
            LegacyOrCurrent::Keyboard {
                modifiers,
                key1,
                key2,
            } => Self::Keyboard {
                modifiers,
                key1,
                key2,
            },
            LegacyOrCurrent::Media { usage } => Self::Media { usage },
            LegacyOrCurrent::System(sys) => Self::System(sys),
            LegacyOrCurrent::Macro {
                index,
                loop_mode,
                loop_count,
            } => Self::Macro {
                index,
                loop_mode,
                loop_count,
            },
            LegacyOrCurrent::Raw(raw) => Self::Raw(raw),
        })
    }
}

impl MouseButtonSlot {
    pub const DEFAULT_FIVE: [Self; MOUSE_BUTTON_COUNT] = [
        Self::Mouse(MouseButtonAction::Left),
        Self::Mouse(MouseButtonAction::Right),
        Self::Mouse(MouseButtonAction::Middle),
        Self::Mouse(MouseButtonAction::Forward),
        Self::Mouse(MouseButtonAction::Back),
    ];

    pub const fn to_wire(&self) -> [u8; 4] {
        match *self {
            Self::Disable => [0, 0, 0, 0],
            Self::Mouse(act) => match act {
                MouseButtonAction::Left => [0x10, 0x01, 0x00, 0x00],
                MouseButtonAction::Right => [0x10, 0x02, 0x00, 0x00],
                MouseButtonAction::Middle => [0x10, 0x04, 0x00, 0x00],
                MouseButtonAction::DoubleClick => [0x10, 0x01, 0x02, 0x00],
                MouseButtonAction::Back => [0x10, 0x08, 0x00, 0x00],
                MouseButtonAction::Forward => [0x10, 0x10, 0x00, 0x00],
                MouseButtonAction::WheelUp => [0x10, 0x00, 0x00, 0xFE],
                MouseButtonAction::WheelDown => [0x10, 0x00, 0x00, 0x02],
                MouseButtonAction::DpiPlus => [0x1F, 0x01, 0x00, 0x00],
                MouseButtonAction::DpiMinus => [0x1F, 0x00, 0x00, 0x00],
            },
            Self::Keyboard { modifiers, key1, key2 } => {
                if modifiers == 0 && key1 >= 0xE0 && key1 <= 0xE7 {
                    [0x20, 1u8 << (key1 - 0xE0), 0x00, key2]
                } else {
                    [0x20, modifiers, key1, key2]
                }
            }
            Self::Media { usage } => {
                let [lo, hi] = usage.to_le_bytes();
                [0x30, lo, hi, 0x00]
            }
            Self::System(sys) => [0x40, sys as u8, 0x00, 0x00],
            Self::Macro {
                index,
                loop_mode,
                loop_count,
            } => [0x60, index, loop_mode, loop_count],
            Self::Raw(bytes) => bytes,
        }
    }
}

pub const CHATTERING_THRESHOLD_MIN_MS: u16 = 5;
pub const CHATTERING_THRESHOLD_MAX_MS: u16 = 150;
pub const CHATTERING_DEFAULT_THRESHOLD_MS: u16 = 45;

/// Configuração persistente do monitor de **Switch Chattering** do mouse no Daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatteringConfig {
    pub enabled: bool,
    pub threshold_ms: u16,
}

impl Default for ChatteringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_ms: CHATTERING_DEFAULT_THRESHOLD_MS,
        }
    }
}

impl ChatteringConfig {
    pub fn normalize(&mut self) {
        self.threshold_ms = self
            .threshold_ms
            .clamp(CHATTERING_THRESHOLD_MIN_MS, CHATTERING_THRESHOLD_MAX_MS);
    }
}

/// Métricas acumuladas de **Switch Chattering** para um botão físico do mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchChatteringButtonStats {
    pub button: MouseButton,
    pub total_clicks: u64,
    pub chatter_count: u64,
    pub last_interval_ms: Option<u32>,
    pub min_interval_ms: Option<u32>,
}

/// Snapshot instantâneo da telemetria de **Switch Chattering** mantida pelo Daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchChatteringSnapshot {
    pub enabled: bool,
    pub threshold_ms: u16,
    pub total_chatter_events: u64,
    pub last_chatter_button: Option<MouseButton>,
    pub last_chatter_interval_ms: Option<u32>,
    pub per_button: [SwitchChatteringButtonStats; MOUSE_BUTTON_COUNT],
}

impl Default for SwitchChatteringSnapshot {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_ms: CHATTERING_DEFAULT_THRESHOLD_MS,
            total_chatter_events: 0,
            last_chatter_button: None,
            last_chatter_interval_ms: None,
            per_button: MouseButton::ALL.map(|button| SwitchChatteringButtonStats {
                button,
                total_clicks: 0,
                chatter_count: 0,
                last_interval_ms: None,
                min_interval_ms: None,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseProfile {
    pub config: MouseConfig,
    pub buttons: [MouseButtonSlot; MOUSE_BUTTON_COUNT],
    pub chattering: ChatteringConfig,
}

impl Default for MouseProfile {
    fn default() -> Self {
        Self {
            config: MouseConfig::default(),
            buttons: MouseButtonSlot::DEFAULT_FIVE,
            chattering: ChatteringConfig::default(),
        }
    }
}

/// Garante que apenas a interface `0xFFB2 / 0x01` dos VID/PIDs do AJ179 seja aceita (ADR-0003).
pub const fn is_safe_mouse_interface(
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
) -> Option<MouseConnectionMode> {
    if vid != MOUSE_NEARLINK_VID || usage_page == MOUSE_DFU_BLOCKED_USAGE_PAGE {
        return None;
    }
    if usage_page != MOUSE_VENDOR_USAGE_PAGE || usage != MOUSE_VENDOR_USAGE {
        return None;
    }
    match pid {
        MOUSE_NEARLINK_PID_USB => Some(MouseConnectionMode::UsbWired),
        MOUSE_NEARLINK_PID_24G => Some(MouseConnectionMode::NearLink24G),
        _ => None,
    }
}

/// Bloqueia comandos destrutivos/DFU antes de tocar o barramento HID (`sub_431ce0` Factory Reset `06 0F ... FF`).
pub fn validate_outgoing_packet(pkt: &[u8; HID_REPORT_PAYLOAD_SIZE]) -> Result<(), ProtocolError> {
    if pkt[0] == 0x06 && pkt[1] == 0x0F && pkt[5] == 0xFF {
        return Err(ProtocolError::InvalidProtocolData(
            "Comando bloqueado pela política anti-brick (Factory Reset 06 0F ... FF)".into(),
        ));
    }
    if pkt[0] != 0x06 && !(pkt[0] == 0x52 && pkt[1] == 0x52) {
        return Err(ProtocolError::InvalidProtocolData(format!(
            "Prefixo de comando não autorizado para o mouse: 0x{:02X} 0x{:02X}",
            pkt[0], pkt[1]
        )));
    }
    Ok(())
}

/// Espelha `sub_44adb0`: remove byte inicial `0x00` (Report ID implícito no Windows) quando presente.
pub fn normalize_rx_report(raw: &[u8]) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut out = [0u8; HID_REPORT_PAYLOAD_SIZE];
    if raw.is_empty() {
        return out;
    }
    let slice = if raw[0] == 0x00 { &raw[1..] } else { raw };
    let n = slice.len().min(HID_REPORT_PAYLOAD_SIZE);
    out[..n].copy_from_slice(&slice[..n]);
    out
}

pub fn format_fw_version(major_minor_bcd: u8, patch: u8) -> String {
    let hi = (major_minor_bcd >> 4) & 0x0F;
    let lo = major_minor_bcd & 0x0F;
    format!("{hi}.{lo}.{patch}")
}

pub fn build_read_status_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = MOUSE_REPORT_ID;
    pkt[1] = 0x05;
    pkt
}

pub fn build_read_receiver_fw_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = MOUSE_REPORT_ID;
    pkt[1] = 0x07;
    pkt
}

pub fn build_param_set_packet(len: u8, param_id: u8, data: &[u8]) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = MOUSE_REPORT_ID;
    pkt[1] = 0x13;
    pkt[2] = len;
    pkt[5] = param_id;
    let max_copy = data.len().min(HID_REPORT_PAYLOAD_SIZE - 6);
    pkt[6..6 + max_copy].copy_from_slice(&data[..max_copy]);
    pkt
}

/// `sub_4312b0`: `[6]=dpi_count, [7]=dpi_index, [8..9]=dpi_value u16_le`.
pub fn build_set_dpi_packet(count: u8, index: u8, dpi_value: u16) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let count = count.clamp(1, MOUSE_MAX_DPI_STAGES as u8);
    let index = index.min(count - 1);
    let [lo, hi] = dpi_value.to_le_bytes();
    build_param_set_packet(0x05, 0x01, &[count, index, lo, hi])
}

pub fn build_set_polling_rate_packet(rate: MousePollingRate) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x02, 0x02, &[rate as u8])
}

/// `sub_431790`: envia `len=0x04` com `[6]=scroll_flag` e `[7..9]=00 00 00`.
pub fn build_set_scroll_packet(dir: ScrollDirection) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x04, 0x05, &[dir as u8])
}

pub fn build_set_lod_packet(lod: LiftOffDistance) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x02, 0x06, &[lod as u8])
}

pub fn build_set_sensor_flags_packet(flags: SensorFlags) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x02, 0x07, &[flags.to_wire()])
}

pub fn build_set_highspeed_packet(enabled: bool) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x02, 0x08, &[u8::from(enabled)])
}

pub fn build_start_pairing_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    build_param_set_packet(0x01, 0x09, &[0x00])
}

pub fn build_set_debounce_packet(ms: u8) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let clamped = ms.clamp(MOUSE_DEBOUNCE_MIN_MS, MOUSE_DEBOUNCE_MAX_MS);
    build_param_set_packet(0x02, 0x0A, &[clamped])
}

pub fn build_set_sleep_packet(light_sleep: u16, deep_sleep: u16) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let [l0, l1] = light_sleep.to_le_bytes();
    let [d0, d1] = deep_sleep.to_le_bytes();
    build_param_set_packet(0x05, 0x0B, &[l0, l1, d0, d1])
}

/// `sub_4310e0`: inicializa cabeçalho `06 12 20 00 00` + 9 slots padrão e grava cada slot em `5 + idx*4`.
pub fn build_set_keymap_packet(slots: &[MouseButtonSlot]) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = MOUSE_REPORT_ID;
    pkt[1] = 0x12;
    pkt[2] = 0x20;
    let defaults: [[u8; 4]; 9] = [
        [0x10, 0x01, 0x00, 0x00],
        [0x10, 0x02, 0x00, 0x00],
        [0x10, 0x04, 0x00, 0x00],
        [0x10, 0x10, 0x00, 0x00],
        [0x10, 0x08, 0x00, 0x00],
        [0x10, 0x20, 0x00, 0x00],
        [0x10, 0x40, 0x00, 0x00],
        [0x1F, 0x01, 0x00, 0x00],
        [0x1F, 0x00, 0x00, 0x00],
    ];
    for (idx, d) in defaults.iter().enumerate() {
        let base = 5 + idx * 4;
        pkt[base..base + 4].copy_from_slice(d);
    }
    for (idx, slot) in slots.iter().take(9).enumerate() {
        let base = 5 + idx * 4;
        pkt[base..base + 4].copy_from_slice(&slot.to_wire());
    }
    pkt
}

/// Garante que teclas, modificadores ou botões pressionados tenham um release correspondente (`sub_41dd70`).
fn pair_dangling_releases(steps: &[MacroStep]) -> Vec<MacroStep> {
    let mut out = steps.to_vec();
    let mut held_keys: Vec<u8> = Vec::new();
    let mut held_mods: Vec<u8> = Vec::new();
    let mut held_mouse: Vec<MouseButton> = Vec::new();

    for step in steps {
        match step.action {
            MacroAction::Key { hid_usage, down } => {
                if (0xE0..=0xE7).contains(&hid_usage) {
                    let mask = 1u8 << (hid_usage - 0xE0);
                    if down {
                        if !held_mods.contains(&mask) {
                            held_mods.push(mask);
                        }
                    } else {
                        held_mods.retain(|&m| m != mask);
                    }
                } else if down {
                    if !held_keys.contains(&hid_usage) {
                        held_keys.push(hid_usage);
                    }
                } else {
                    held_keys.retain(|&k| k != hid_usage);
                }
            }
            MacroAction::Modifier { bitmask, down } => {
                if down {
                    if !held_mods.contains(&bitmask) {
                        held_mods.push(bitmask);
                    }
                } else {
                    held_mods.retain(|&m| m != bitmask);
                }
            }
            MacroAction::MouseButton { button, down } => {
                if down {
                    if !held_mouse.contains(&button) {
                        held_mouse.push(button);
                    }
                } else {
                    held_mouse.retain(|&b| b != button);
                }
            }
            _ => {}
        }
    }

    for k in held_keys.into_iter().rev() {
        out.push(MacroStep {
            delay_ms: 10,
            action: MacroAction::Key {
                hid_usage: k,
                down: false,
            },
        });
    }
    for m in held_mods.into_iter().rev() {
        out.push(MacroStep {
            delay_ms: 10,
            action: MacroAction::Modifier {
                bitmask: m,
                down: false,
            },
        });
    }
    for b in held_mouse.into_iter().rev() {
        out.push(MacroStep {
            delay_ms: 10,
            action: MacroAction::MouseButton {
                button: b,
                down: false,
            },
        });
    }
    out
}

/// Codifica um passo de macro no formato de 4 bytes do AJ179 (`sub_430860`).
fn encode_mouse_macro_step(step: &MacroStep, is_first: bool) -> [u8; 4] {
    let delay = if is_first && step.delay_ms == 0 {
        10
    } else {
        step.delay_ms.min(MOUSE_MACRO_MAX_DELAY_MS)
    };
    let [d_lo, mut d_hi] = delay.to_le_bytes();
    let (flags, code) = match step.action {
        MacroAction::Key { hid_usage, down } => {
            if (0xE0..=0xE7).contains(&hid_usage) {
                let mask = 1u8 << (hid_usage - 0xE0);
                (if down { 0x41 } else { 0x01 }, mask)
            } else {
                (if down { 0x42 } else { 0x02 }, hid_usage)
            }
        }
        MacroAction::Modifier { bitmask, down } => (if down { 0x41 } else { 0x01 }, bitmask),
        MacroAction::MouseButton { button, down } => {
            (if down { 0x43 } else { 0x03 }, button as u8)
        }
        MacroAction::MouseMoveX { delta } => {
            let d = delta.clamp(-15, 15);
            if d < 0 {
                d_hi |= 0x80;
            }
            (0x06, ((d as u8) & 0x0F) << 4)
        }
        MacroAction::MouseMoveY { delta } => {
            let d = delta.clamp(-15, 15);
            if d < 0 {
                d_hi |= 0x40;
            }
            (0x06, (d as u8) & 0x0F)
        }
        MacroAction::Raw {
            event_type,
            down,
            button,
        } => {
            let wire_flags = if down {
                (event_type & 0xBF) | 0x40
            } else {
                event_type
            };
            (wire_flags, button)
        }
    };
    [d_lo, d_hi, flags, code]
}

/// Compila até 32 macros no contêiner binário de `sub_430860` (`[0x00..0x3F]` offsets LE + corpo `0x40..`, máx 4000 bytes).
pub fn compile_mouse_macros_container(
    macros: &[MacroDefinition],
) -> Result<Vec<u8>, ProtocolError> {
    let mut buf = vec![0u8; MOUSE_MACRO_HEADER_SIZE];
    for (slot_idx, macro_def) in macros.iter().take(MOUSE_MACRO_MAX_SLOTS).enumerate() {
        let normalized = pair_dangling_releases(&macro_def.steps);
        if normalized.is_empty() {
            continue;
        }
        let start_offset = buf.len();
        let needed = normalized.len() * 4 + 8;
        if start_offset + needed > MOUSE_MACRO_MAX_BYTES {
            return Err(ProtocolError::InvalidPayloadSize {
                expected: MOUSE_MACRO_MAX_BYTES,
                received: start_offset + needed,
            });
        }
        let [o_lo, o_hi] = (start_offset as u16).to_le_bytes();
        buf[slot_idx * 2] = o_lo;
        buf[slot_idx * 2 + 1] = o_hi;

        for (step_idx, step) in normalized.iter().enumerate() {
            buf.extend_from_slice(&encode_mouse_macro_step(step, step_idx == 0));
        }
        // Terminador obrigatório em [+4..+7] (`[10ms, 0x83, 0x01]`) + 4 bytes zero de padding (`sub_430860`).
        buf.extend_from_slice(&[0x0A, 0x00, 0x83, 0x01, 0x00, 0x00, 0x00, 0x00]);
    }
    Ok(buf)
}

/// Fatiamento `06 31` de até `59` (`0x3B`) bytes por pacote HID (`loc_430f3a..0x4310a2`).
pub fn build_upload_mouse_macros_packets(
    container: &[u8],
) -> Result<Vec<[u8; HID_REPORT_PAYLOAD_SIZE]>, ProtocolError> {
    if container.is_empty() || container.len() > MOUSE_MACRO_MAX_BYTES {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: MOUSE_MACRO_MAX_BYTES,
            received: container.len(),
        });
    }
    let total_len = container.len();
    let full_chunks = total_len / MOUSE_MACRO_CHUNK_SIZE;
    let num_packets = full_chunks + 1;
    let mut packets = Vec::with_capacity(num_packets);

    for chunk_idx in 0..num_packets {
        let offset = chunk_idx * MOUSE_MACRO_CHUNK_SIZE;
        let chunk_len = if chunk_idx == full_chunks {
            total_len - offset
        } else {
            MOUSE_MACRO_CHUNK_SIZE
        };
        let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
        pkt[0] = MOUSE_REPORT_ID;
        pkt[1] = 0x31;
        pkt[2] = chunk_len as u8;
        pkt[3..5].copy_from_slice(&(offset as u16).to_le_bytes());
        if chunk_len > 0 {
            pkt[5..5 + chunk_len].copy_from_slice(&container[offset..offset + chunk_len]);
        }
        packets.push(pkt);
    }
    Ok(packets)
}

pub fn build_apply_packet() -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = MOUSE_REPORT_ID;
    pkt[1] = 0x0F;
    pkt[2] = 0x01;
    pkt[5] = 0x08;
    pkt
}

pub fn build_profile_switch_packet(profile_id: u8) -> [u8; HID_REPORT_PAYLOAD_SIZE] {
    let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
    pkt[0] = 0x52;
    pkt[1] = 0x52;
    pkt[2] = profile_id;
    pkt
}

pub fn parse_status_response(
    rx: &[u8; HID_REPORT_PAYLOAD_SIZE],
    connection_mode: MouseConnectionMode,
    receiver_fw: Option<String>,
) -> Result<MouseStatus, ProtocolError> {
    if rx[9] == 0 && rx[10] == 0 {
        return Err(ProtocolError::InvalidProtocolData(
            "Resposta 06 05 ainda sem versão/estado válido do mouse (rx[9..10]==0)".into(),
        ));
    }

    let mut dpi_stages = [0u16; MOUSE_MAX_DPI_STAGES];
    let mut raw_dpi_words = [0u16; MOUSE_MAX_DPI_STAGES];
    for i in 0..MOUSE_MAX_DPI_STAGES {
        let base = 18 + i * 2;
        let raw = u16::from_le_bytes([rx[base], rx[base + 1]]);
        raw_dpi_words[i] = raw;
        dpi_stages[i] = raw.saturating_add(1).saturating_mul(MOUSE_DPI_STEP);
    }

    let mut config = MouseConfig {
        dpi_count: rx[16],
        dpi_index: rx[17],
        dpi_stages,
        polling_rate: MousePollingRate::from_wire(rx[30]),
        scroll_direction: ScrollDirection::from_wire(rx[32]),
        lod: LiftOffDistance::from_wire(rx[33]),
        sensor: SensorFlags::from_wire(rx[34]),
        highspeed_mode: rx[35] != 0,
        debounce_ms: rx[36],
        light_sleep: u16::from_le_bytes([rx[37], rx[38]]),
        deep_sleep: u16::from_le_bytes([rx[39], rx[40]]),
    };
    config.normalize();

    Ok(MouseStatus {
        connection_mode,
        online: rx[12] != 0,
        charging: rx[14] != 0,
        battery_percent: rx[13].min(100),
        mouse_fw: format_fw_version(rx[10], rx[9]),
        receiver_fw,
        raw_mode: rx[11],
        report_flag: rx[31],
        config,
        raw_dpi_words,
    })
}

pub fn parse_receiver_fw_response(rx: &[u8; HID_REPORT_PAYLOAD_SIZE]) -> Option<String> {
    if rx[5] == 1 {
        Some(format_fw_version(rx[10], rx[9]))
    } else {
        None
    }
}

pub fn parse_async_notify(rx: &[u8; HID_REPORT_PAYLOAD_SIZE]) -> Option<MouseAsyncEvent> {
    if rx[1] != 0xFA {
        return None;
    }
    match rx[5] {
        1 | 6 => Some(MouseAsyncEvent::Reconnect),
        2 => Some(MouseAsyncEvent::DpiStageChanged { index: rx[6] }),
        // `sub_434ae0` ignora `rx[6] == 0` para não zerar bateria em repouso/transição.
        3 if rx[6] != 0 => Some(MouseAsyncEvent::BatteryUpdated {
            battery_percent: rx[6].min(100),
            charging: rx[7] != 0,
        }),
        0x50 if rx[6] != 0 => Some(MouseAsyncEvent::Special {
            code: rx[6],
            value: rx[7],
        }),
        _ => None,
    }
}

pub struct Aj179MouseDevice {
    device: hidapi::HidDevice,
    connection_mode: MouseConnectionMode,
}

impl Aj179MouseDevice {
    /// Abre exclusivamente `UsagePage=0xFFB2, Usage=0x01`, priorizando Cabo USB (`0x6D2E`) sobre 2.4G (`0x642E`).
    pub fn open(api: &hidapi::HidApi) -> Result<Self, ProtocolError> {
        let mut fallback_24g = None;
        for d in api.device_list() {
            if let Some(mode) =
                is_safe_mouse_interface(d.vendor_id(), d.product_id(), d.usage_page(), d.usage())
            {
                match mode {
                    MouseConnectionMode::UsbWired => {
                        let device = d.open_device(api)?;
                        return Ok(Self {
                            device,
                            connection_mode: mode,
                        });
                    }
                    MouseConnectionMode::NearLink24G if fallback_24g.is_none() => {
                        fallback_24g = Some((d, mode));
                    }
                    _ => {}
                }
            }
        }
        if let Some((d, mode)) = fallback_24g {
            let device = d.open_device(api)?;
            return Ok(Self {
                device,
                connection_mode: mode,
            });
        }
        Err(ProtocolError::DeviceNotFound {
            vid: MOUSE_NEARLINK_VID,
            pid: MOUSE_NEARLINK_PID_24G,
        })
    }

    pub const fn connection_mode(&self) -> MouseConnectionMode {
        self.connection_mode
    }

    fn write_and_maybe_read(
        &mut self,
        pkt: &[u8; HID_REPORT_PAYLOAD_SIZE],
        expect_reply: bool,
        timeout_ms: i32,
    ) -> Result<[u8; HID_REPORT_PAYLOAD_SIZE], ProtocolError> {
        validate_outgoing_packet(pkt)?;
        self.device.write(pkt)?;
        let mut raw = [0u8; HID_REPORT_PAYLOAD_SIZE + 1];
        if !expect_reply {
            return Ok([0u8; HID_REPORT_PAYLOAD_SIZE]);
        }
        let n = self.device.read_timeout(&mut raw, timeout_ms)?;
        if n == 0 {
            return Err(ProtocolError::ResponseTimeout { opcode: pkt[1] });
        }
        Ok(normalize_rx_report(&raw[..n]))
    }

    pub fn read_receiver_fw(&mut self) -> Result<Option<String>, ProtocolError> {
        let pkt = build_read_receiver_fw_packet();
        match self.write_and_maybe_read(&pkt, true, 150) {
            Ok(rx) => Ok(parse_receiver_fw_response(&rx)),
            Err(ProtocolError::ResponseTimeout { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Leitura bruta de diagnóstico (`06 05`), retornando o buffer pós-normalização.
    pub fn read_status_raw(&mut self) -> Result<[u8; HID_REPORT_PAYLOAD_SIZE], ProtocolError> {
        let pkt = build_read_status_packet();
        for attempt in 0..5 {
            match self.write_and_maybe_read(&pkt, true, 150) {
                Ok(rx) if !(rx[9] == 0 && rx[10] == 0) => return Ok(rx),
                Ok(_) if attempt < 4 => thread::sleep(Duration::from_millis(20)),
                Ok(rx) => return Ok(rx),
                Err(ProtocolError::ResponseTimeout { .. }) if attempt < 4 => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    pub fn read_status(&mut self) -> Result<MouseStatus, ProtocolError> {
        let receiver_fw = if self.connection_mode == MouseConnectionMode::NearLink24G {
            self.read_receiver_fw().ok().flatten()
        } else {
            None
        };
        let rx = self.read_status_raw()?;
        parse_status_response(&rx, self.connection_mode, receiver_fw)
    }

    pub fn poll_notification(
        &mut self,
        timeout_ms: i32,
    ) -> Result<Option<MouseAsyncEvent>, ProtocolError> {
        let mut raw = [0u8; HID_REPORT_PAYLOAD_SIZE + 1];
        let n = self.device.read_timeout(&mut raw, timeout_ms)?;
        if n == 0 {
            return Ok(None);
        }
        let rx = normalize_rx_report(&raw[..n]);
        Ok(parse_async_notify(&rx))
    }

    pub fn set_dpi_stage(&mut self, count: u8, index: u8, dpi_value: u16) -> Result<(), ProtocolError> {
        let pkt = build_set_dpi_packet(count, index, dpi_value);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_polling_rate(&mut self, rate: MousePollingRate) -> Result<(), ProtocolError> {
        let pkt = build_set_polling_rate_packet(rate);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_scroll_direction(&mut self, dir: ScrollDirection) -> Result<(), ProtocolError> {
        let pkt = build_set_scroll_packet(dir);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_lod(&mut self, lod: LiftOffDistance) -> Result<(), ProtocolError> {
        let pkt = build_set_lod_packet(lod);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_sensor_flags(&mut self, flags: SensorFlags) -> Result<(), ProtocolError> {
        let pkt = build_set_sensor_flags_packet(flags);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_highspeed_mode(&mut self, enabled: bool) -> Result<(), ProtocolError> {
        let pkt = build_set_highspeed_packet(enabled);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_debounce_ms(&mut self, ms: u8) -> Result<(), ProtocolError> {
        let pkt = build_set_debounce_packet(ms);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_sleep_timers(&mut self, light_sleep: u16, deep_sleep: u16) -> Result<(), ProtocolError> {
        let pkt = build_set_sleep_packet(light_sleep, deep_sleep);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    pub fn set_keymap(&mut self, slots: &[MouseButtonSlot]) -> Result<(), ProtocolError> {
        let pkt = build_set_keymap_packet(slots);
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }

    /// Compila e envia o contêiner de macros (`06 31`) em fatias de 59 bytes com intervalo de 10 ms (`sub_430860`).
    pub fn upload_macros(&mut self, macros: &[MacroDefinition]) -> Result<(), ProtocolError> {
        let container = compile_mouse_macros_container(macros)?;
        let packets = build_upload_mouse_macros_packets(&container)?;
        for pkt in packets {
            self.write_and_maybe_read(&pkt, true, 150)?;
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn apply_refresh(&mut self) -> Result<(), ProtocolError> {
        let pkt = build_apply_packet();
        self.write_and_maybe_read(&pkt, false, 0).map(|_| ())
    }

    pub fn start_pairing(&mut self) -> Result<(), ProtocolError> {
        let pkt = build_start_pairing_packet();
        self.write_and_maybe_read(&pkt, true, 150).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dfu_interface_and_accepts_only_ffb2() {
        assert_eq!(
            is_safe_mouse_interface(0x7768, 0x642E, 0xFFB1, 0x01),
            None
        );
        assert_eq!(
            is_safe_mouse_interface(0x7768, 0x6D2E, 0xFFB1, 0x01),
            None
        );
        assert_eq!(
            is_safe_mouse_interface(0x7768, 0x642E, 0xFFB2, 0x02),
            None
        );
        assert_eq!(
            is_safe_mouse_interface(0x7768, 0x642E, 0xFFB2, 0x01),
            Some(MouseConnectionMode::NearLink24G)
        );
        assert_eq!(
            is_safe_mouse_interface(0x7768, 0x6D2E, 0xFFB2, 0x01),
            Some(MouseConnectionMode::UsbWired)
        );
    }

    #[test]
    fn blocks_factory_reset_and_untrusted_prefixes() {
        let mut reset = [0u8; 64];
        reset[0] = 0x06;
        reset[1] = 0x0F;
        reset[2] = 0x01;
        reset[5] = 0xFF;
        assert!(validate_outgoing_packet(&reset).is_err());

        let mut bad = [0u8; 64];
        bad[0] = 0xAA;
        assert!(validate_outgoing_packet(&bad).is_err());

        assert!(validate_outgoing_packet(&build_apply_packet()).is_ok());
    }

    #[test]
    fn builds_exact_disasm_packets() {
        let status = build_read_status_packet();
        assert_eq!(&status[..4], &[0x06, 0x05, 0x00, 0x00]);

        let dpi = build_set_dpi_packet(4, 1, 800);
        assert_eq!(
            &dpi[..10],
            &[0x06, 0x13, 0x05, 0x00, 0x00, 0x01, 0x04, 0x01, 0x20, 0x03]
        );

        let rate = build_set_polling_rate_packet(MousePollingRate::Hz500);
        assert_eq!(&rate[..7], &[0x06, 0x13, 0x02, 0x00, 0x00, 0x02, 0x02]);

        let scroll = build_set_scroll_packet(ScrollDirection::Reverse);
        assert_eq!(&scroll[..7], &[0x06, 0x13, 0x04, 0x00, 0x00, 0x05, 0x01]);

        let lod = build_set_lod_packet(LiftOffDistance::High2Mm);
        assert_eq!(&lod[..7], &[0x06, 0x13, 0x02, 0x00, 0x00, 0x06, 0x01]);

        let flags = SensorFlags {
            ripple_control: true,
            linear_correction: false,
            motion_sync: true,
        };
        let sensor = build_set_sensor_flags_packet(flags);
        assert_eq!(&sensor[..7], &[0x06, 0x13, 0x02, 0x00, 0x00, 0x07, 0x05]);

        let hs = build_set_highspeed_packet(true);
        assert_eq!(&hs[..7], &[0x06, 0x13, 0x02, 0x00, 0x00, 0x08, 0x01]);

        let pair = build_start_pairing_packet();
        assert_eq!(&pair[..7], &[0x06, 0x13, 0x01, 0x00, 0x00, 0x09, 0x00]);

        let debounce = build_set_debounce_packet(99);
        assert_eq!(&debounce[..7], &[0x06, 0x13, 0x02, 0x00, 0x00, 0x0A, 50]);

        let sleep = build_set_sleep_packet(60, 300);
        assert_eq!(
            &sleep[..10],
            &[0x06, 0x13, 0x05, 0x00, 0x00, 0x0B, 60, 0, 0x2C, 0x01]
        );

        let keymap = build_set_keymap_packet(&[
            MouseButtonSlot::Mouse(MouseButtonAction::DoubleClick),
            MouseButtonSlot::Macro {
                index: 2,
                loop_mode: 1,
                loop_count: 3,
            },
        ]);
        assert_eq!(&keymap[..5], &[0x06, 0x12, 0x20, 0x00, 0x00]);
        assert_eq!(&keymap[5..9], &[0x10, 0x01, 0x02, 0x00]);
        assert_eq!(&keymap[9..13], &[0x60, 0x02, 0x01, 0x03]);
        assert_eq!(&keymap[13..17], &[0x10, 0x04, 0x00, 0x00]);

        let apply = build_apply_packet();
        assert_eq!(&apply[..6], &[0x06, 0x0F, 0x01, 0x00, 0x00, 0x08]);
    }

    #[test]
    fn parses_status_and_notifications() {
        let mut rx = [0u8; 64];
        rx[0] = 0x06;
        rx[1] = 0x05;
        rx[9] = 4;
        rx[10] = 0x12; // 1.2.4
        rx[12] = 1; // online
        rx[13] = 85; // 85%
        rx[14] = 0; // not charging
        rx[16] = 4; // dpi_count
        rx[17] = 1; // dpi_index
        // (7+1)*50 = 400, (15+1)*50 = 800
        rx[18..20].copy_from_slice(&7u16.to_le_bytes());
        rx[20..22].copy_from_slice(&15u16.to_le_bytes());
        rx[30] = 3; // 1000Hz
        rx[32] = 1; // reverse
        rx[33] = 1; // 2.0mm
        rx[34] = 0b101; // ripple + motion_sync
        rx[35] = 1; // highspeed
        rx[36] = 6; // 6ms
        rx[37..39].copy_from_slice(&120u16.to_le_bytes());
        rx[39..41].copy_from_slice(&600u16.to_le_bytes());

        let parsed = parse_status_response(&rx, MouseConnectionMode::NearLink24G, None).unwrap();
        assert_eq!(parsed.mouse_fw, "1.2.4");
        assert!(parsed.online);
        assert_eq!(parsed.battery_percent, 85);
        assert_eq!(parsed.config.dpi_stages[0], 400);
        assert_eq!(parsed.config.dpi_stages[1], 800);
        assert_eq!(parsed.config.polling_rate, MousePollingRate::Hz1000);
        assert_eq!(parsed.config.scroll_direction, ScrollDirection::Reverse);
        assert_eq!(parsed.config.lod, LiftOffDistance::High2Mm);
        assert!(parsed.config.sensor.motion_sync);
        assert!(!parsed.config.sensor.linear_correction);
        assert_eq!(parsed.config.debounce_ms, 6);
        assert_eq!(parsed.config.light_sleep, 120);
        assert_eq!(parsed.config.deep_sleep, 600);

        let mut notify = [0u8; 64];
        notify[0] = 0x06;
        notify[1] = 0xFA;
        notify[5] = 3;
        notify[6] = 77;
        notify[7] = 1;
        assert_eq!(
            parse_async_notify(&notify),
            Some(MouseAsyncEvent::BatteryUpdated {
                charging: true,
                battery_percent: 77,
            })
        );

        notify[6] = 0;
        assert_eq!(parse_async_notify(&notify), None);
    }

    #[test]
    fn normalize_rx_strips_leading_zero_report_id() {
        let raw = [0x00, 0x06, 0x05, 0xAA];
        let norm = normalize_rx_report(&raw);
        assert_eq!(norm[..3], [0x06, 0x05, 0xAA]);

        let raw_no_zero = [0x06, 0x05, 0xBB];
        let norm2 = normalize_rx_report(&raw_no_zero);
        assert_eq!(norm2[..3], [0x06, 0x05, 0xBB]);
    }

    #[test]
    fn mouse_macro_container_matches_sub_430860_layout() {
        let macros = vec![
            MacroDefinition {
                id: "m1".into(),
                name: "Ctrl+A+Click".into(),
                steps: vec![
                    // Left Ctrl via HID 0xE0 -> modifier 0x41 mask 0x01; delay 0 -> 10ms no passo 0
                    MacroStep {
                        delay_ms: 0,
                        action: MacroAction::Key {
                            hid_usage: 0xE0,
                            down: true,
                        },
                    },
                    // 'A' (HID 0x04) -> normal key 0x42, delay 30ms
                    MacroStep {
                        delay_ms: 30,
                        action: MacroAction::Key {
                            hid_usage: 0x04,
                            down: true,
                        },
                    },
                    // Espera Release automático de 'A' e Ctrl (pair_dangling_releases)
                    // mais um clique Forward down/up
                    MacroStep {
                        delay_ms: 15,
                        action: MacroAction::MouseButton {
                            button: MouseButton::Forward,
                            down: true,
                        },
                    },
                    MacroStep {
                        delay_ms: 20,
                        action: MacroAction::MouseButton {
                            button: MouseButton::Forward,
                            down: false,
                        },
                    },
                ],
            },
            MacroDefinition {
                id: "m2".into(),
                name: "Move XY".into(),
                steps: vec![
                    MacroStep {
                        delay_ms: 12,
                        action: MacroAction::MouseMoveX { delta: -3 },
                    },
                    MacroStep {
                        delay_ms: 12,
                        action: MacroAction::MouseMoveY { delta: -2 },
                    },
                ],
            },
        ];

        let container = compile_mouse_macros_container(&macros).unwrap();
        // Slot 0 começa em 0x0040; Slot 2..31 permanecem 0x0000
        assert_eq!(u16::from_le_bytes([container[0], container[1]]), 0x0040);
        assert_eq!(u16::from_le_bytes([container[4], container[5]]), 0x0000);

        // Passo 0 (0x40): 10ms + 0x41 (Mod Down) + 0x01 (LCtrl bitmask)
        assert_eq!(&container[0x40..0x44], &[0x0A, 0x00, 0x41, 0x01]);
        // Passo 1 (0x44): 30ms + 0x42 (Key Down) + 0x04 ('A')
        assert_eq!(&container[0x44..0x48], &[0x1E, 0x00, 0x42, 0x04]);
        // Passo 2 (0x48): Forward Down (0x43, 0x10)
        assert_eq!(&container[0x48..0x4C], &[0x0F, 0x00, 0x43, 0x10]);
        // Passo 3 (0x4C): Forward Up (0x03, 0x10)
        assert_eq!(&container[0x4C..0x50], &[0x14, 0x00, 0x03, 0x10]);
        // Auto-release Key 'A' (0x50) e Modifier LCtrl (0x54)
        assert_eq!(&container[0x50..0x54], &[0x0A, 0x00, 0x02, 0x04]);
        assert_eq!(&container[0x54..0x58], &[0x0A, 0x00, 0x01, 0x01]);
        // Terminador [10ms, 0x83, 0x01] + 4 zero bytes padding em 0x58..0x60
        assert_eq!(
            &container[0x58..0x60],
            &[0x0A, 0x00, 0x83, 0x01, 0x00, 0x00, 0x00, 0x00]
        );

        // Slot 1 começa imediatamente após padding (0x0060)
        assert_eq!(u16::from_le_bytes([container[2], container[3]]), 0x0060);
        // Move X -3: delay 12 | 0x8000 em LE = [0x0C, 0x80], type 0x06, (-3 as u8 & 0x0F) << 4 = 0xD0
        assert_eq!(&container[0x60..0x64], &[0x0C, 0x80, 0x06, 0xD0]);
        // Move Y -2: delay 12 | 0x4000 em LE = [0x0C, 0x40], type 0x06, (-2 as u8 & 0x0F) = 0x0E
        assert_eq!(&container[0x64..0x68], &[0x0C, 0x40, 0x06, 0x0E]);
    }

    #[test]
    fn mouse_macro_chunking_uses_59_byte_packets() {
        // Container com 120 bytes => 3 pacotes: 59 + 59 + 2
        let container = vec![0xAAu8; 120];
        let pkts = build_upload_mouse_macros_packets(&container).unwrap();
        assert_eq!(pkts.len(), 3);
        assert_eq!(&pkts[0][..5], &[0x06, 0x31, 59, 0x00, 0x00]);
        assert_eq!(&pkts[1][..5], &[0x06, 0x31, 59, 59, 0x00]);
        assert_eq!(&pkts[2][..5], &[0x06, 0x31, 2, 118, 0x00]);
        assert_eq!(pkts[2][5], 0xAA);
        assert_eq!(pkts[2][6], 0xAA);
        assert_eq!(pkts[2][7], 0x00);

        // Tamanho exatamente divisível por 59 => pacote extra final com len=0 (`sub_430860`)
        let exact = vec![0x55u8; 118];
        let pkts_exact = build_upload_mouse_macros_packets(&exact).unwrap();
        assert_eq!(pkts_exact.len(), 3);
        assert_eq!(&pkts_exact[2][..5], &[0x06, 0x31, 0, 118, 0x00]);
    }

    #[test]
    fn mouse_macro_container_rejects_overflow_above_4000_bytes() {
        let too_many_steps: Vec<MacroStep> = (0..1000)
            .map(|_| MacroStep {
                delay_ms: 10,
                action: MacroAction::MouseButton {
                    button: MouseButton::Left,
                    down: true,
                },
            })
            .collect();
        let res = compile_mouse_macros_container(&[MacroDefinition {
            id: "huge".into(),
            name: "Huge".into(),
            steps: too_many_steps,
        }]);
        assert!(matches!(res, Err(ProtocolError::InvalidPayloadSize { .. })));
    }
}
