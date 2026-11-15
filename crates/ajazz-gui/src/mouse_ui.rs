use crate::ipc_client::IpcRequest;
use crate::{keymap_ui, GuiState, MainWindow, MouseChatterRowItem};
use ajazz_protocol::mouse::{
    clamp_dpi, LiftOffDistance, MouseButtonAction, MouseButtonSlot, MouseConfig,
    MouseConnectionMode, MousePollingRate, MouseProfile, MouseStatus, ScrollDirection,
    SwitchChatteringSnapshot, CHATTERING_THRESHOLD_MAX_MS, CHATTERING_THRESHOLD_MIN_MS,
    MOUSE_BUTTON_COUNT, MOUSE_DEBOUNCE_MAX_MS, MOUSE_DEBOUNCE_MIN_MS, MOUSE_MAX_DPI_STAGES,
    MOUSE_MACRO_MAX_SLOTS,
};
use ajazz_protocol::{MacroDefinition, MediaKey, MouseButton, SystemKey, HID_KEY_CATALOG};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub const MOUSE_ACTIONS: [MouseButtonAction; 10] = [
    MouseButtonAction::Left,
    MouseButtonAction::Right,
    MouseButtonAction::Middle,
    MouseButtonAction::DoubleClick,
    MouseButtonAction::Forward,
    MouseButtonAction::Back,
    MouseButtonAction::WheelUp,
    MouseButtonAction::WheelDown,
    MouseButtonAction::DpiPlus,
    MouseButtonAction::DpiMinus,
];

const SLOT_TITLES: [&str; MOUSE_BUTTON_COUNT] = [
    "[0] CLIQUE ESQUERDO",
    "[1] CLIQUE DIREITO",
    "[2] BOTÃO DO MEIO",
    "[3] LATERAL AVANÇAR",
    "[4] LATERAL VOLTAR",
];


pub fn media_labels() -> ModelRc<SharedString> {
    let items: Vec<SharedString> = MediaKey::ALL
        .iter()
        .map(|m| SharedString::from(m.label()))
        .collect();
    ModelRc::from(Rc::new(VecModel::from(items)))
}

