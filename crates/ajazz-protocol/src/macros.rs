use crate::keymap::{compile_key_matrix, KeyAssignment, MouseButton, HID_KEY_CATALOG};
use crate::ProtocolError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Magic little-endian do cabeçalho de contêiner de macros (`0x55AA` LE = `[0xAA, 0x55]`, `sub_491eb0`).
pub const MACRO_CONTAINER_MAGIC: u16 = 0x55AA;
pub const MACRO_HEADER_SIZE: usize = 16;

/// Ação atômica de um evento de macro (4 bytes no fio, `sub_448040`, `sub_41db60`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MacroAction {
    /// Tecla HID padrão (`Type = 10`, `button = hid_usage`).
    Key { hid_usage: u8, down: bool },
    /// Tecla modificadora (`Type = 9`, `button = modifier_bitmask`).
    Modifier { bitmask: u8, down: bool },
    /// Botão do mouse (`Type = 1`, `button = MouseButton`).
    MouseButton { button: MouseButton, down: bool },
    /// Movimento relativo horizontal (`Type = 4`; bit 7 = esquerda).
    MouseMoveX { delta: i8 },
    /// Movimento relativo vertical (`Type = 5`; bit 7 = cima).
    MouseMoveY { delta: i8 },
    /// Evento bruto para compatibilidade futura.
    Raw {
        event_type: u8,
        down: bool,
        button: u8,
    },
}

impl MacroAction {
    /// Codifica `(flags_type, button)` nos bytes `[2, 3]` de um evento de 4 bytes.
    pub fn to_wire_pair(&self) -> (u8, u8) {
        match *self {
            Self::Key { hid_usage, down } => (0x0A | if down { 0x80 } else { 0 }, hid_usage),
            Self::Modifier { bitmask, down } => (0x09 | if down { 0x80 } else { 0 }, bitmask),
            Self::MouseButton { button, down } => (0x01 | if down { 0x80 } else { 0 }, button as u8),
            Self::MouseMoveX { delta } => {
                let sign = if delta < 0 { 0x80 } else { 0x00 };
                (0x04 | sign, delta.unsigned_abs())
            }
            Self::MouseMoveY { delta } => {
                let sign = if delta < 0 { 0x80 } else { 0x00 };
                (0x05 | sign, delta.unsigned_abs())
            }
            Self::Raw {
                event_type,
                down,
                button,
            } => ((event_type & 0x7F) | if down { 0x80 } else { 0 }, button),
        }
    }

    /// Decodifica bytes `[2, 3]` de um evento de macro de 4 bytes.
    pub fn from_wire_pair(flags_type: u8, button: u8) -> Self {
        let down = (flags_type & 0x80) != 0;
        match flags_type & 0x7F {
            0x0A => Self::Key {
                hid_usage: button,
                down,
            },
            0x09 => Self::Modifier {
                bitmask: button,
                down,
            },
            0x01 => MouseButton::from_wire(button)
                .map(|b| Self::MouseButton { button: b, down })
                .unwrap_or(Self::Raw {
                    event_type: 0x01,
                    down,
                    button,
                }),
            0x04 => {
                let mag = button.min(127) as i8;
                Self::MouseMoveX {
                    delta: if down { -mag } else { mag },
                }
            }
            0x05 => {
                let mag = button.min(127) as i8;
                Self::MouseMoveY {
                    delta: if down { -mag } else { mag },
                }
            }
            other => Self::Raw {
                event_type: other,
                down,
                button,
            },
        }
    }

    pub fn label(&self) -> String {
        match *self {
            Self::Key { hid_usage, down } => {
                let dir = if down { "↓" } else { "↑" };
                let name = HID_KEY_CATALOG
                    .iter()
                    .find(|e| e.modifiers == 0 && e.hid_usage == hid_usage)
                    .map(|e| e.label)
                    .unwrap_or("HID");
                format!("{dir} {name}")
            }
            Self::Modifier { bitmask, down } => {
                let dir = if down { "↓" } else { "↑" };
                let name = HID_KEY_CATALOG
                    .iter()
                    .find(|e| e.hid_usage == 0 && e.modifiers == bitmask)
                    .map(|e| e.label)
                    .unwrap_or("Mod");
                format!("{dir} {name}")
            }
            Self::MouseButton { button, down } => {
                let dir = if down { "↓" } else { "↑" };
                format!("{dir} {}", button.label())
            }
            Self::MouseMoveX { delta } => format!("Move X {:+}", delta),
            Self::MouseMoveY { delta } => format!("Move Y {:+}", delta),
            Self::Raw {
                event_type,
                down,
                button,
            } => format!("Raw({event_type}, {}, 0x{button:02X})", if down { "D" } else { "U" }),
        }
    }
}

/// Passo individual de uma macro com atraso de 16 bits (`0..=65535 ms`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    pub delay_ms: u16,
    pub action: MacroAction,
}

/// Definição completa de uma macro editável e persistível no `UserProfile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroDefinition {
    pub id: String,
    pub name: String,
    pub steps: Vec<MacroStep>,
}

