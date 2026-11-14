use crate::matrix::{AK820_KEY_LAYOUT, VISIBLE_KEYS_COUNT};
use crate::{ProtocolError, HID_REPORT_PAYLOAD_SIZE, REPORT_ID_COMMAND, calculate_checksum};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Tamanho máximo de carga útil por chunk nos comandos em bloco (`sub_479910`, `sub_479bd0`, etc.).
pub const TRANSFER_CHUNK_SIZE: usize = 56;

/// Assinaturas de fábrica de 3 bytes Big-Endian (`[b0, b1, b2]`) para as 82 teclas visíveis
/// do AK820 na ordem exata de `AK820_KEY_LAYOUT` (extraído da tabela `.rdata 0x68dd58`).
pub const AK820_FACTORY_ASSIGNMENTS: [[u8; 3]; VISIBLE_KEYS_COUNT] = [
    // Fileira 0: Esc, F1..F12, Del
    [0x20, 0x00, 0x29],
    [0x20, 0x00, 0x3A],
    [0x20, 0x00, 0x3B],
    [0x20, 0x00, 0x3C],
    [0x20, 0x00, 0x3D],
    [0x20, 0x00, 0x3E],
    [0x20, 0x00, 0x3F],
    [0x20, 0x00, 0x40],
    [0x20, 0x00, 0x41],
    [0x20, 0x00, 0x42],
    [0x20, 0x00, 0x43],
    [0x20, 0x00, 0x44],
    [0x20, 0x00, 0x45],
    [0x20, 0x00, 0x4C],
    // Fileira 1: `, 1..0, -, =, Backspace, Home
    [0x20, 0x00, 0x35],
    [0x20, 0x00, 0x1E],
    [0x20, 0x00, 0x1F],
    [0x20, 0x00, 0x20],
    [0x20, 0x00, 0x21],
    [0x20, 0x00, 0x22],
    [0x20, 0x00, 0x23],
    [0x20, 0x00, 0x24],
    [0x20, 0x00, 0x25],
    [0x20, 0x00, 0x26],
    [0x20, 0x00, 0x27],
    [0x20, 0x00, 0x2D],
    [0x20, 0x00, 0x2E],
    [0x20, 0x00, 0x2A],
    [0x20, 0x00, 0x4A],
    // Fileira 2: Tab, Q..P, [, ], \, PgUp
    [0x20, 0x00, 0x2B],
    [0x20, 0x00, 0x14],
    [0x20, 0x00, 0x1A],
    [0x20, 0x00, 0x08],
    [0x20, 0x00, 0x15],
    [0x20, 0x00, 0x17],
    [0x20, 0x00, 0x1C],
    [0x20, 0x00, 0x18],
    [0x20, 0x00, 0x0C],
    [0x20, 0x00, 0x12],
    [0x20, 0x00, 0x13],
    [0x20, 0x00, 0x2F],
    [0x20, 0x00, 0x30],
    [0x20, 0x00, 0x31],
    [0x20, 0x00, 0x4B],
    // Fileira 3: Caps, A..L, ;, ', Enter, PgDn
    [0x20, 0x00, 0x39],
    [0x20, 0x00, 0x04],
    [0x20, 0x00, 0x16],
    [0x20, 0x00, 0x07],
    [0x20, 0x00, 0x09],
    [0x20, 0x00, 0x0A],
    [0x20, 0x00, 0x0B],
    [0x20, 0x00, 0x0D],
    [0x20, 0x00, 0x0E],
    [0x20, 0x00, 0x0F],
    [0x20, 0x00, 0x33],
    [0x20, 0x00, 0x34],
    [0x20, 0x00, 0x28],
    [0x20, 0x00, 0x4E],
    // Fileira 4: LShift, Z..M, ,, ., /, RShift, End, Up
    [0x20, 0x02, 0x00],
    [0x20, 0x00, 0x1D],
    [0x20, 0x00, 0x1B],
    [0x20, 0x00, 0x06],
    [0x20, 0x00, 0x19],
    [0x20, 0x00, 0x05],
    [0x20, 0x00, 0x11],
    [0x20, 0x00, 0x10],
    [0x20, 0x00, 0x36],
    [0x20, 0x00, 0x37],
    [0x20, 0x00, 0x38],
    [0x20, 0x20, 0x00],
    [0x20, 0x00, 0x4D],
    [0x20, 0x00, 0x52],
    // Fileira 5: LCtrl, LWin, LAlt, Space, RAlt, Fn, RCtrl, Left, Down, Right
    [0x20, 0x01, 0x00],
    [0x20, 0x08, 0x00],
    [0x20, 0x04, 0x00],
    [0x20, 0x00, 0x2C],
    [0x20, 0x40, 0x00],
    [0xA0, 0x01, 0x00],
    [0x20, 0x10, 0x00],
    [0x20, 0x00, 0x50],
    [0x20, 0x00, 0x51],
    [0x20, 0x00, 0x4F],
];