pub fn format_duration_secs(secs: u16) -> String {
    if secs == 0 {
        "Desligado".into()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs.is_multiple_of(60) {
        format!("{} min", secs / 60)
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

pub fn mouse_action_label(action: MouseButtonAction) -> &'static str {
    match action {
        MouseButtonAction::Left => "Clique Esq.",
        MouseButtonAction::Right => "Clique Dir.",
        MouseButtonAction::Middle => "Botão Meio",
        MouseButtonAction::DoubleClick => "Duplo Clique",
        MouseButtonAction::Forward => "Avançar",
        MouseButtonAction::Back => "Voltar",
        MouseButtonAction::WheelUp => "Scroll Cima",
        MouseButtonAction::WheelDown => "Scroll Baixo",
        MouseButtonAction::DpiPlus => "DPI +",
        MouseButtonAction::DpiMinus => "DPI -",
    }
}

pub fn slot_short_label_with_macros(slot: &MouseButtonSlot, macros: &[MacroDefinition]) -> String {
    match slot {
        MouseButtonSlot::Macro { index, .. } => macros
            .get(usize::from(*index))
            .map(|m| format!("Macro: {}", m.name))
            .unwrap_or_else(|| format!("Macro #{}", index + 1)),
        other => slot_short_label(other),
    }
}

pub fn slot_short_label(slot: &MouseButtonSlot) -> String {
    match slot {
        MouseButtonSlot::Disable => "Desabilitado".into(),
        MouseButtonSlot::Mouse(a) => mouse_action_label(*a).into(),
        MouseButtonSlot::Keyboard {
            modifiers,
            key1,
            key2,
        } => format_keyboard_slot(*modifiers, *key1, *key2),
        MouseButtonSlot::Media { usage } => MediaKey::from_usage(*usage)
            .map(|m| m.label().to_string())
            .unwrap_or_else(|| format!("Mídia 0x{usage:04X}")),
        MouseButtonSlot::System(sys) => match sys {
            SystemKey::Power => "Power (ACPI)".into(),
            SystemKey::Sleep => "Sleep (ACPI)".into(),
            SystemKey::Wake => "Wake (ACPI)".into(),
        },
        MouseButtonSlot::Macro { index, .. } => format!("Macro #{}", index + 1),
        MouseButtonSlot::Raw(b) => format!("Raw {:02X}{:02X}", b[0], b[1]),
    }
}

fn format_keyboard_slot(modifiers: u8, key1: u8, key2: u8) -> String {
    let mut parts = Vec::new();
    if modifiers & 0x11 != 0 {
        parts.push("Ctrl".to_string());
    }
    if modifiers & 0x22 != 0 {
        parts.push("Shift".to_string());
    }
    if modifiers & 0x44 != 0 {
        parts.push("Alt".to_string());
    }
    if modifiers & 0x88 != 0 {
        parts.push("Win".to_string());
    }
    for code in [key1, key2] {
        if code != 0 {
            let name = HID_KEY_CATALOG
                .iter()
                .find(|k| k.hid_usage == code)
                .map(|k| k.label.to_string())
                .unwrap_or_else(|| format!("0x{code:02X}"));
            parts.push(name);
        }
    }
    if parts.is_empty() {
        "Tecla".into()
    } else {
        parts.join("+")
    }
}

/// Garante que ao editar o `slot_idx`, ao menos um botão permaneça como `Clique Esquerdo`.
pub fn would_lose_left_click(
    buttons: &[MouseButtonSlot; MOUSE_BUTTON_COUNT],
    slot_idx: usize,
    next: &MouseButtonSlot,
) -> bool {
    if matches!(next, MouseButtonSlot::Mouse(MouseButtonAction::Left)) {
        return false;
    }
    !buttons
        .iter()
        .enumerate()
        .any(|(i, b)| i != slot_idx && matches!(b, MouseButtonSlot::Mouse(MouseButtonAction::Left)))
}

pub fn draft_button_from_ui(ui: &MainWindow) -> MouseButtonSlot {
    match ui.get_ms_btn_category() {
        0 => {
            let idx = ui.get_ms_btn_mouse_idx().max(0) as usize;
            MouseButtonSlot::Mouse(MOUSE_ACTIONS.get(idx).copied().unwrap_or(MouseButtonAction::Left))
        }
        1 => {
            let mod_mask = (ui.get_ms_btn_mod_mask() & 0xFF) as u8;
            let key_idx = ui.get_ms_btn_key_idx().max(0) as usize;
            let hid = HID_KEY_CATALOG
                .get(key_idx)
                .map(|k| k.hid_usage)
                .unwrap_or(0x04);
            MouseButtonSlot::Keyboard {
                modifiers: mod_mask,
                key1: hid,
                key2: 0,
            }
        }
        2 => {
            let idx = ui.get_ms_btn_media_idx().max(0) as usize;
            let usage = MediaKey::ALL
                .get(idx)
                .copied()
                .unwrap_or(MediaKey::PlayPause) as u16;
            MouseButtonSlot::Media { usage }
        }
        3 => {
            let idx = (ui.get_ms_btn_system_idx().max(0) as usize).min(SystemKey::ALL.len() - 1);
            MouseButtonSlot::System(SystemKey::ALL[idx])
        }
        4 => {
            let index = ui
                .get_ms_btn_macro_idx()
                .clamp(0, (MOUSE_MACRO_MAX_SLOTS - 1) as i32) as u8;
            let loop_mode = ui.get_ms_btn_macro_mode().clamp(0, 2) as u8;
            let loop_count = if loop_mode == 0 {
                ui.get_ms_btn_macro_repeat().clamp(1, 99) as u8
            } else {
                1
            };
            MouseButtonSlot::Macro {
                index,
                loop_mode,
                loop_count,
            }
        }
        _ => MouseButtonSlot::Disable,
    }
}

pub fn update_left_click_warning(
    ui: &MainWindow,
    buttons: &[MouseButtonSlot; MOUSE_BUTTON_COUNT],
) {
    let slot = (ui.get_ms_selected_slot().max(0) as usize).min(MOUSE_BUTTON_COUNT - 1);
    let candidate = draft_button_from_ui(ui);
    ui.set_ms_btn_left_warning(would_lose_left_click(buttons, slot, &candidate));
}

pub fn sync_selected_button_to_ui(
    ui: &MainWindow,
    buttons: &[MouseButtonSlot; MOUSE_BUTTON_COUNT],
    macros: &[MacroDefinition],
    slot_idx: usize,
) {
    let idx = slot_idx.min(MOUSE_BUTTON_COUNT - 1);
    let slot = &buttons[idx];
    ui.set_ms_selected_slot(idx as i32);
    ui.set_ms_slot_title(SharedString::from(SLOT_TITLES[idx]));
    ui.set_ms_slot_current_binding(SharedString::from(slot_short_label_with_macros(slot, macros)));
    ui.set_ms_btn_capturing_key(false);

    match slot {
        MouseButtonSlot::Mouse(act) => {
            ui.set_ms_btn_category(0);
            let a_idx = MOUSE_ACTIONS.iter().position(|a| a == act).unwrap_or(0);
            ui.set_ms_btn_mouse_idx(a_idx as i32);
        }
        MouseButtonSlot::Keyboard {
            modifiers,
            key1,
            ..
        } => {
            ui.set_ms_btn_category(1);
            ui.set_ms_btn_mod_mask(*modifiers as i32);
            let k_idx = HID_KEY_CATALOG
                .iter()
                .position(|k| k.hid_usage == *key1)
                .unwrap_or(0);
            ui.set_ms_btn_key_idx(k_idx as i32);
        }
        MouseButtonSlot::Media { usage } => {
            ui.set_ms_btn_category(2);
            let m_idx = MediaKey::ALL
                .iter()
                .position(|m| (*m as u16) == *usage)
                .unwrap_or(0);
            ui.set_ms_btn_media_idx(m_idx as i32);
        }
        MouseButtonSlot::System(sys) => {
            ui.set_ms_btn_category(3);
            let s_idx = SystemKey::ALL
                .iter()
                .position(|k| k == sys)
                .unwrap_or(0);
            ui.set_ms_btn_system_idx(s_idx as i32);
        }
        MouseButtonSlot::Macro {
            index,
            loop_mode,
            loop_count,
        } => {
            ui.set_ms_btn_category(4);
            ui.set_ms_btn_macro_idx(i32::from(*index).clamp(0, (MOUSE_MACRO_MAX_SLOTS - 1) as i32));
            ui.set_ms_btn_macro_mode(i32::from(*loop_mode).clamp(0, 2));
            ui.set_ms_btn_macro_repeat(i32::from(*loop_count).clamp(1, 99));
        }
        MouseButtonSlot::Disable | MouseButtonSlot::Raw(_) => {
            ui.set_ms_btn_category(5);
        }
    }

    update_left_click_warning(ui, buttons);
}

pub struct MouseStatusSyncPlan {
    pub sync_config: bool,
    pub sync_dpi_index: Option<i32>,
    pub sync_buttons: bool,
    pub draft_cfg: MouseConfig,
    pub dirty: bool,
    pub buttons: [MouseButtonSlot; MOUSE_BUTTON_COUNT],
}

/// Mescla resposta `MouseStatus` periódica sem sobrescrever rascunhos em edição na GUI.
pub fn merge_mouse_status(
    st: &mut GuiState,
    mouse_connected: bool,
    mouse_status: Option<MouseStatus>,
    profile: MouseProfile,
) -> MouseStatusSyncPlan {
    let prev_cfg = st.profile.mouse.config.clone();
    let prev_buttons = st.profile.mouse.buttons.clone();
    let was_dirty = st.draft_mouse_config != prev_cfg;
    let first_or_forced = !st.mouse_synced_initial || st.mouse_force_resync;

    st.profile.mouse = profile;
    st.profile.mouse.config.normalize();
    st.mouse_connected = mouse_connected;
    st.mouse_status = mouse_status;

    let cfg_changed_externally = st.profile.mouse.config != prev_cfg;
    let dpi_stage_changed_externally = st.profile.mouse.config.dpi_index != prev_cfg.dpi_index;
    let buttons_changed = st.profile.mouse.buttons != prev_buttons;

    let sync_config = first_or_forced || (!was_dirty && cfg_changed_externally);
    let mut sync_dpi_index = None;
    if sync_config {
        st.draft_mouse_config = st.profile.mouse.config.clone();
    } else if dpi_stage_changed_externally {
        // Troca pelo botão físico do mouse deve sempre refletir na UI sem descartar rascunhos de DPI/sensor.
        let count = st
            .draft_mouse_config
            .dpi_count
            .clamp(1, MOUSE_MAX_DPI_STAGES as u8);
        let idx = st.profile.mouse.config.dpi_index.min(count - 1);
        st.draft_mouse_config.dpi_index = idx;
        sync_dpi_index = Some(idx as i32);
    }
    st.mouse_synced_initial = true;
    st.mouse_force_resync = false;

    let dirty = st.draft_mouse_config != st.profile.mouse.config;
    MouseStatusSyncPlan {
        sync_config,
        sync_dpi_index,
        sync_buttons: first_or_forced || buttons_changed,
        draft_cfg: st.draft_mouse_config.clone(),
        dirty,
        buttons: st.profile.mouse.buttons.clone(),
    }
}

pub fn sync_button_labels_only(
    ui: &MainWindow,
    buttons: &[MouseButtonSlot; MOUSE_BUTTON_COUNT],
    macros: &[MacroDefinition],
) {
    let labels: Vec<SharedString> = buttons
        .iter()
        .map(|b| SharedString::from(slot_short_label_with_macros(b, macros)))
        .collect();
    ui.set_ms_button_labels(ModelRc::from(Rc::new(VecModel::from(labels))));
    update_left_click_warning(ui, buttons);
}

pub fn sync_buttons_list_to_ui(
    ui: &MainWindow,
    buttons: &[MouseButtonSlot; MOUSE_BUTTON_COUNT],
    macros: &[MacroDefinition],
) {
    sync_button_labels_only(ui, buttons, macros);
    let current = ui.get_ms_selected_slot().max(0) as usize;
    sync_selected_button_to_ui(ui, buttons, macros, current);
}

fn physical_button_label(btn: MouseButton) -> &'static str {
    match btn {
        MouseButton::Left => "Botão Esquerdo",
        MouseButton::Right => "Botão Direito",
        MouseButton::Middle => "Botão Meio",
        MouseButton::Back => "Lateral Voltar",
        MouseButton::Forward => "Lateral Avançar",
    }
}

pub fn sync_chattering_to_ui(ui: &MainWindow, snap: &SwitchChatteringSnapshot) {
    ui.set_ms_chatter_enabled(snap.enabled);
    ui.set_ms_chatter_threshold_ms(i32::from(snap.threshold_ms));
    ui.set_ms_chatter_total(snap.total_chatter_events as i32);

    let alert_text = if snap.total_chatter_events > 0 {
        let btn = snap
            .last_chatter_button
            .map(physical_button_label)
            .unwrap_or("Botão");
        let ms = snap
            .last_chatter_interval_ms
            .map(|v| format!("{v} ms"))
            .unwrap_or_else(|| "-- ms".into());
        format!(
            "Alerta: {} evento(s) de Switch Chattering detectado(s)! Último: {btn} ({ms})",
            snap.total_chatter_events
        )
    } else if !snap.enabled {
        "Monitor de Switch Chattering pausado pelo usuário.".into()
    } else {
        "Nenhum rebote anormal detectado nos switches físicos.".into()
    };
    ui.set_ms_chatter_alert_text(SharedString::from(alert_text));

    let rows: Vec<MouseChatterRowItem> = snap
        .per_button
        .iter()
        .map(|entry| MouseChatterRowItem {
            button_name: SharedString::from(physical_button_label(entry.button)),
            total_clicks: entry.total_clicks as i32,
            chatter_count: entry.chatter_count as i32,
            last_ms: SharedString::from(
                entry
                    .last_interval_ms
                    .map(|v| format!("{v} ms"))
                    .unwrap_or_else(|| "--".into()),
            ),
            min_ms: SharedString::from(
                entry
                    .min_interval_ms
                    .map(|v| format!("{v} ms"))
                    .unwrap_or_else(|| "--".into()),
            ),
            has_chatter: entry.chatter_count > 0,
        })
        .collect();
    ui.set_ms_chatter_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
}

pub fn sync_config_to_ui(ui: &MainWindow, cfg: &MouseConfig, dirty: bool) {
    let count = cfg.dpi_count.clamp(1, MOUSE_MAX_DPI_STAGES as u8);
    let idx = cfg.dpi_index.min(count - 1);
    ui.set_ms_dpi_count(count as i32);
    ui.set_ms_dpi_index(idx as i32);

    let vals: Vec<i32> = cfg.dpi_stages.iter().map(|d| clamp_dpi(*d) as i32).collect();
    let texts: Vec<SharedString> = cfg
        .dpi_stages
        .iter()
        .map(|d| SharedString::from(clamp_dpi(*d).to_string()))
        .collect();
    ui.set_ms_dpi_values(ModelRc::from(Rc::new(VecModel::from(vals))));
    ui.set_ms_dpi_texts(ModelRc::from(Rc::new(VecModel::from(texts))));

    ui.set_ms_polling_idx(match cfg.polling_rate {
        MousePollingRate::Hz125 => 0,
        MousePollingRate::Hz250 => 1,
        MousePollingRate::Hz500 => 2,
        MousePollingRate::Hz1000 => 3,
    });
    ui.set_ms_lod_idx(match cfg.lod {
        LiftOffDistance::Low1Mm => 0,
        LiftOffDistance::High2Mm => 1,
    });
    ui.set_ms_motion_sync(cfg.sensor.motion_sync);
    ui.set_ms_ripple_control(cfg.sensor.ripple_control);
    ui.set_ms_linear_correction(cfg.sensor.linear_correction);
    ui.set_ms_highspeed(cfg.highspeed_mode);
    ui.set_ms_debounce_ms(
        cfg.debounce_ms
            .clamp(MOUSE_DEBOUNCE_MIN_MS, MOUSE_DEBOUNCE_MAX_MS) as i32,
    );
    ui.set_ms_scroll_idx(match cfg.scroll_direction {
        ScrollDirection::Forward => 0,
        ScrollDirection::Reverse => 1,
    });
    ui.set_ms_light_sleep(cfg.light_sleep as i32);
    ui.set_ms_deep_sleep(cfg.deep_sleep as i32);
    ui.set_ms_light_sleep_label(SharedString::from(format_duration_secs(cfg.light_sleep)));
    ui.set_ms_deep_sleep_label(SharedString::from(format_duration_secs(cfg.deep_sleep)));
    ui.set_ms_dirty(dirty);
}

pub fn sync_telemetry_to_ui(
    ui: &MainWindow,
    mouse_connected: bool,
    status: Option<&MouseStatus>,
) {
    ui.set_ms_connected(mouse_connected);
    if !mouse_connected {
        ui.set_ms_online(false);
        ui.set_ms_charging(false);
        ui.set_ms_battery_percent(0);
        ui.set_ms_conn_label(SharedString::from("OFFLINE"));
        ui.set_ms_fw_label(SharedString::from(
            "VID 0x7768 • UsagePage 0xFFB2 (DFU 0xFFB1 bloqueado)",
        ));
        ui.set_sidebar_mouse_subtitle(SharedString::from("Desconectado"));
        return;
    }

    if let Some(st) = status {
        let mode_short = match (st.connection_mode, st.online) {
            (MouseConnectionMode::UsbWired, _) => "USB",
            (MouseConnectionMode::NearLink24G, true) => "2.4G",
            (MouseConnectionMode::NearLink24G, false) => "Sleep",
        };
        let conn_badge = match (st.connection_mode, st.online) {
            (MouseConnectionMode::UsbWired, _) => "USB WIRED",
            (MouseConnectionMode::NearLink24G, true) => "2.4G NEARLINK",
            (MouseConnectionMode::NearLink24G, false) => "2.4G SLEEP",
        };
        let rx_fw = st.receiver_fw.as_deref().unwrap_or("--");
        let chg = if st.charging { " CHG" } else { "" };

        ui.set_ms_online(st.online);
        ui.set_ms_charging(st.charging);
        ui.set_ms_battery_percent(st.battery_percent as i32);
        ui.set_ms_conn_label(SharedString::from(conn_badge));
        ui.set_ms_fw_label(SharedString::from(format!(
            "FW Mouse: {} • Receptor: {} • UsagePage 0xFFB2",
            st.mouse_fw, rx_fw
        )));
        ui.set_sidebar_mouse_subtitle(SharedString::from(format!(
            "{mode_short} • {}%{chg}",
            st.battery_percent
        )));
    } else {
        ui.set_ms_online(false);
        ui.set_ms_conn_label(SharedString::from("LENDO..."));
        ui.set_sidebar_mouse_subtitle(SharedString::from("Conectado • ..."));
    }
}

pub fn capture_keyboard_slot(
    text: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
) -> Option<(u8, usize)> {
    let (_, hid) = keymap_ui::slint_key_to_hid_entry(text)?;
    let mod_mask = keymap_ui::modifier_mask_from_flags(ctrl, shift, alt, meta);
    let idx = HID_KEY_CATALOG
        .iter()
        .position(|k| k.hid_usage == hid)
        .unwrap_or(0);
    Some((mod_mask, idx))
}

fn mutate_mouse_draft(
    state: &Arc<Mutex<GuiState>>,
    ui_w: &slint::Weak<MainWindow>,
    mutate: impl FnOnce(&mut MouseConfig),
) {
    let (cfg, dirty) = {
        let mut st = state.lock().unwrap();
        mutate(&mut st.draft_mouse_config);
        st.draft_mouse_config.normalize();
        let dirty = st.draft_mouse_config != st.profile.mouse.config;
        (st.draft_mouse_config.clone(), dirty)
    };
    if let Some(ui) = ui_w.upgrade() {
        sync_config_to_ui(&ui, &cfg, dirty);
    }
}

fn refresh_button_warning(state: &Arc<Mutex<GuiState>>, ui: &MainWindow) {
    let st = state.lock().unwrap();
    update_left_click_warning(ui, &st.profile.mouse.buttons);
}

/// Registra todos os callbacks da aba `Mouse AJ179 NL P` no `MainWindow`.
pub fn register_mouse_callbacks(
    ui: &MainWindow,
    state: &Arc<Mutex<GuiState>>,
    tx: &mpsc::UnboundedSender<IpcRequest>,
) {
    ui.set_ms_media_labels(media_labels());

    // Quantidade de estágios DPI ativos (1..6)
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_dpi_count_changed(move |cnt| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                cfg.dpi_count = cnt.clamp(1, MOUSE_MAX_DPI_STAGES as i32) as u8;
            });
        });
    }

    // Seleção do estágio ativo
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_dpi_stage_selected(move |idx| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                cfg.dpi_index = idx.clamp(0, (MOUSE_MAX_DPI_STAGES - 1) as i32) as u8;
            });
        });
    }

    // Ajuste -500 / -50 / +50 / +500
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_dpi_step(move |slot, delta| {
            let s = slot.clamp(0, (MOUSE_MAX_DPI_STAGES - 1) as i32) as usize;
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                let next = (cfg.dpi_stages[s] as i32 + delta).clamp(50, 26_000) as u16;
                cfg.dpi_stages[s] = clamp_dpi(next);
            });
        });
    }

    // Edição de texto DPI (atualiza valor em rascunho sem interromper a digitação)
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_dpi_text_edited(move |slot, text| {
            let s = slot.clamp(0, (MOUSE_MAX_DPI_STAGES - 1) as i32) as usize;
            if let Some(dpi) = parse_dpi_input(&text) {
                let (vals, dirty) = {
                    let mut st = st_c.lock().unwrap();
                    st.draft_mouse_config.dpi_stages[s] = dpi;
                    st.draft_mouse_config.normalize();
                    let vals: Vec<i32> = st
                        .draft_mouse_config
                        .dpi_stages
                        .iter()
                        .map(|d| *d as i32)
                        .collect();
                    let dirty = st.draft_mouse_config != st.profile.mouse.config;
                    (vals, dirty)
                };
                if let Some(ui) = ui_w.upgrade() {
                    ui.set_ms_dpi_values(ModelRc::from(Rc::new(VecModel::from(vals))));
                    ui.set_ms_dirty(dirty);
                }
            }
        });
    }

    // Enter na caixa de texto DPI normaliza exibição
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_dpi_text_committed(move |slot, text| {
            let s = slot.clamp(0, (MOUSE_MAX_DPI_STAGES - 1) as i32) as usize;
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                if let Some(dpi) = parse_dpi_input(&text) {
                    cfg.dpi_stages[s] = dpi;
                }
            });
        });
    }

    // Polling Rate
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_polling_selected(move |idx| {
            let rate = match idx {
                0 => MousePollingRate::Hz125,
                1 => MousePollingRate::Hz250,
                2 => MousePollingRate::Hz500,
                _ => MousePollingRate::Hz1000,
            };
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.polling_rate = rate);
        });
    }

    // LOD
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_lod_selected(move |idx| {
            let lod = if idx == 1 {
                LiftOffDistance::High2Mm
            } else {
                LiftOffDistance::Low1Mm
            };
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.lod = lod);
        });
    }

    // Sensor flags (0=Motion Sync, 1=Ripple Control, 2=Linear Correction)
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_sensor_toggled(move |kind| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| match kind {
                0 => cfg.sensor.motion_sync = !cfg.sensor.motion_sync,
                1 => cfg.sensor.ripple_control = !cfg.sensor.ripple_control,
                _ => cfg.sensor.linear_correction = !cfg.sensor.linear_correction,
            });
        });
    }

    // High-Speed Mode
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_highspeed_toggled(move || {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.highspeed_mode = !cfg.highspeed_mode);
        });
    }

    // Debounce +/- e Presets
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_debounce_step(move |delta| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                cfg.debounce_ms = (cfg.debounce_ms as i32 + delta)
                    .clamp(MOUSE_DEBOUNCE_MIN_MS as i32, MOUSE_DEBOUNCE_MAX_MS as i32)
                    as u8;
            });
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_debounce_set(move |val| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| {
                cfg.debounce_ms =
                    val.clamp(MOUSE_DEBOUNCE_MIN_MS as i32, MOUSE_DEBOUNCE_MAX_MS as i32) as u8;
            });
        });
    }

    // Scroll direction & Sleep timers
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_scroll_selected(move |idx| {
            let dir = if idx == 1 {
                ScrollDirection::Reverse
            } else {
                ScrollDirection::Forward
            };
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.scroll_direction = dir);
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_light_sleep_selected(move |secs| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.light_sleep = secs.max(0) as u16);
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_deep_sleep_selected(move |secs| {
            mutate_mouse_draft(&st_c, &ui_w, |cfg| cfg.deep_sleep = secs.max(0) as u16);
        });
    }

    // Aplicar Config Rascunho / Sincronizar / Pareamento
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_apply_config_clicked(move || {
            let (cfg, dirty) = {
                let mut st = st_c.lock().unwrap();
                st.draft_mouse_config.normalize();
                let dirty = st.draft_mouse_config != st.profile.mouse.config;
                (st.draft_mouse_config.clone(), dirty)
            };
            if let Some(ui) = ui_w.upgrade() {
                sync_config_to_ui(&ui, &cfg, dirty);
            }
            let _ = tx_c.send(IpcRequest::SetMouseConfig { config: cfg });
        });
    }
    {
        let tx_c = tx.clone();
        ui.on_ms_sync_clicked(move || {
            let _ = tx_c.send(IpcRequest::SyncMouseFromDevice);
        });
    }
    {
        let tx_c = tx.clone();
        ui.on_ms_start_pairing_clicked(move || {
            let _ = tx_c.send(IpcRequest::StartMousePairing);
        });
    }

    // Botões (5 slots)
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_slot_selected(move |slot| {
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_selected_button_to_ui(
                    &ui,
                    &st.profile.mouse.buttons,
                    &st.profile.macros,
                    slot.max(0) as usize,
                );
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_category_selected(move |cat| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_category(cat);
                ui.set_ms_btn_capturing_key(false);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_mouse_selected(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_mouse_idx(idx);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_toggle_modifier(move |bit| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_mod_mask(ui.get_ms_btn_mod_mask() ^ bit);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_catalog_key_selected(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_key_idx(idx);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_capture_key(move |text, ctrl, shift, alt, meta| {
            if let Some((mod_mask, idx)) = capture_keyboard_slot(&text, ctrl, shift, alt, meta) {
                if let Some(ui) = ui_w.upgrade() {
                    ui.set_ms_btn_mod_mask(mod_mask as i32);
                    ui.set_ms_btn_key_idx(idx as i32);
                    ui.set_ms_btn_capturing_key(false);
                    refresh_button_warning(&st_c, &ui);
                }
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_media_selected(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_media_idx(idx);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_system_selected(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_system_idx(idx);
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_macro_selected(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_macro_idx(idx.clamp(0, (MOUSE_MACRO_MAX_SLOTS - 1) as i32));
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        ui.on_ms_btn_macro_mode_selected(move |mode| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_ms_btn_macro_mode(mode.clamp(0, 2));
                refresh_button_warning(&st_c, &ui);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        ui.on_ms_btn_macro_repeat_step(move |delta| {
            if let Some(ui) = ui_w.upgrade() {
                let next = (ui.get_ms_btn_macro_repeat() + delta).clamp(1, 99);
                ui.set_ms_btn_macro_repeat(next);
            }
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_apply_button_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let slot = (ui.get_ms_selected_slot().max(0) as usize).min(MOUSE_BUTTON_COUNT - 1);
            let assignment = draft_button_from_ui(&ui);
            {
                let mut st = st_c.lock().unwrap();
                if would_lose_left_click(&st.profile.mouse.buttons, slot, &assignment) {
                    ui.set_ms_btn_left_warning(true);
                    ui.set_ms_feedback_text(SharedString::from(
                        "Trava anti-lockout: mantenha ao menos um Clique Esquerdo.",
                    ));
                    ui.set_ms_feedback_is_error(true);
                    return;
                }
                if let MouseButtonSlot::Macro { index, .. } = assignment {
                    let m_idx = usize::from(index);
                    if m_idx >= st.profile.macros.len() {
                        ui.set_ms_feedback_text(SharedString::from(
                            "Selecione uma macro válida antes de aplicar no mouse.",
                        ));
                        ui.set_ms_feedback_is_error(true);
                        return;
                    }
                    if st.profile.macros[m_idx].steps.is_empty() {
                        ui.set_ms_feedback_text(SharedString::from(
                            "A macro escolhida está vazia. Adicione passos no editor primeiro.",
                        ));
                        ui.set_ms_feedback_is_error(true);
                        return;
                    }
                }
                st.profile.mouse.buttons[slot] = assignment.clone();
                sync_buttons_list_to_ui(&ui, &st.profile.mouse.buttons, &st.profile.macros);
            }
            let _ = tx_c.send(IpcRequest::SetMouseButton {
                slot: slot as u8,
                assignment,
            });
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_reset_buttons_clicked(move || {
            {
                let mut st = st_c.lock().unwrap();
                st.profile.mouse.buttons = MouseButtonSlot::DEFAULT_FIVE;
                if let Some(ui) = ui_w.upgrade() {
                    sync_buttons_list_to_ui(&ui, &st.profile.mouse.buttons, &st.profile.macros);
                }
            }
            let _ = tx_c.send(IpcRequest::ResetMouseButtons);
        });
    }

    // Diagnóstico de Switch Chattering
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_chatter_toggle_clicked(move || {
            let mut cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.mouse.chattering.enabled = !st.profile.mouse.chattering.enabled;
                st.chattering_snap.enabled = st.profile.mouse.chattering.enabled;
                if let Some(ui) = ui_w.upgrade() {
                    sync_chattering_to_ui(&ui, &st.chattering_snap);
                }
                st.profile.mouse.chattering
            };
            cfg.normalize();
            let _ = tx_c.send(IpcRequest::SetChatteringConfig { config: cfg });
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_chatter_threshold_step(move |delta| {
            let cfg = {
                let mut st = st_c.lock().unwrap();
                let next = (i32::from(st.profile.mouse.chattering.threshold_ms) + delta).clamp(
                    i32::from(CHATTERING_THRESHOLD_MIN_MS),
                    i32::from(CHATTERING_THRESHOLD_MAX_MS),
                ) as u16;
                st.profile.mouse.chattering.threshold_ms = next;
                st.profile.mouse.chattering.normalize();
                st.chattering_snap.threshold_ms = st.profile.mouse.chattering.threshold_ms;
                if let Some(ui) = ui_w.upgrade() {
                    sync_chattering_to_ui(&ui, &st.chattering_snap);
                }
                st.profile.mouse.chattering
            };
            let _ = tx_c.send(IpcRequest::SetChatteringConfig { config: cfg });
        });
    }
    {
        let ui_w = ui.as_weak();
        let st_c = Arc::clone(state);
        let tx_c = tx.clone();
        ui.on_ms_chatter_reset_clicked(move || {
            {
                let mut st = st_c.lock().unwrap();
                let cfg = st.profile.mouse.chattering;
                st.chattering_snap = SwitchChatteringSnapshot {
                    enabled: cfg.enabled,
                    threshold_ms: cfg.threshold_ms,
                    ..Default::default()
                };
                if let Some(ui) = ui_w.upgrade() {
                    sync_chattering_to_ui(&ui, &st.chattering_snap);
                }
            }
            let _ = tx_c.send(IpcRequest::ResetChatteringStats);
        });
    }
}

fn parse_dpi_input(raw: &str) -> Option<u16> {
    let val = raw.trim().parse::<u32>().ok()?;
    if val < 50 {
        return None;
    }
    Some(clamp_dpi(val.min(26_000) as u16))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prevents_removing_last_left_click_but_allows_reassigning_if_another_slot_has_left() {
        let mut buttons = MouseButtonSlot::DEFAULT_FIVE;
        assert!(would_lose_left_click(&buttons, 0, &MouseButtonSlot::Disable));
        assert!(!would_lose_left_click(
            &buttons,
            0,
            &MouseButtonSlot::Mouse(MouseButtonAction::Left)
        ));
        assert!(!would_lose_left_click(
            &buttons,
            3,
            &MouseButtonSlot::System(SystemKey::Power)
        ));

        // Se Slot 1 também tem Clique Esquerdo, liberar Slot 0 é permitido
        buttons[1] = MouseButtonSlot::Mouse(MouseButtonAction::Left);
        assert!(!would_lose_left_click(&buttons, 0, &MouseButtonSlot::Disable));
    }

    #[test]
    fn formats_sleep_durations_cleanly() {
        assert_eq!(format_duration_secs(0), "Desligado");
        assert_eq!(format_duration_secs(10), "10s");
        assert_eq!(format_duration_secs(60), "1 min");
        assert_eq!(format_duration_secs(300), "5 min");
        assert_eq!(format_duration_secs(90), "1m 30s");
    }

    #[test]
    fn periodic_poll_does_not_reset_button_or_config_editor_draft() {
        let mut st = GuiState::new(ajazz_protocol::UserProfile::default());
        let initial_profile = st.profile.mouse.clone();
        let first = merge_mouse_status(&mut st, true, None, initial_profile.clone());
        assert!(first.sync_config);
        assert!(first.sync_buttons);

        // Poll seguinte sem alterações externas não deve resetar nem o editor de botões nem os campos de texto de DPI
        let second = merge_mouse_status(&mut st, true, None, initial_profile);
        assert!(!second.sync_config);
        assert!(!second.sync_buttons);

        // Se outro cliente/reset alterar os botões no daemon, aí sim resincroniza o editor
        let mut updated = st.profile.mouse.clone();
        updated.buttons[3] = MouseButtonSlot::System(SystemKey::Sleep);
        let third = merge_mouse_status(&mut st, true, None, updated);
        assert!(third.sync_buttons);

        // Mesmo com rascunho dirty em edição, troca física do botão de DPI deve atualizar dpi_index
        st.draft_mouse_config.dpi_stages[0] = 1600;
        let mut hw_dpi = st.profile.mouse.clone();
        hw_dpi.config.dpi_index = 2;
        let fourth = merge_mouse_status(&mut st, true, None, hw_dpi);
        assert!(!fourth.sync_config);
        assert_eq!(fourth.sync_dpi_index, Some(2));
        assert_eq!(st.draft_mouse_config.dpi_stages[0], 1600);
        assert!(fourth.dirty);
    }

    #[test]
    fn legacy_sniper_dpi_config_deserializes_safely_to_disable() {
        let migrated: MouseButtonSlot =
            serde_json::from_str(r#"{"SniperDpi":{"level_mask":1}}"#).unwrap();
        assert_eq!(migrated, MouseButtonSlot::Disable);
        assert_eq!(migrated.to_wire(), [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn slot_short_label_with_macros_resolves_named_macro() {
        let macros = vec![MacroDefinition {
            id: "m1".into(),
            name: "Ctrl+C".into(),
            steps: vec![],
        }];
        let bound = MouseButtonSlot::Macro {
            index: 0,
            loop_mode: 0,
            loop_count: 1,
        };
        assert_eq!(slot_short_label_with_macros(&bound, &macros), "Macro: Ctrl+C");
        let missing = MouseButtonSlot::Macro {
            index: 2,
            loop_mode: 1,
            loop_count: 0,
        };
        assert_eq!(slot_short_label_with_macros(&missing, &macros), "Macro #3");
    }

    #[test]
    fn parse_dpi_input_clamps_high_values_and_ignores_incomplete_prefixes() {
        assert_eq!(parse_dpi_input("40"), None);
        assert_eq!(parse_dpi_input("1630"), Some(1650));
        assert_eq!(parse_dpi_input("80000"), Some(26000));
    }
}