/// Serializa uma lista empacotada de macros no formato `0x55AA` LE exigido por `Opcode 0x15` (`sub_491eb0`).
pub fn build_macro_container_blob(
    macros: &[&MacroDefinition],
    max_bytes: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let macro_count = macros.len();
    let offset_table_bytes = macro_count * 2;
    let body_bytes: usize = macros.iter().map(|m| 4 + m.steps.len() * 4).sum();
    let total_bytes = MACRO_HEADER_SIZE + offset_table_bytes + body_bytes;

    if max_bytes > 0 && total_bytes > max_bytes {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: max_bytes,
            received: total_bytes,
        });
    }
    if total_bytes > u16::MAX as usize {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: u16::MAX as usize,
            received: total_bytes,
        });
    }

    let mut blob = vec![0u8; total_bytes];
    blob[0..2].copy_from_slice(&MACRO_CONTAINER_MAGIC.to_le_bytes());
    blob[2..4].copy_from_slice(&(total_bytes as u16).to_le_bytes());
    blob[4..6].copy_from_slice(&(macro_count as u16).to_le_bytes());
    // Bytes 6..16 permanecem zerados conforme cabeçalho de `sub_491eb0`.

    let mut cursor = MACRO_HEADER_SIZE + offset_table_bytes;
    for (idx, m) in macros.iter().enumerate() {
        let off_pos = MACRO_HEADER_SIZE + idx * 2;
        blob[off_pos..off_pos + 2].copy_from_slice(&(cursor as u16).to_le_bytes());

        let event_count = m.steps.len().min(u16::MAX as usize) as u16;
        blob[cursor..cursor + 2].copy_from_slice(&event_count.to_le_bytes());
        blob[cursor + 2] = 0;
        blob[cursor + 3] = 0;
        cursor += 4;

        for step in m.steps.iter().take(event_count as usize) {
            let [d_lo, d_hi] = step.delay_ms.to_le_bytes();
            let (flags_type, button) = step.action.to_wire_pair();
            blob[cursor] = d_lo;
            blob[cursor + 1] = d_hi;
            blob[cursor + 2] = flags_type;
            blob[cursor + 3] = button;
            cursor += 4;
        }
    }

    Ok(blob)
}

/// Faz o parsing de um contêiner binário `0x55AA` LE para verificação em testes e ferramentas.
pub fn parse_macro_container_blob(blob: &[u8]) -> Result<Vec<Vec<MacroStep>>, ProtocolError> {
    if blob.len() < MACRO_HEADER_SIZE {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: MACRO_HEADER_SIZE,
            received: blob.len(),
        });
    }
    let magic = u16::from_le_bytes([blob[0], blob[1]]);
    if magic != MACRO_CONTAINER_MAGIC {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: MACRO_CONTAINER_MAGIC as usize,
            received: magic as usize,
        });
    }
    let total_bytes = u16::from_le_bytes([blob[2], blob[3]]) as usize;
    if total_bytes > blob.len() {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: total_bytes,
            received: blob.len(),
        });
    }
    let macro_count = u16::from_le_bytes([blob[4], blob[5]]) as usize;
    let table_end = MACRO_HEADER_SIZE + macro_count * 2;
    if table_end > total_bytes {
        return Err(ProtocolError::InvalidPayloadSize {
            expected: table_end,
            received: total_bytes,
        });
    }

    let mut out = Vec::with_capacity(macro_count);
    for idx in 0..macro_count {
        let off = u16::from_le_bytes([
            blob[MACRO_HEADER_SIZE + idx * 2],
            blob[MACRO_HEADER_SIZE + idx * 2 + 1],
        ]) as usize;
        if off + 4 > total_bytes {
            return Err(ProtocolError::InvalidPayloadSize {
                expected: off + 4,
                received: total_bytes,
            });
        }
        let event_count = u16::from_le_bytes([blob[off], blob[off + 1]]) as usize;
        let events_end = off + 4 + event_count * 4;
        if events_end > total_bytes {
            return Err(ProtocolError::InvalidPayloadSize {
                expected: events_end,
                received: total_bytes,
            });
        }
        let mut steps = Vec::with_capacity(event_count);
        for e in 0..event_count {
            let p = off + 4 + e * 4;
            let delay_ms = u16::from_le_bytes([blob[p], blob[p + 1]]);
            let action = MacroAction::from_wire_pair(blob[p + 2], blob[p + 3]);
            steps.push(MacroStep { delay_ms, action });
        }
        out.push(steps);
    }
    Ok(out)
}

/// Empacota apenas as macros referenciadas no `keymap` (`sub_4918b0`), constrói o blob `0x15` (se houver macros)
/// e devolve `(Option<blob_0x15>, matriz_0x09)`.
pub fn compile_keymap_and_macros(
    baseline_matrix: &[u8],
    led_map: &[u8],
    overrides: &BTreeMap<u16, KeyAssignment>,
    macros: &[MacroDefinition],
    max_macro_bytes: usize,
) -> Result<(Option<Vec<u8>>, Vec<u8>), ProtocolError> {
    let mut packed_macros: Vec<&MacroDefinition> = Vec::new();
    let mut macro_index_by_id: HashMap<&str, u8> = HashMap::new();

    for assign in overrides.values() {
        if let KeyAssignment::Macro { macro_id, .. } = assign {
            if macro_index_by_id.contains_key(macro_id.as_str()) {
                continue;
            }
            if let Some(def) = macros.iter().find(|m| m.id == *macro_id) {
                let next_idx = packed_macros.len();
                if next_idx <= u8::MAX as usize {
                    macro_index_by_id.insert(def.id.as_str(), next_idx as u8);
                    packed_macros.push(def);
                }
            }
        }
    }

    let macro_blob = if packed_macros.is_empty() {
        None
    } else {
        Some(build_macro_container_blob(&packed_macros, max_macro_bytes)?)
    };

    let matrix_bytes = compile_key_matrix(baseline_matrix, led_map, overrides, &macro_index_by_id)?;
    Ok((macro_blob, matrix_bytes))
}