/// Bitmask de modificadores padrão USB HID (`b1` nos registros `0x20 MM KK`).
pub mod modifiers {
    pub const LCTRL: u8 = 0x01;
    pub const LSHIFT: u8 = 0x02;
    pub const LALT: u8 = 0x04;
    pub const LWIN: u8 = 0x08;
    pub const RCTRL: u8 = 0x10;
    pub const RSHIFT: u8 = 0x20;
    pub const RALT: u8 = 0x40;
    pub const RWIN: u8 = 0x80;
}

/// Entrada canônica para seleção de teclas em UI e serialização de macros/remap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidKeyEntry {
    pub label: &'static str,
    pub modifiers: u8,
    pub hid_usage: u8,
}

/// Catálogo completo de teclas padrão e modificadores (`0x6c5e20`).
pub const HID_KEY_CATALOG: &[HidKeyEntry] = &[
    // Modificadores puros
    HidKeyEntry { label: "LCtrl", modifiers: modifiers::LCTRL, hid_usage: 0x00 },
    HidKeyEntry { label: "LShift", modifiers: modifiers::LSHIFT, hid_usage: 0x00 },
    HidKeyEntry { label: "LAlt", modifiers: modifiers::LALT, hid_usage: 0x00 },
    HidKeyEntry { label: "LWin", modifiers: modifiers::LWIN, hid_usage: 0x00 },
    HidKeyEntry { label: "RCtrl", modifiers: modifiers::RCTRL, hid_usage: 0x00 },
    HidKeyEntry { label: "RShift", modifiers: modifiers::RSHIFT, hid_usage: 0x00 },
    HidKeyEntry { label: "RAlt", modifiers: modifiers::RALT, hid_usage: 0x00 },
    HidKeyEntry { label: "RWin", modifiers: modifiers::RWIN, hid_usage: 0x00 },
    // Letras A..Z
    HidKeyEntry { label: "A", modifiers: 0, hid_usage: 0x04 },
    HidKeyEntry { label: "B", modifiers: 0, hid_usage: 0x05 },
    HidKeyEntry { label: "C", modifiers: 0, hid_usage: 0x06 },
    HidKeyEntry { label: "D", modifiers: 0, hid_usage: 0x07 },
    HidKeyEntry { label: "E", modifiers: 0, hid_usage: 0x08 },
    HidKeyEntry { label: "F", modifiers: 0, hid_usage: 0x09 },
    HidKeyEntry { label: "G", modifiers: 0, hid_usage: 0x0A },
    HidKeyEntry { label: "H", modifiers: 0, hid_usage: 0x0B },
    HidKeyEntry { label: "I", modifiers: 0, hid_usage: 0x0C },
    HidKeyEntry { label: "J", modifiers: 0, hid_usage: 0x0D },
    HidKeyEntry { label: "K", modifiers: 0, hid_usage: 0x0E },
    HidKeyEntry { label: "L", modifiers: 0, hid_usage: 0x0F },
    HidKeyEntry { label: "M", modifiers: 0, hid_usage: 0x10 },
    HidKeyEntry { label: "N", modifiers: 0, hid_usage: 0x11 },
    HidKeyEntry { label: "O", modifiers: 0, hid_usage: 0x12 },
    HidKeyEntry { label: "P", modifiers: 0, hid_usage: 0x13 },
    HidKeyEntry { label: "Q", modifiers: 0, hid_usage: 0x14 },
    HidKeyEntry { label: "R", modifiers: 0, hid_usage: 0x15 },
    HidKeyEntry { label: "S", modifiers: 0, hid_usage: 0x16 },
    HidKeyEntry { label: "T", modifiers: 0, hid_usage: 0x17 },
    HidKeyEntry { label: "U", modifiers: 0, hid_usage: 0x18 },
    HidKeyEntry { label: "V", modifiers: 0, hid_usage: 0x19 },
    HidKeyEntry { label: "W", modifiers: 0, hid_usage: 0x1A },
    HidKeyEntry { label: "X", modifiers: 0, hid_usage: 0x1B },
    HidKeyEntry { label: "Y", modifiers: 0, hid_usage: 0x1C },
    HidKeyEntry { label: "Z", modifiers: 0, hid_usage: 0x1D },
    // Números 1..0
    HidKeyEntry { label: "1", modifiers: 0, hid_usage: 0x1E },
    HidKeyEntry { label: "2", modifiers: 0, hid_usage: 0x1F },
    HidKeyEntry { label: "3", modifiers: 0, hid_usage: 0x20 },
    HidKeyEntry { label: "4", modifiers: 0, hid_usage: 0x21 },
    HidKeyEntry { label: "5", modifiers: 0, hid_usage: 0x22 },
    HidKeyEntry { label: "6", modifiers: 0, hid_usage: 0x23 },
    HidKeyEntry { label: "7", modifiers: 0, hid_usage: 0x24 },
    HidKeyEntry { label: "8", modifiers: 0, hid_usage: 0x25 },
    HidKeyEntry { label: "9", modifiers: 0, hid_usage: 0x26 },
    HidKeyEntry { label: "0", modifiers: 0, hid_usage: 0x27 },
    // Controle & pontuação
    HidKeyEntry { label: "Enter", modifiers: 0, hid_usage: 0x28 },
    HidKeyEntry { label: "Esc", modifiers: 0, hid_usage: 0x29 },
    HidKeyEntry { label: "Backspace", modifiers: 0, hid_usage: 0x2A },
    HidKeyEntry { label: "Tab", modifiers: 0, hid_usage: 0x2B },
    HidKeyEntry { label: "Space", modifiers: 0, hid_usage: 0x2C },
    HidKeyEntry { label: "-", modifiers: 0, hid_usage: 0x2D },
    HidKeyEntry { label: "=", modifiers: 0, hid_usage: 0x2E },
    HidKeyEntry { label: "[", modifiers: 0, hid_usage: 0x2F },
    HidKeyEntry { label: "]", modifiers: 0, hid_usage: 0x30 },
    HidKeyEntry { label: "\\", modifiers: 0, hid_usage: 0x31 },
    HidKeyEntry { label: ";", modifiers: 0, hid_usage: 0x33 },
    HidKeyEntry { label: "'", modifiers: 0, hid_usage: 0x34 },
    HidKeyEntry { label: "`", modifiers: 0, hid_usage: 0x35 },
    HidKeyEntry { label: ",", modifiers: 0, hid_usage: 0x36 },
    HidKeyEntry { label: ".", modifiers: 0, hid_usage: 0x37 },
    HidKeyEntry { label: "/", modifiers: 0, hid_usage: 0x38 },
    HidKeyEntry { label: "Caps", modifiers: 0, hid_usage: 0x39 },
    // Função F1..F12
    HidKeyEntry { label: "F1", modifiers: 0, hid_usage: 0x3A },
    HidKeyEntry { label: "F2", modifiers: 0, hid_usage: 0x3B },
    HidKeyEntry { label: "F3", modifiers: 0, hid_usage: 0x3C },
    HidKeyEntry { label: "F4", modifiers: 0, hid_usage: 0x3D },
    HidKeyEntry { label: "F5", modifiers: 0, hid_usage: 0x3E },
    HidKeyEntry { label: "F6", modifiers: 0, hid_usage: 0x3F },
    HidKeyEntry { label: "F7", modifiers: 0, hid_usage: 0x40 },
    HidKeyEntry { label: "F8", modifiers: 0, hid_usage: 0x41 },
    HidKeyEntry { label: "F9", modifiers: 0, hid_usage: 0x42 },
    HidKeyEntry { label: "F10", modifiers: 0, hid_usage: 0x43 },
    HidKeyEntry { label: "F11", modifiers: 0, hid_usage: 0x44 },
    HidKeyEntry { label: "F12", modifiers: 0, hid_usage: 0x45 },
    // Navegação e sistema
    HidKeyEntry { label: "PrintScreen", modifiers: 0, hid_usage: 0x46 },
    HidKeyEntry { label: "ScrollLock", modifiers: 0, hid_usage: 0x47 },
    HidKeyEntry { label: "Pause", modifiers: 0, hid_usage: 0x48 },
    HidKeyEntry { label: "Insert", modifiers: 0, hid_usage: 0x49 },
    HidKeyEntry { label: "Home", modifiers: 0, hid_usage: 0x4A },
    HidKeyEntry { label: "PgUp", modifiers: 0, hid_usage: 0x4B },
    HidKeyEntry { label: "Del", modifiers: 0, hid_usage: 0x4C },
    HidKeyEntry { label: "End", modifiers: 0, hid_usage: 0x4D },
    HidKeyEntry { label: "PgDn", modifiers: 0, hid_usage: 0x4E },
    HidKeyEntry { label: "Right", modifiers: 0, hid_usage: 0x4F },
    HidKeyEntry { label: "Left", modifiers: 0, hid_usage: 0x50 },
    HidKeyEntry { label: "Down", modifiers: 0, hid_usage: 0x51 },
    HidKeyEntry { label: "Up", modifiers: 0, hid_usage: 0x52 },
    // Numpad e App
    HidKeyEntry { label: "NumLock", modifiers: 0, hid_usage: 0x53 },
    HidKeyEntry { label: "Num/", modifiers: 0, hid_usage: 0x54 },
    HidKeyEntry { label: "Num*", modifiers: 0, hid_usage: 0x55 },
    HidKeyEntry { label: "Num-", modifiers: 0, hid_usage: 0x56 },
    HidKeyEntry { label: "Num+", modifiers: 0, hid_usage: 0x57 },
    HidKeyEntry { label: "NumEnter", modifiers: 0, hid_usage: 0x58 },
    HidKeyEntry { label: "Num1", modifiers: 0, hid_usage: 0x59 },
    HidKeyEntry { label: "Num2", modifiers: 0, hid_usage: 0x5A },
    HidKeyEntry { label: "Num3", modifiers: 0, hid_usage: 0x5B },
    HidKeyEntry { label: "Num4", modifiers: 0, hid_usage: 0x5C },
    HidKeyEntry { label: "Num5", modifiers: 0, hid_usage: 0x5D },
    HidKeyEntry { label: "Num6", modifiers: 0, hid_usage: 0x5E },
    HidKeyEntry { label: "Num7", modifiers: 0, hid_usage: 0x5F },
    HidKeyEntry { label: "Num8", modifiers: 0, hid_usage: 0x60 },
    HidKeyEntry { label: "Num9", modifiers: 0, hid_usage: 0x61 },
    HidKeyEntry { label: "Num0", modifiers: 0, hid_usage: 0x62 },
    HidKeyEntry { label: "Num.", modifiers: 0, hid_usage: 0x63 },
    HidKeyEntry { label: "Menu", modifiers: 0, hid_usage: 0x65 },
];

