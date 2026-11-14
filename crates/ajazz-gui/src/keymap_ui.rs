use ajazz_protocol::{
    modifiers, KeyAssignment, MacroAction, MacroDefinition, MacroTriggerMode, MediaKey, MouseButton,
    PollingRate, SystemKey, UserProfile, AK820_FACTORY_ASSIGNMENTS, AK820_KEY_LAYOUT,
    HID_KEY_CATALOG, VISIBLE_KEYS_COUNT,
};
use slint::platform::Key as SlintKey;
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

use crate::{GuiState, MacroStepItem, MainWindow};

/// Rótulos da lista unificada na categoria `Mídia / Sistema / Fn / Camadas`.
pub fn extra_action_labels() -> ModelRc<SharedString> {
    let mut items: Vec<SharedString> = MediaKey::ALL
        .iter()
        .map(|m| SharedString::from(m.label()))
        .collect();
    for sys in SystemKey::ALL {
        items.push(SharedString::from(sys.label()));
    }
    items.push(SharedString::from("Fn (Tecla Função)"));
    for layer in 0..3u8 {
        items.push(SharedString::from(format!("Camada / Perfil {}", layer + 1)));
    }
    ModelRc::from(Rc::new(VecModel::from(items)))
}

/// Rótulos do catálogo de teclas `HID_KEY_CATALOG`.
pub fn key_catalog_labels() -> ModelRc<SharedString> {
    let items: Vec<SharedString> = HID_KEY_CATALOG
        .iter()
        .map(|e| SharedString::from(e.label))
        .collect();
    ModelRc::from(Rc::new(VecModel::from(items)))
}

/// Constrói `KeyAssignment` para o índice da aba `Mídia / Sistema / Fn / Camadas`.
pub fn assignment_from_extra_index(idx: usize) -> Option<KeyAssignment> {
    let media_len = MediaKey::ALL.len();
    if idx < media_len {
        return Some(KeyAssignment::Media(MediaKey::ALL[idx]));
    }
    let sys_idx = idx - media_len;
    if sys_idx < SystemKey::ALL.len() {
        return Some(KeyAssignment::System(SystemKey::ALL[sys_idx]));
    }
    let fn_idx = sys_idx - SystemKey::ALL.len();
    if fn_idx == 0 {
        return Some(KeyAssignment::Fn);
    }
    let layer_idx = fn_idx - 1;
    if layer_idx < 3 {
        return Some(KeyAssignment::Layer(layer_idx as u8));
    }
    None
}

/// Retorna o índice na aba `Mídia / Sistema` correspondente a um `KeyAssignment`, se aplicável.
pub fn extra_index_from_assignment(a: &KeyAssignment) -> Option<usize> {
    match a {
        KeyAssignment::Media(m) => MediaKey::ALL.iter().position(|x| x == m),
        KeyAssignment::System(s) => SystemKey::ALL
            .iter()
            .position(|x| x == s)
            .map(|i| MediaKey::ALL.len() + i),
        KeyAssignment::Fn => Some(MediaKey::ALL.len() + SystemKey::ALL.len()),
        KeyAssignment::Layer(l) if (*l as usize) < 3 => {
            Some(MediaKey::ALL.len() + SystemKey::ALL.len() + 1 + (*l as usize))
        }
        _ => None,
    }
}

/// Traduz `KeyEvent.text` do Slint para `(modifiers, hid_usage)` de `HID_KEY_CATALOG`.
pub fn slint_key_to_hid_entry(text: &str) -> Option<(u8, u8)> {
    let ch = text.chars().next()?;
    let label = match ch {
        c if c == char::from(SlintKey::Escape) => "Esc",
        c if c == char::from(SlintKey::Tab) => "Tab",
        c if c == char::from(SlintKey::Backspace) => "Backspace",
        c if c == char::from(SlintKey::Return) || c == '\n' => "Enter",
        c if c == char::from(SlintKey::Space) => "Space",
        c if c == char::from(SlintKey::Delete) => "Del",
        c if c == char::from(SlintKey::Insert) => "Insert",
        c if c == char::from(SlintKey::Home) => "Home",
        c if c == char::from(SlintKey::End) => "End",
        c if c == char::from(SlintKey::PageUp) => "PgUp",
        c if c == char::from(SlintKey::PageDown) => "PgDn",
        c if c == char::from(SlintKey::LeftArrow) => "Left",
        c if c == char::from(SlintKey::RightArrow) => "Right",
        c if c == char::from(SlintKey::UpArrow) => "Up",
        c if c == char::from(SlintKey::DownArrow) => "Down",
        c if c == char::from(SlintKey::CapsLock) => "Caps",
        c if c == char::from(SlintKey::Shift) => "LShift",
        c if c == char::from(SlintKey::ShiftR) => "RShift",
        c if c == char::from(SlintKey::Control) => "LCtrl",
        c if c == char::from(SlintKey::ControlR) => "RCtrl",
        c if c == char::from(SlintKey::Alt) => "LAlt",
        c if c == char::from(SlintKey::AltGr) => "RAlt",
        c if c == char::from(SlintKey::Meta) => "LWin",
        c if c == char::from(SlintKey::MetaR) => "RWin",
        c if c == char::from(SlintKey::F1) => "F1",
        c if c == char::from(SlintKey::F2) => "F2",
        c if c == char::from(SlintKey::F3) => "F3",
        c if c == char::from(SlintKey::F4) => "F4",
        c if c == char::from(SlintKey::F5) => "F5",
        c if c == char::from(SlintKey::F6) => "F6",
        c if c == char::from(SlintKey::F7) => "F7",
        c if c == char::from(SlintKey::F8) => "F8",
        c if c == char::from(SlintKey::F9) => "F9",
        c if c == char::from(SlintKey::F10) => "F10",
        c if c == char::from(SlintKey::F11) => "F11",
        c if c == char::from(SlintKey::F12) => "F12",
        c if c == char::from(SlintKey::SysReq) => "PrintScreen",
        c if c == char::from(SlintKey::ScrollLock) => "ScrollLock",
        c if c == char::from(SlintKey::Pause) => "Pause",
        c if c == char::from(SlintKey::Menu) => "Menu",
        'a'..='z' | 'A'..='Z' => {
            let up = ch.to_ascii_uppercase();
            return HID_KEY_CATALOG
                .iter()
                .find(|e| e.modifiers == 0 && e.label.len() == 1 && e.label.starts_with(up))
                .map(|e| (e.modifiers, e.hid_usage));
        }
        '0'..='9' => {
            return HID_KEY_CATALOG
                .iter()
                .find(|e| e.modifiers == 0 && e.label.len() == 1 && e.label.starts_with(ch))
                .map(|e| (e.modifiers, e.hid_usage));
        }
        '-' | '_' => "-",
        '=' | '+' => "=",
        '[' | '{' => "[",
        ']' | '}' => "]",
        '\\' | '|' => "\\",
        ';' | ':' => ";",
        '\'' | '"' => "'",
        '`' | '~' => "`",
        ',' | '<' => ",",
        '.' | '>' => ".",
        '/' | '?' => "/",
        _ => return None,
    };

    HID_KEY_CATALOG
        .iter()
        .find(|e| e.label == label)
        .map(|e| (e.modifiers, e.hid_usage))
}

/// Constrói máscara de modificadores a partir das flags de evento do Slint.
pub fn modifier_mask_from_flags(ctrl: bool, shift: bool, alt: bool, meta: bool) -> u8 {
    let mut m = 0u8;
    if ctrl {
        m |= modifiers::LCTRL;
    }
    if shift {
        m |= modifiers::LSHIFT;
    }
    if alt {
        m |= modifiers::LALT;
    }
    if meta {
        m |= modifiers::LWIN;
    }
    m
}