/// Botões de mouse suportados por remapeamento (`0x10 BB 00`) e ações de macro (`Type = 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0x01,
    Right = 0x02,
    Middle = 0x04,
    Back = 0x08,
    Forward = 0x10,
}

impl MouseButton {
    pub const ALL: [Self; 5] = [Self::Left, Self::Right, Self::Middle, Self::Back, Self::Forward];

    pub fn from_wire(byte: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|&b| (b as u8) == byte)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Mouse Esquerdo",
            Self::Right => "Mouse Direito",
            Self::Middle => "Mouse Meio",
            Self::Back => "Mouse Voltar",
            Self::Forward => "Mouse Avançar",
        }
    }
}

/// Comandos multimídia / Consumer Control HID Usage Page `0x0C` (`0x30 LL HH`, tabela `tab_7140`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MediaKey {
    MediaPlayer = 0x0183,
    PlayPause = 0x00CD,
    Stop = 0x00B7,
    PrevTrack = 0x00B6,
    NextTrack = 0x00B5,
    VolDown = 0x00EA,
    VolUp = 0x00E9,
    Mute = 0x00E2,
    WebHome = 0x0223,
    WebRefresh = 0x0227,
    WebStop = 0x0226,
    WebBack = 0x0224,
    WebForward = 0x0225,
    Bookmarks = 0x022A,
    Search = 0x0221,
    MyComputer = 0x0194,
    Calculator = 0x0192,
    Email = 0x018A,
}