/// Retorna a atribuição efetiva e se a tecla possui sobrescrita customizada.
pub fn effective_assignment(profile: &UserProfile, layout_idx: usize) -> (KeyAssignment, bool) {
    let phys_led = AK820_KEY_LAYOUT
        .get(layout_idx)
        .map(|k| k.phys_led as u16)
        .unwrap_or(0);
    if let Some(override_a) = profile.keymap.get(&phys_led) {
        (override_a.clone(), true)
    } else {
        let factory = AK820_FACTORY_ASSIGNMENTS
            .get(layout_idx)
            .copied()
            .unwrap_or([0x20, 0x00, 0x00]);
        (KeyAssignment::from_wire_bytes(factory), false)
    }
}

/// Atualiza estado do rascunho de remapeamento na UI quando o usuário seleciona uma tecla.
pub fn sync_selected_key_to_ui(ui: &MainWindow, profile: &UserProfile, layout_idx: usize) {
    let idx = layout_idx.min(VISIBLE_KEYS_COUNT.saturating_sub(1));
    let key_name = AK820_KEY_LAYOUT.get(idx).map(|k| k.name).unwrap_or("?");
    let (effective, is_remapped) = effective_assignment(profile, idx);

    let binding_desc = match &effective {
        KeyAssignment::Macro { macro_id, mode } => {
            let mname = profile
                .macros
                .iter()
                .find(|m| &m.id == macro_id)
                .map(|m| m.name.as_str())
                .unwrap_or(macro_id.as_str());
            let mdesc = match mode {
                MacroTriggerMode::Once => "1x",
                MacroTriggerMode::HoldStop => "Soltar",
                MacroTriggerMode::Toggle => "Toggle",
                MacroTriggerMode::HoldImmediateStop => "Imed.",
                MacroTriggerMode::Repeat(n) => {
                    return apply_selected_text_and_draft(
                        ui,
                        profile,
                        idx,
                        key_name,
                        &format!("Remapeado: Macro '{mname}' ({n}x)"),
                        is_remapped,
                        &effective,
                    );
                }
            };
            format!("Remapeado: Macro '{mname}' ({mdesc})")
        }
        other => {
            let prefix = if is_remapped { "Remapeado" } else { "Padrão" };
            format!("{prefix}: {}", other.short_label())
        }
    };

    apply_selected_text_and_draft(
        ui,
        profile,
        idx,
        key_name,
        &binding_desc,
        is_remapped,
        &effective,
    );
}