impl MediaKey {
    pub const ALL: [Self; 18] = [
        Self::PlayPause,
        Self::Stop,
        Self::PrevTrack,
        Self::NextTrack,
        Self::VolUp,
        Self::VolDown,
        Self::Mute,
        Self::MediaPlayer,
        Self::Calculator,
        Self::MyComputer,
        Self::Email,
        Self::Search,
        Self::WebHome,
        Self::WebBack,
        Self::WebForward,
        Self::WebRefresh,
        Self::WebStop,
        Self::Bookmarks,
    ];

    pub fn from_usage(usage: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|&m| (m as u16) == usage)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::MediaPlayer => "Media Player",
            Self::PlayPause => "Play / Pause",
            Self::Stop => "Stop Mídia",
            Self::PrevTrack => "Faixa Anterior",
            Self::NextTrack => "Próxima Faixa",
            Self::VolDown => "Volume -",
            Self::VolUp => "Volume +",
            Self::Mute => "Mudo",
            Self::WebHome => "Navegador Home",
            Self::WebRefresh => "Atualizar Página",
            Self::WebStop => "Parar Página",
            Self::WebBack => "Navegador Voltar",
            Self::WebForward => "Navegador Avançar",
            Self::Bookmarks => "Favoritos",
            Self::Search => "Pesquisar",
            Self::MyComputer => "Meu Computador",
            Self::Calculator => "Calculadora",
            Self::Email => "E-mail",
        }
    }
}

/// Ações de sistema ACPI (`0x40 SS 00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SystemKey {
    Power = 0x01,
    Sleep = 0x02,
    Wake = 0x04,
}

impl SystemKey {
    pub const ALL: [Self; 3] = [Self::Power, Self::Sleep, Self::Wake];

    pub fn from_wire(byte: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|&k| (k as u8) == byte)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Power => "Desligar (Power)",
            Self::Sleep => "Suspender (Sleep)",
            Self::Wake => "Acordar (Wake)",
        }
    }
}

/// Modo de disparo de macro vinculada a uma tecla (`0x70 MM RR` ou `0x71 MM CC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MacroTriggerMode {
    /// `0x70 MM 00`: Executa uma vez ao pressionar (`Regular`).
    #[default]
    Once,
    /// `0x70 MM 01`: Repete e para quando soltar no fim do ciclo (`Release Stop`).
    HoldStop,
    /// `0x70 MM 02`: Alterna ligar/desligar a cada toque (`Toggle`).
    Toggle,
    /// `0x70 MM 03`: Interrompe imediatamente ao soltar a tecla (`Release Immediately`).
    HoldImmediateStop,
    /// `0x71 MM CC`: Repete exatamente `1..=255` vezes.
    Repeat(u8),
}

/// Atribuição configurável para qualquer tecla física do AK820 (`KeySet` 3 bytes Big-Endian).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyAssignment {
    Disabled,
    KeyCombo { modifiers: u8, hid_usage: u8 },
    Mouse(MouseButton),
    Media(MediaKey),
    System(SystemKey),
    Macro { macro_id: String, mode: MacroTriggerMode },
    Layer(u8),
    Fn,
    Raw([u8; 3]),
}

impl KeyAssignment {
    /// Codifica para registro de 3 bytes Big-Endian quando não depende de índice empacotado de macro.
    pub fn to_wire_bytes_with_macro_index(&self, packed_macro_idx: Option<u8>) -> Option<[u8; 3]> {
        match self {
            Self::Disabled => Some([0x20, 0x00, 0x00]),
            Self::KeyCombo { modifiers, hid_usage } => Some([0x20, *modifiers, *hid_usage]),
            Self::Mouse(btn) => Some([0x10, *btn as u8, 0x00]),
            Self::Media(m) => {
                let [lo, hi] = (*m as u16).to_le_bytes();
                Some([0x30, lo, hi])
            }
            Self::System(s) => Some([0x40, *s as u8, 0x00]),
            Self::Macro { mode, .. } => {
                let idx = packed_macro_idx?;
                match *mode {
                    MacroTriggerMode::Once => Some([0x70, idx, 0x00]),
                    MacroTriggerMode::HoldStop => Some([0x70, idx, 0x01]),
                    MacroTriggerMode::Toggle => Some([0x70, idx, 0x02]),
                    MacroTriggerMode::HoldImmediateStop => Some([0x70, idx, 0x03]),
                    MacroTriggerMode::Repeat(count) => Some([0x71, idx, count.max(1)]),
                }
            }
            Self::Layer(layer) => Some([0x72, *layer, 0x00]),
            Self::Fn => Some([0xA0, 0x01, 0x00]),
            Self::Raw(bytes) => Some(*bytes),
        }
    }