fn apply_selected_text_and_draft(
    ui: &MainWindow,
    profile: &UserProfile,
    idx: usize,
    key_name: &str,
    binding_desc: &str,
    is_remapped: bool,
    effective: &KeyAssignment,
) {
    ui.set_selected_key_idx(idx as i32);
    ui.set_km_selected_key_name(SharedString::from(key_name));
    ui.set_km_current_binding(SharedString::from(binding_desc));
    ui.set_km_selected_remapped(is_remapped);

    match effective {
        KeyAssignment::Disabled => ui.set_km_draft_category(4),
        KeyAssignment::KeyCombo {
            modifiers: mods,
            hid_usage,
        } => {
            ui.set_km_draft_category(0);
            if *hid_usage == 0 {
                let cat_idx = HID_KEY_CATALOG
                    .iter()
                    .position(|e| e.hid_usage == 0 && e.modifiers == *mods)
                    .unwrap_or(0);
                ui.set_km_draft_mod_mask(0);
                ui.set_km_draft_key_idx(cat_idx as i32);
            } else {
                let cat_idx = HID_KEY_CATALOG
                    .iter()
                    .position(|e| e.modifiers == 0 && e.hid_usage == *hid_usage)
                    .unwrap_or(8);
                ui.set_km_draft_mod_mask(*mods as i32);
                ui.set_km_draft_key_idx(cat_idx as i32);
            }
        }
        KeyAssignment::Mouse(btn) => {
            ui.set_km_draft_category(1);
            let m_idx = MouseButton::ALL.iter().position(|x| x == btn).unwrap_or(0);
            ui.set_km_draft_mouse_idx(m_idx as i32);
        }
        KeyAssignment::Media(_)
        | KeyAssignment::System(_)
        | KeyAssignment::Fn
        | KeyAssignment::Layer(_) => {
            ui.set_km_draft_category(2);
            if let Some(ex_idx) = extra_index_from_assignment(effective) {
                ui.set_km_draft_extra_idx(ex_idx as i32);
            }
        }
        KeyAssignment::Macro { macro_id, mode } => {
            ui.set_km_draft_category(3);
            let m_idx = profile
                .macros
                .iter()
                .position(|m| &m.id == macro_id)
                .unwrap_or(0);
            ui.set_km_draft_macro_idx(m_idx as i32);
            match *mode {
                MacroTriggerMode::Once => ui.set_km_draft_macro_mode(0),
                MacroTriggerMode::HoldStop => ui.set_km_draft_macro_mode(1),
                MacroTriggerMode::Toggle => ui.set_km_draft_macro_mode(2),
                MacroTriggerMode::HoldImmediateStop => ui.set_km_draft_macro_mode(3),
                MacroTriggerMode::Repeat(n) => {
                    ui.set_km_draft_macro_mode(4);
                    ui.set_km_draft_macro_repeat(n.max(1) as i32);
                }
            }
        }
        KeyAssignment::Raw(_) => ui.set_km_draft_category(0),
    }
}

/// Constrói o `KeyAssignment` a partir do formulário atual de `KeymapControls`.
pub fn build_assignment_from_ui(ui: &MainWindow, macros: &[MacroDefinition]) -> Option<KeyAssignment> {
    match ui.get_km_draft_category() {
        0 => {
            let entry = HID_KEY_CATALOG.get(ui.get_km_draft_key_idx().max(0) as usize)?;
            let mod_mask = (ui.get_km_draft_mod_mask().max(0) as u8) | entry.modifiers;
            Some(KeyAssignment::KeyCombo {
                modifiers: mod_mask,
                hid_usage: entry.hid_usage,
            })
        }
        1 => {
            let btn = MouseButton::ALL
                .get(ui.get_km_draft_mouse_idx().max(0) as usize)
                .copied()?;
            Some(KeyAssignment::Mouse(btn))
        }
        2 => assignment_from_extra_index(ui.get_km_draft_extra_idx().max(0) as usize),
        3 => {
            let mac = macros.get(ui.get_km_draft_macro_idx().max(0) as usize)?;
            let mode = match ui.get_km_draft_macro_mode() {
                1 => MacroTriggerMode::HoldStop,
                2 => MacroTriggerMode::Toggle,
                3 => MacroTriggerMode::HoldImmediateStop,
                4 => MacroTriggerMode::Repeat(ui.get_km_draft_macro_repeat().clamp(1, 255) as u8),
                _ => MacroTriggerMode::Once,
            };
            Some(KeyAssignment::Macro {
                macro_id: mac.id.clone(),
                mode,
            })
        }
        4 => Some(KeyAssignment::Disabled),
        _ => None,
    }
}