    /// Decodifica um registro bruto `[b0, b1, b2]` lido via `Opcode 0x07`.
    pub fn from_wire_bytes(bytes: [u8; 3]) -> Self {
        match bytes {
            [0x20, 0x00, 0x00] => Self::Disabled,
            [0x20, mods, hid] => Self::KeyCombo { modifiers: mods, hid_usage: hid },
            [0x10, btn, 0x00] => MouseButton::from_wire(btn)
                .map(Self::Mouse)
                .unwrap_or(Self::Raw(bytes)),
            [0x30, lo, hi] => MediaKey::from_usage(u16::from_le_bytes([lo, hi]))
                .map(Self::Media)
                .unwrap_or(Self::Raw(bytes)),
            [0x40, sys, 0x00] => SystemKey::from_wire(sys)
                .map(Self::System)
                .unwrap_or(Self::Raw(bytes)),
            [0x72, layer, 0x00] => Self::Layer(layer),
            [0xA0, 0x01, 0x00] => Self::Fn,
            other => Self::Raw(other),
        }
    }

    /// Rótulo curto legível para exibição sobre a tecla na GUI.
    pub fn short_label(&self) -> String {
        match self {
            Self::Disabled => "OFF".into(),
            Self::KeyCombo { modifiers, hid_usage } => {
                let mut parts: Vec<&str> = Vec::new();
                if *modifiers & modifiers::LCTRL != 0 || *modifiers & modifiers::RCTRL != 0 {
                    parts.push("Ctrl");
                }
                if *modifiers & modifiers::LSHIFT != 0 || *modifiers & modifiers::RSHIFT != 0 {
                    parts.push("Shift");
                }
                if *modifiers & modifiers::LALT != 0 || *modifiers & modifiers::RALT != 0 {
                    parts.push("Alt");
                }
                if *modifiers & modifiers::LWIN != 0 || *modifiers & modifiers::RWIN != 0 {
                    parts.push("Win");
                }
                if *hid_usage != 0 {
                    if let Some(entry) = HID_KEY_CATALOG
                        .iter()
                        .find(|e| e.modifiers == 0 && e.hid_usage == *hid_usage)
                    {
                        parts.push(entry.label);
                    } else {
                        return if parts.is_empty() {
                            format!("0x{:02X}", hid_usage)
                        } else {
                            format!("{}+0x{:02X}", parts.join("+"), hid_usage)
                        };
                    }
                } else if parts.len() == 1 {
                    if let Some(entry) = HID_KEY_CATALOG
                        .iter()
                        .find(|e| e.hid_usage == 0 && e.modifiers == *modifiers)
                    {
                        return entry.label.to_string();
                    }
                }
                parts.join("+")
            }
            Self::Mouse(b) => match b {
                MouseButton::Left => "M1".into(),
                MouseButton::Right => "M2".into(),
                MouseButton::Middle => "M3".into(),
                MouseButton::Back => "M4".into(),
                MouseButton::Forward => "M5".into(),
            },
            Self::Media(m) => m.label().to_string(),
            Self::System(s) => s.label().to_string(),
            Self::Macro { .. } => "Macro".into(),
            Self::Layer(l) => format!("Camada {}", l + 1),
            Self::Fn => "Fn".into(),
            Self::Raw(b) => format!("{:02X}{:02X}{:02X}", b[0], b[1], b[2]),
        }
    }
}

/// Retorna a assinatura de fábrica de 3 bytes para um `phys_led`, se pertencer às 82 teclas do AK820.
pub fn factory_assignment_for_phys_led(phys_led: u16) -> Option<[u8; 3]> {
    AK820_KEY_LAYOUT
        .iter()
        .position(|k| k.phys_led == phys_led as usize)
        .map(|idx| AK820_FACTORY_ASSIGNMENTS[idx])
}

/// Constrói pacotes OUT de 64 bytes em chunks de 56 bytes para transferência em bloco (`0x03`, `0x05`, `0x07`, `0x09`, `0x15`, `0x1B`).
pub fn build_chunked_transfer_packets(
    opcode: u8,
    base_offset: u16,
    data: &[u8],
) -> Vec<[u8; HID_REPORT_PAYLOAD_SIZE]> {
    let mut packets = Vec::with_capacity(data.len().div_ceil(TRANSFER_CHUNK_SIZE).max(1));
    if data.is_empty() {
        return packets;
    }

    let mut off = 0usize;
    while off < data.len() {
        let chunk_len = (data.len() - off).min(TRANSFER_CHUNK_SIZE);
        let wire_off = (base_offset as usize) + off;
        let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = opcode;
        pkt[4] = chunk_len as u8;
        pkt[5] = (wire_off & 0xFF) as u8;
        pkt[6] = ((wire_off >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;
        pkt[8..8 + chunk_len].copy_from_slice(&data[off..off + chunk_len]);

        let sum = calculate_checksum(&pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;
        packets.push(pkt);
        off += chunk_len;
    }
    packets
}

/// Constrói um comando de leitura em bloco (`0x03`, `0x05`, `0x07`, `0x1B`) para um intervalo `[offset, offset+len)`.
pub fn build_chunked_read_packets(
    opcode: u8,
    base_offset: u16,
    total_len: usize,
) -> Vec<(usize, [u8; HID_REPORT_PAYLOAD_SIZE])> {
    let mut packets = Vec::with_capacity(total_len.div_ceil(TRANSFER_CHUNK_SIZE));
    let mut off = 0usize;
    while off < total_len {
        let chunk_len = (total_len - off).min(TRANSFER_CHUNK_SIZE);
        let wire_off = (base_offset as usize) + off;
        let mut pkt = [0u8; HID_REPORT_PAYLOAD_SIZE];
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = opcode;
        pkt[4] = chunk_len as u8;
        pkt[5] = (wire_off & 0xFF) as u8;
        pkt[6] = ((wire_off >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;

        let sum = calculate_checksum(&pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;
        packets.push((chunk_len, pkt));
        off += chunk_len;
    }
    packets
}

/// Mapeia `phys_led -> hw_key_idx` usando a tabela `led_map` retornada por `Opcode 0x1B` (`GET_LED_MAPPING`).
pub fn resolve_phys_led_to_hw_slot(led_map: &[u8]) -> HashMap<u16, usize> {
    let mut map = HashMap::new();
    for (hw_slot, &phys_led) in led_map.iter().enumerate() {
        if phys_led != 0xFF {
            map.entry(phys_led as u16).or_insert(hw_slot);
        }
    }
    map
}

/// Compila os overrides de `UserProfile::keymap` sobre uma matriz base (`total_keys * 3` bytes lidos de `0x07`),
/// restaurando automaticamente as 82 teclas visíveis não sobrescritas para seu padrão de fábrica (`AK820_FACTORY_ASSIGNMENTS`).
pub fn compile_key_matrix(
    baseline_matrix: &[u8],
    led_map: &[u8],
    overrides: &BTreeMap<u16, KeyAssignment>,
    macro_index_by_id: &HashMap<&str, u8>,
) -> Result<Vec<u8>, ProtocolError> {
    let expected = led_map.len() * 3;
    if baseline_matrix.len() < expected {
        return Err(ProtocolError::InvalidPayloadSize {
            expected,
            received: baseline_matrix.len(),
        });
    }

    let mut out = baseline_matrix[..expected].to_vec();
    let phys_to_slot = resolve_phys_led_to_hw_slot(led_map);

    for (layout_idx, key) in AK820_KEY_LAYOUT.iter().enumerate() {
        let phys_led = key.phys_led as u16;
        let Some(&hw_slot) = phys_to_slot.get(&phys_led) else {
            continue;
        };
        let triplet_offset = hw_slot * 3;
        if triplet_offset + 3 > out.len() {
            continue;
        }

        let factory = AK820_FACTORY_ASSIGNMENTS[layout_idx];
        let bytes = match overrides.get(&phys_led) {
            Some(KeyAssignment::Macro { macro_id, .. }) => {
                let packed_idx = macro_index_by_id.get(macro_id.as_str()).copied();
                match overrides.get(&phys_led).and_then(|a| a.to_wire_bytes_with_macro_index(packed_idx)) {
                    Some(b) => b,
                    None => factory,
                }
            }
            Some(assign) => assign.to_wire_bytes_with_macro_index(None).unwrap_or(factory),
            None => factory,
        };
        out[triplet_offset..triplet_offset + 3].copy_from_slice(&bytes);
    }

    Ok(out)
}