/// Sincroniza lista de macros, passos em rascunho, keymap indicators e SystemStages para a UI.
pub fn sync_keymap_macros_system_to_ui(ui: &MainWindow, st: &GuiState) {
    let remapped_flags: Vec<bool> = AK820_KEY_LAYOUT
        .iter()
        .map(|k| st.profile.keymap.contains_key(&(k.phys_led as u16)))
        .collect();
    ui.set_remapped_keys(ModelRc::from(Rc::new(VecModel::from(remapped_flags))));

    let macro_names: Vec<SharedString> = st
        .profile
        .macros
        .iter()
        .map(|m| SharedString::from(m.name.as_str()))
        .collect();
    ui.set_macro_names(ModelRc::from(Rc::new(VecModel::from(macro_names))));

    let sel_key = ui.get_selected_key_idx().clamp(0, (VISIBLE_KEYS_COUNT - 1) as i32) as usize;
    sync_selected_key_to_ui(ui, &st.profile, sel_key);

    // Sincroniza SystemStages
    ui.set_sys_win_lock(st.profile.system.win_lock);
    ui.set_sys_wasd_swap(st.profile.system.wasd_swap);
    let poll_idx = PollingRate::ALL
        .iter()
        .position(|&r| r == st.profile.system.polling_rate)
        .unwrap_or(0) as i32;
    ui.set_sys_polling_rate_idx(poll_idx);
}

/// Atualiza a lista de passos na aba `Macros` a partir de `st.draft_macro_steps`.
pub fn sync_macro_draft_to_ui(ui: &MainWindow, st: &GuiState) {
    ui.set_macro_selected_idx(st.selected_macro_idx);
    ui.set_macro_draft_name(SharedString::from(st.draft_macro_name.as_str()));

    let rows: Vec<MacroStepItem> = st
        .draft_macro_steps
        .iter()
        .enumerate()
        .map(|(i, s)| MacroStepItem {
            index_label: SharedString::from(format!("#{:02}", i + 1)),
            action_label: SharedString::from(s.action.label()),
            delay_ms: s.delay_ms as i32,
        })
        .collect();
    ui.set_macro_draft_steps(ModelRc::from(Rc::new(VecModel::from(rows))));
}

/// Converte pressão/soltura de tecla do Slint para `MacroAction`.
pub fn macro_action_from_slint_key(text: &str, down: bool) -> Option<MacroAction> {
    let (mods, usage) = slint_key_to_hid_entry(text)?;
    if usage == 0 && mods != 0 {
        Some(MacroAction::Modifier {
            bitmask: mods,
            down,
        })
    } else {
        Some(MacroAction::Key {
            hid_usage: usage,
            down,
        })
    }
}

/// Gera próximo atraso em ms durante gravação ao vivo (real ou fixo).
pub fn compute_record_delay_ms(st: &mut GuiState, use_fixed: bool, fixed_text: &str) -> u16 {
    if use_fixed {
        return fixed_text.trim().parse::<u16>().unwrap_or(25);
    }
    let now = std::time::Instant::now();
    let delay = match st.last_record_instant {
        Some(prev) => now.duration_since(prev).as_millis().min(65535) as u16,
        None => 0,
    };
    st.last_record_instant = Some(now);
    delay
}

/// Helper simples para criar ID único local de macro.
pub fn next_macro_id(existing: &[MacroDefinition]) -> (String, String) {
    let mut n = existing.len() + 1;
    loop {
        let id = format!("macro_{n}");
        if !existing.iter().any(|m| m.id == id) {
            return (id, format!("Macro {n}"));
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    #[test]
    fn test_extra_action_roundtrip() {
        let labels = extra_action_labels();
        assert_eq!(labels.row_count(), MediaKey::ALL.len() + SystemKey::ALL.len() + 4);
        for i in 0..labels.row_count() {
            let a = assignment_from_extra_index(i).expect("extra action");
            assert_eq!(extra_index_from_assignment(&a), Some(i));
        }
    }

    #[test]
    fn test_slint_key_translation() {
        assert_eq!(slint_key_to_hid_entry("a"), Some((0, 0x04)));
        assert_eq!(slint_key_to_hid_entry("Z"), Some((0, 0x1D)));
        assert_eq!(slint_key_to_hid_entry(&char::from(SlintKey::Escape).to_string()), Some((0, 0x29)));
        assert_eq!(slint_key_to_hid_entry(&char::from(SlintKey::F12).to_string()), Some((0, 0x45)));
    }
}
