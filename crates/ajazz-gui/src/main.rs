slint::include_modules!();

mod file_dialog;
mod ipc_client;
mod keymap_ui;
mod mouse_ui;

use ajazz_protocol::mouse::{MouseConfig, MouseStatus};
use ajazz_protocol::{
    AudioStyle, DirectionKind, KeyAssignment, LightingConfig, LightingDirection, LightingMode,
    MacroAction, MacroDefinition, MacroStep, MouseButton, PollingRate, RgbColor, UserProfile,
    AK820_KEY_LAYOUT, HID_KEY_CATALOG, TOTAL_PHYSICAL_LEDS, VISIBLE_KEYS_COUNT,
};
use ipc_client::{send_request, IpcRequest, IpcResponse};
use slint::{Color, ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub struct GuiState {
    pub profile: UserProfile,
    pub synced_initial: bool,
    pub force_resync: bool,
    pub hue: f32,
    pub sat: f32,
    pub val: f32,
    pub selected_macro_idx: i32,
    pub draft_macro_id: String,
    pub draft_macro_name: String,
    pub draft_macro_steps: Vec<MacroStep>,
    pub last_record_instant: Option<Instant>,
    pub gif_path: Option<String>,
    pub gif_fps: u8,
    pub is_playing_gif: bool,
    pub draft_mouse_config: MouseConfig,
    pub mouse_synced_initial: bool,
    pub mouse_force_resync: bool,
    pub mouse_connected: bool,
    pub mouse_status: Option<MouseStatus>,
}

impl GuiState {
    fn new(profile: UserProfile) -> Self {
        let (hue, sat, val) = rgb_to_hsv(profile.lighting.color);
        let draft_mouse_config = profile.mouse.config.clone();
        Self {
            profile,
            synced_initial: false,
            force_resync: false,
            hue,
            sat,
            val,
            selected_macro_idx: -1,
            draft_macro_id: String::new(),
            draft_macro_name: String::new(),
            draft_macro_steps: Vec::new(),
            last_record_instant: None,
            gif_path: None,
            gif_fps: 25,
            is_playing_gif: false,
            draft_mouse_config,
            mouse_synced_initial: false,
            mouse_force_resync: false,
            mouse_connected: false,
            mouse_status: None,
        }
    }
}

fn to_slint_color(c: RgbColor) -> Color {
    Color::from_rgb_u8(c.r, c.g, c.b)
}

fn from_slint_color(c: Color) -> RgbColor {
    RgbColor::new(c.red(), c.green(), c.blue())
}

fn rgb_to_hsv(c: RgbColor) -> (f32, f32, f32) {
    let r = c.r as f32 / 255.0;
    let g = c.g as f32 / 255.0;
    let b = c.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta < 1e-5 {
        0.0
    } else if (max - r).abs() < 1e-5 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-5 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max < 1e-5 { 0.0 } else { delta / max };
    (h, s, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> RgbColor {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h / 60.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    RgbColor::new(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

fn render_hue_strip() -> Image {
    const W: u32 = 180;
    const H: u32 = 1;
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(W, H);
    let slice = buf.make_mut_slice();
    for x in 0..W {
        let rgb = hsv_to_rgb((x as f32 / (W - 1) as f32) * 359.0, 1.0, 1.0);
        slice[x as usize] = Rgb8Pixel::new(rgb.r, rgb.g, rgb.b);
    }
    Image::from_rgb8(buf)
}

fn render_sv_square(hue: f32) -> Image {
    const W: u32 = 110;
    const H: u32 = 66;
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(W, H);
    let slice = buf.make_mut_slice();
    for y in 0..H {
        let v = 1.0 - (y as f32 / (H - 1) as f32);
        for x in 0..W {
            let s = x as f32 / (W - 1) as f32;
            let rgb = hsv_to_rgb(hue, s, v);
            slice[(y * W + x) as usize] = Rgb8Pixel::new(rgb.r, rgb.g, rgb.b);
        }
    }
    Image::from_rgb8(buf)
}

fn parse_hex_color(input: &str) -> Option<RgbColor> {
    let trimmed = input.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
    Some(RgbColor::new(r, g, b))
}

fn direction_labels(kind: DirectionKind) -> (&'static str, &'static str) {
    match kind {
        DirectionKind::Horizontal => ("L → R", "R → L"),
        DirectionKind::Circular => ("ANTI-HOR", "HORÁRIO"),
        DirectionKind::Vertical => ("CIMA → BX", "BX → CIMA"),
        DirectionKind::Radial => ("DENTRO", "FORA"),
        DirectionKind::None => ("L → R", "R → L"),
    }
}

fn compute_preview_colors(profile: &UserProfile) -> Vec<Color> {
    const BANDS: [RgbColor; 7] = [
        RgbColor::new(0xEF, 0x44, 0x44),
        RgbColor::new(0xF9, 0x73, 0x16),
        RgbColor::new(0xEA, 0xB3, 0x08),
        RgbColor::new(0x22, 0xC5, 0x5E),
        RgbColor::new(0x06, 0xB6, 0xD4),
        RgbColor::new(0x3B, 0x82, 0xF6),
        RgbColor::new(0xA8, 0x55, 0xF7),
    ];

    let cfg = profile.lighting;
    let caps = cfg.mode.capabilities();
    let mut out = Vec::with_capacity(VISIBLE_KEYS_COUNT);

    if cfg.brightness == 0 {
        out.resize(VISIBLE_KEYS_COUNT, Color::from_rgb_u8(0, 0, 0));
        return out;
    }

    if caps.is_custom_per_key {
        let bank = &profile.custom_banks[cfg.custom_group.min(2) as usize];
        for key in &AK820_KEY_LAYOUT {
            let rgb = bank.get(key.phys_led).copied().unwrap_or_default();
            out.push(to_slint_color(rgb));
        }
        return out;
    }

    if cfg.multicolor || !caps.supports_color {
        for key in &AK820_KEY_LAYOUT {
            let band = ((key.norm_x * 6.99).floor() as usize).min(6);
            out.push(to_slint_color(BANDS[band]));
        }
        return out;
    }

    out.resize(VISIBLE_KEYS_COUNT, to_slint_color(cfg.color));
    out
}

fn sync_ui_from_state(ui: &MainWindow, st: &GuiState) {
    let cfg = st.profile.lighting;
    let mode_idx = LightingMode::ALL
        .iter()
        .position(|&m| m == cfg.mode)
        .unwrap_or(0) as i32;
    let caps = cfg.mode.capabilities();
    let (dir_p, dir_r) = direction_labels(caps.direction_kind);

    ui.set_active_mode_index(mode_idx);
    ui.set_brightness(cfg.brightness as i32);
    ui.set_speed(cfg.speed as i32);
    ui.set_direction(cfg.direction as i32);
    ui.set_multicolor(cfg.multicolor);
    ui.set_zzcc_mode(cfg.zzcc_mode as i32);
    ui.set_custom_bank(cfg.custom_group as i32);
    ui.set_audio_style(cfg.audio_style as i32);
    ui.set_audio_bg_enabled(cfg.audio_bg_enabled);
    ui.set_audio_bg_color(to_slint_color(cfg.audio_bg_color));
    ui.set_gif_fps(st.gif_fps as i32);
    ui.set_is_playing_gif(st.is_playing_gif);
    if let Some(ref p) = st.gif_path {
        let name = std::path::Path::new(p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(p.as_str());
        ui.set_gif_file_name(SharedString::from(name));
        ui.set_gif_has_file(true);
    } else {
        ui.set_gif_file_name(SharedString::from("Nenhum .gif selecionado"));
        ui.set_gif_has_file(false);
    }

    ui.set_supports_color(caps.supports_color);
    ui.set_supports_multicolor(caps.supports_multicolor);
    ui.set_supports_speed(caps.supports_speed);
    ui.set_supports_direction(caps.direction_kind != DirectionKind::None);
    ui.set_dir_primary_label(SharedString::from(dir_p));
    ui.set_dir_reverse_label(SharedString::from(dir_r));
    ui.set_supports_zzcc(caps.supports_zzcc_mode);
    ui.set_is_custom_mode(caps.is_custom_per_key);
    ui.set_is_audio_mode(cfg.mode == LightingMode::Audio);

    ui.set_current_color(to_slint_color(cfg.color));
    ui.set_hex_text(SharedString::from(format!(
        "#{:02X}{:02X}{:02X}",
        cfg.color.r, cfg.color.g, cfg.color.b
    )));
    ui.set_red_text(SharedString::from(cfg.color.r.to_string()));
    ui.set_green_text(SharedString::from(cfg.color.g.to_string()));
    ui.set_blue_text(SharedString::from(cfg.color.b.to_string()));

    let key_model = Rc::new(VecModel::from(compute_preview_colors(&st.profile)));
    ui.set_key_colors(ModelRc::from(key_model));

    keymap_ui::sync_keymap_macros_system_to_ui(ui, st);
    keymap_ui::sync_macro_draft_to_ui(ui, st);

    let ms_dirty = st.draft_mouse_config != st.profile.mouse.config;
    mouse_ui::sync_telemetry_to_ui(ui, st.mouse_connected, st.mouse_status.as_ref());
    mouse_ui::sync_buttons_list_to_ui(ui, &st.profile.mouse.buttons);
    mouse_ui::sync_config_to_ui(ui, &st.draft_mouse_config, ms_dirty);
}

fn update_color_and_hsv(st: &mut GuiState, color: RgbColor, update_hsv: bool) {
    st.profile.lighting.color = color;
    if st.profile.lighting.mode != LightingMode::Custom
        && st.profile.lighting.mode.capabilities().supports_color
    {
        st.profile.lighting.multicolor = false;
    }
    st.profile.normalize();
    if update_hsv {
        let (h, s, v) = rgb_to_hsv(color);
        st.hue = h;
        st.sat = s;
        st.val = v;
    }
}

fn apply_picked_color(
    state: &Arc<Mutex<GuiState>>,
    ui_w: &slint::Weak<MainWindow>,
    tx: &mpsc::UnboundedSender<IpcRequest>,
    rgb: RgbColor,
) {
    let (is_custom, cfg) = {
        let mut st = state.lock().unwrap();
        update_color_and_hsv(&mut st, rgb, true);
        (
            st.profile.lighting.mode == LightingMode::Custom,
            st.profile.lighting,
        )
    };
    if let Some(ui) = ui_w.upgrade() {
        let st = state.lock().unwrap();
        sync_ui_from_state(&ui, &st);
    }
    if !is_custom {
        dispatch_lighting(tx, cfg);
    }
}

fn commit_lighting_change(
    state: &Arc<Mutex<GuiState>>,
    ui_w: &slint::Weak<MainWindow>,
    tx: &mpsc::UnboundedSender<IpcRequest>,
    mutate: impl FnOnce(&mut GuiState),
) {
    let cfg = {
        let mut st = state.lock().unwrap();
        mutate(&mut st);
        st.profile.normalize();
        st.profile.lighting
    };
    if let Some(ui) = ui_w.upgrade() {
        let st = state.lock().unwrap();
        sync_ui_from_state(&ui, &st);
    }
    dispatch_lighting(tx, cfg);
}

fn dispatch_lighting(tx: &mpsc::UnboundedSender<IpcRequest>, mut cfg: LightingConfig) {
    cfg.normalize();
    let _ = tx.send(IpcRequest::SetLighting { config: cfg });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = MainWindow::new()?;
    let state = Arc::new(Mutex::new(GuiState::new(UserProfile::default())));
    let h0 = state.lock().unwrap().hue;
    let s0 = state.lock().unwrap().sat;
    let v0 = state.lock().unwrap().val;

    main_window.set_hsv_hue_image(render_hue_strip());
    main_window.set_hsv_sv_image(render_sv_square(h0));
    main_window.set_hsv_h(h0);
    main_window.set_hsv_s(s0);
    main_window.set_hsv_v(v0);
    main_window.set_km_key_catalog_labels(keymap_ui::key_catalog_labels());
    main_window.set_km_extra_action_labels(keymap_ui::extra_action_labels());
    let mode_labels_vec: Vec<SharedString> = LightingMode::ALL
        .iter()
        .map(|m| SharedString::from(m.label()))
        .collect();
    main_window.set_mode_labels(ModelRc::from(Rc::new(VecModel::from(mode_labels_vec))));
    {
        let st = state.lock().unwrap();
        sync_ui_from_state(&main_window, &st);
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<IpcRequest>();

    let ui_weak_status = main_window.as_weak();
    let state_ipc = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Falha ao criar runtime Tokio");

        rt.block_on(async move {
            let apply_status = |resp: Result<IpcResponse, String>, state_ref: &Arc<Mutex<GuiState>>, ui_w: &slint::Weak<MainWindow>, is_mouse_cmd: bool| -> bool {
                match resp {
                    Ok(IpcResponse::Status { device_connected, profile, is_playing_gif, .. }) => {
                        let mut should_sync_ui = false;
                        {
                            let mut st = state_ref.lock().unwrap();
                            if st.is_playing_gif != is_playing_gif {
                                st.is_playing_gif = is_playing_gif;
                            }
                            if !st.synced_initial || st.force_resync {
                                st.profile = profile;
                                st.profile.normalize();
                                let (h, s, v) = rgb_to_hsv(st.profile.lighting.color);
                                st.hue = h;
                                st.sat = s;
                                st.val = v;
                                if !st.profile.macros.is_empty() && st.selected_macro_idx < 0 {
                                    st.selected_macro_idx = 0;
                                    st.draft_macro_id = st.profile.macros[0].id.clone();
                                    st.draft_macro_name = st.profile.macros[0].name.clone();
                                    st.draft_macro_steps = st.profile.macros[0].steps.clone();
                                }
                                st.synced_initial = true;
                                st.force_resync = false;
                                should_sync_ui = true;
                            }
                        }
                        let state_ui = Arc::clone(state_ref);
                        let ui_clone = ui_w.clone();
                        let _ = ui_clone.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(true);
                            ui.set_device_connected(device_connected);
                            ui.set_is_playing_gif(is_playing_gif);
                            if should_sync_ui {
                                let st = state_ui.lock().unwrap();
                                sync_ui_from_state(&ui, &st);
                            }
                        });
                        false
                    }
                    Ok(IpcResponse::MouseStatus { mouse_connected, mouse_status, profile }) => {
                        let (draft_cfg, dirty, buttons) = {
                            let mut st = state_ref.lock().unwrap();
                            let was_dirty = st.draft_mouse_config != st.profile.mouse.config;
                            st.profile.mouse = profile;
                            st.profile.mouse.config.normalize();
                            st.mouse_connected = mouse_connected;
                            st.mouse_status = mouse_status.clone();
                            if !st.mouse_synced_initial || st.mouse_force_resync || !was_dirty {
                                st.draft_mouse_config = st.profile.mouse.config.clone();
                                st.mouse_synced_initial = true;
                                st.mouse_force_resync = false;
                            }
                            let dirty = st.draft_mouse_config != st.profile.mouse.config;
                            (
                                st.draft_mouse_config.clone(),
                                dirty,
                                st.profile.mouse.buttons.clone(),
                            )
                        };
                        let ui_clone = ui_w.clone();
                        let _ = ui_clone.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(true);
                            mouse_ui::sync_telemetry_to_ui(&ui, mouse_connected, mouse_status.as_ref());
                            mouse_ui::sync_buttons_list_to_ui(&ui, &buttons);
                            mouse_ui::sync_config_to_ui(&ui, &draft_cfg, dirty);
                        });
                        false
                    }
                    Ok(IpcResponse::Ok { message }) => {
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(true);
                            if is_mouse_cmd {
                                ui.set_ms_feedback_text(SharedString::from(message));
                                ui.set_ms_feedback_is_error(false);
                            } else {
                                ui.set_feedback_text(SharedString::from(message));
                                ui.set_feedback_is_error(false);
                            }
                        });
                        false
                    }
                    Ok(IpcResponse::Error { error }) => {
                        if !is_mouse_cmd {
                            let mut st = state_ref.lock().unwrap();
                            st.force_resync = true;
                        }
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(true);
                            if is_mouse_cmd {
                                ui.set_ms_feedback_text(SharedString::from(error));
                                ui.set_ms_feedback_is_error(true);
                            } else {
                                ui.set_feedback_text(SharedString::from(error));
                                ui.set_feedback_is_error(true);
                            }
                        });
                        true
                    }
                    Ok(IpcResponse::Pong) => {
                        let _ = ui_w.upgrade_in_event_loop(|ui| ui.set_daemon_online(true));
                        false
                    }
                    Err(_) => {
                        let _ = ui_w.upgrade_in_event_loop(|ui| {
                            ui.set_daemon_online(false);
                            ui.set_device_connected(false);
                            mouse_ui::sync_telemetry_to_ui(&ui, false, None);
                        });
                        false
                    }
                }
            };

            apply_status(send_request(IpcRequest::GetStatus).await, &state_ipc, &ui_weak_status, false);
            apply_status(send_request(IpcRequest::GetMouseStatus).await, &state_ipc, &ui_weak_status, true);

            loop {
                tokio::select! {
                    Some(req) = rx.recv() => {
                        let is_mouse_cmd = matches!(
                            req,
                            IpcRequest::SetMouseConfig { .. }
                                | IpcRequest::SetMouseButton { .. }
                                | IpcRequest::ResetMouseButtons
                                | IpcRequest::SyncMouseFromDevice
                                | IpcRequest::StartMousePairing
                                | IpcRequest::GetMouseStatus
                        );
                        let was_sync = matches!(req, IpcRequest::SyncFromDevice);
                        let was_mouse_sync = matches!(
                            req,
                            IpcRequest::SyncMouseFromDevice | IpcRequest::SetMouseConfig { .. }
                        );
                        let is_gif_cmd = matches!(req, IpcRequest::PlayGif { .. } | IpcRequest::StopGif);
                        let resp = send_request(req).await;
                        let was_ok = matches!(&resp, Ok(IpcResponse::Ok { .. }));
                        let need_refresh = apply_status(resp, &state_ipc, &ui_weak_status, is_mouse_cmd);
                        if is_mouse_cmd {
                            if was_mouse_sync && was_ok {
                                state_ipc.lock().unwrap().mouse_force_resync = true;
                            }
                            apply_status(send_request(IpcRequest::GetMouseStatus).await, &state_ipc, &ui_weak_status, true);
                        } else if was_sync || is_gif_cmd || need_refresh {
                            if was_sync {
                                state_ipc.lock().unwrap().force_resync = true;
                            }
                            apply_status(send_request(IpcRequest::GetStatus).await, &state_ipc, &ui_weak_status, false);
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {
                        apply_status(send_request(IpcRequest::GetStatus).await, &state_ipc, &ui_weak_status, false);
                        apply_status(send_request(IpcRequest::GetMouseStatus).await, &state_ipc, &ui_weak_status, true);
                    }
                }
            }
        });
    });

    // Seleção de um dos 20 modos
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_mode_selected(move |idx| {
            let Some(mode) = LightingMode::ALL.get(idx as usize).copied() else {
                return;
            };
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.mode = mode;
            });
        });
    }

    // Brilho 0..4
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_brightness_changed(move |val| {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.brightness = val.clamp(0, 4) as u8;
            });
        });
    }

    // Velocidade 0..4
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_speed_changed(move |val| {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.speed = val.clamp(0, 4) as u8;
            });
        });
    }

    // Direção Fx (0 / 1)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_direction_changed(move |val| {
            let dir = if val == 1 {
                LightingDirection::Reverse
            } else {
                LightingDirection::Primary
            };
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.direction = dir;
            });
        });
    }

    // MultiColor (7-Color)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_multicolor_toggled(move |enabled| {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.multicolor = enabled;
            });
        });
    }

    // Surmount ZZCC Preset 0..3
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_zzcc_changed(move |val| {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.zzcc_mode = val.clamp(0, 3) as u8;
            });
        });
    }

    // Seleção de Banco Custom 1..3
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_custom_bank_selected(move |bank| {
            let bank_u8 = bank.clamp(0, 2) as u8;
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.mode = LightingMode::Custom;
                st.profile.lighting.custom_group = bank_u8;
            });
        });
    }

    // Pintar / Apagar tecla individual no Modo Custom (transação fechada única)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_custom_key_clicked(move |key_idx, eraser| {
            let Some(key_pos) = AK820_KEY_LAYOUT.get(key_idx as usize) else {
                return;
            };
            let (bank_u8, colors_vec) = {
                let mut st = st_c.lock().unwrap();
                let bank_u8 = st.profile.lighting.custom_group.min(2);
                let brush = st.profile.lighting.color;
                let bank = &mut st.profile.custom_banks[bank_u8 as usize];
                bank.resize(TOTAL_PHYSICAL_LEDS, RgbColor::default());
                if key_pos.phys_led < bank.len() {
                    bank[key_pos.phys_led] = if eraser || bank[key_pos.phys_led] == brush {
                        RgbColor::default()
                    } else {
                        brush
                    };
                }
                (bank_u8, bank.clone())
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetCustomBank {
                bank: bank_u8,
                colors: colors_vec,
            });
        });
    }

    // Pintar todas as teclas do Banco Custom atual
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_custom_fill_all(move || {
            let (bank_u8, colors_vec) = {
                let mut st = st_c.lock().unwrap();
                let bank_u8 = st.profile.lighting.custom_group.min(2);
                let brush = st.profile.lighting.color;
                let bank = &mut st.profile.custom_banks[bank_u8 as usize];
                bank.resize(TOTAL_PHYSICAL_LEDS, RgbColor::default());
                for key in &AK820_KEY_LAYOUT {
                    if key.phys_led < bank.len() {
                        bank[key.phys_led] = brush;
                    }
                }
                (bank_u8, bank.clone())
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetCustomBank {
                bank: bank_u8,
                colors: colors_vec,
            });
        });
    }

    // Apagar todas as teclas do Banco Custom atual
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_custom_clear_all(move || {
            let (bank_u8, colors_vec) = {
                let mut st = st_c.lock().unwrap();
                let bank_u8 = st.profile.lighting.custom_group.min(2);
                st.profile.custom_banks[bank_u8 as usize] =
                    vec![RgbColor::default(); TOTAL_PHYSICAL_LEDS];
                (bank_u8, st.profile.custom_banks[bank_u8 as usize].clone())
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetCustomBank {
                bank: bank_u8,
                colors: colors_vec,
            });
        });
    }

    // Audio Sync: estilo 0..6, fundo ON/OFF e cor de fundo
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_audio_style_selected(move |idx| {
            let style = AudioStyle::from_u8(idx.clamp(0, 6) as u8);
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.mode = LightingMode::Audio;
                st.profile.lighting.audio_style = style;
            });
        });
    }

    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_audio_bg_toggled(move |enabled| {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.mode = LightingMode::Audio;
                st.profile.lighting.audio_bg_enabled = enabled;
                if enabled && st.profile.lighting.audio_bg_color == RgbColor::default() {
                    st.profile.lighting.audio_bg_color = st.profile.lighting.color;
                }
            });
        });
    }

    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_audio_bg_use_current(move || {
            commit_lighting_change(&st_c, &ui_w, &tx_c, |st| {
                st.profile.lighting.mode = LightingMode::Audio;
                st.profile.lighting.audio_bg_color = st.profile.lighting.color;
                st.profile.lighting.audio_bg_enabled = true;
            });
        });
    }

    // GIF Player: seletor nativo Windows + FPS + Play/Stop em RAM volátil (0x12)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_gif_browse_clicked(move || {
            let ui_clone = ui_w.clone();
            let st_clone = Arc::clone(&st_c);
            std::thread::spawn(move || {
                if let Some(path) = file_dialog::pick_gif_file() {
                    {
                        let mut st = st_clone.lock().unwrap();
                        st.gif_path = Some(path);
                    }
                    let st_ui = Arc::clone(&st_clone);
                    let _ = ui_clone.upgrade_in_event_loop(move |ui| {
                        let st = st_ui.lock().unwrap();
                        sync_ui_from_state(&ui, &st);
                    });
                }
            });
        });
    }

    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_gif_fps_selected(move |fps| {
            let eff_fps = fps.clamp(1, 60) as u8;
            let restart_path = {
                let mut st = st_c.lock().unwrap();
                st.gif_fps = eff_fps;
                if st.is_playing_gif {
                    st.gif_path.clone()
                } else {
                    None
                }
            };
            if let Some(ui) = ui_w.upgrade() {
                ui.set_gif_fps(eff_fps as i32);
            }
            if let Some(path) = restart_path {
                let _ = tx_c.send(IpcRequest::PlayGif {
                    path,
                    fps: Some(eff_fps),
                });
            }
        });
    }

    {
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_gif_play_toggle(move || {
            let action = {
                let st = st_c.lock().unwrap();
                if st.is_playing_gif {
                    Some(IpcRequest::StopGif)
                } else {
                    st.gif_path.as_ref().map(|p| IpcRequest::PlayGif {
                        path: p.clone(),
                        fps: Some(st.gif_fps),
                    })
                }
            };
            if let Some(req) = action {
                let _ = tx_c.send(req);
            }
        });
    }

    // Clique em Swatch Rápido -> seleciona cor (desliga 7-Color se modo suporta cor sólida) e envia se não-Custom
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_swatch_clicked(move |col| {
            apply_picked_color(&st_c, &ui_w, &tx_c, from_slint_color(col));
        });
    }

    // Input #HEX
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_hex_submitted(move |txt| {
            if let Some(rgb) = parse_hex_color(&txt) {
                apply_picked_color(&st_c, &ui_w, &tx_c, rgb);
            }
        });
    }

    // Inputs R / G / B
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_rgb_submitted(move |r_str, g_str, b_str| {
            let (Ok(r), Ok(g), Ok(b)) = (
                r_str.trim().parse::<u8>(),
                g_str.trim().parse::<u8>(),
                b_str.trim().parse::<u8>(),
            ) else {
                return;
            };
            apply_picked_color(&st_c, &ui_w, &tx_c, RgbColor::new(r, g, b));
        });
    }

    // Abrir Popover HSV sob demanda
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_open_hsv_popover(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let (h, s, v) = {
                let st = st_c.lock().unwrap();
                (st.hue, st.sat, st.val)
            };
            ui.set_hsv_h(h);
            ui.set_hsv_s(s);
            ui.set_hsv_v(v);
            ui.set_hsv_sv_image(render_sv_square(h));
            ui.set_hsv_popover_open(true);
        });
    }

    // Arrastar dentro do quadrado S/V do Popover HSV
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_hsv_sv_updated(move |s, v| {
            let h = {
                let mut st = st_c.lock().unwrap();
                st.sat = s.clamp(0.0, 1.0);
                st.val = v.clamp(0.0, 1.0);
                let rgb = hsv_to_rgb(st.hue, st.sat, st.val);
                update_color_and_hsv(&mut st, rgb, false);
                st.hue
            };
            if let Some(ui) = ui_w.upgrade() {
                ui.set_hsv_h(h);
                ui.set_hsv_s(s.clamp(0.0, 1.0));
                ui.set_hsv_v(v.clamp(0.0, 1.0));
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
        });
    }

    // Arrastar na barra Hue do Popover HSV
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_hsv_hue_updated(move |h| {
            {
                let mut st = st_c.lock().unwrap();
                st.hue = h.clamp(0.0, 359.9);
                let rgb = hsv_to_rgb(st.hue, st.sat, st.val);
                update_color_and_hsv(&mut st, rgb, false);
            }
            if let Some(ui) = ui_w.upgrade() {
                ui.set_hsv_h(h.clamp(0.0, 359.9));
                ui.set_hsv_sv_image(render_sv_square(h.clamp(0.0, 359.9)));
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
        });
    }

    // Soltar o mouse no Popover HSV ou clicar fora -> efetiva cor automaticamente se não-Custom
    {
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_hsv_committed(move || {
            let (is_custom, cfg) = {
                let st = st_c.lock().unwrap();
                (
                    st.profile.lighting.mode == LightingMode::Custom,
                    st.profile.lighting,
                )
            };
            if !is_custom {
                dispatch_lighting(&tx_c, cfg);
            }
        });
    }

    // Botão Aplicar (lê HEX ou RGB digitado caso usuário não tenha pressionado Enter e envia ao Daemon)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_apply_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let hex_raw = ui.get_hex_text().to_string();
            let parsed_rgb = ui
                .get_red_text()
                .trim()
                .parse::<u8>()
                .ok()
                .zip(ui.get_green_text().trim().parse::<u8>().ok())
                .zip(ui.get_blue_text().trim().parse::<u8>().ok())
                .map(|((r, g), b)| RgbColor::new(r, g, b));

            let cfg = {
                let mut st = st_c.lock().unwrap();
                let current_hex = format!(
                    "#{:02X}{:02X}{:02X}",
                    st.profile.lighting.color.r,
                    st.profile.lighting.color.g,
                    st.profile.lighting.color.b
                );
                if !hex_raw.eq_ignore_ascii_case(&current_hex) {
                    if let Some(rgb) = parse_hex_color(&hex_raw) {
                        update_color_and_hsv(&mut st, rgb, true);
                    }
                } else if let Some(rgb) = parsed_rgb {
                    if rgb != st.profile.lighting.color {
                        update_color_and_hsv(&mut st, rgb, true);
                    }
                }
                if st.profile.lighting.mode != LightingMode::Custom
                    && st.profile.lighting.mode.capabilities().supports_color
                {
                    st.profile.lighting.multicolor = false;
                }
                sync_ui_from_state(&ui, &st);
                st.profile.lighting
            };
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // Reconectar Daemon
    {
        let tx_c = tx.clone();
        main_window.on_reconnect_daemon(move || {
            let _ = tx_c.send(IpcRequest::GetStatus);
            let _ = tx_c.send(IpcRequest::GetMouseStatus);
        });
    }

    // --- Callbacks de Keymap ---
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_keymap_key_clicked(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_selected_key_to_ui(&ui, &st.profile, idx.max(0) as usize);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_category_selected(move |cat| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_category(cat);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_toggle_modifier(move |bit| {
            if let Some(ui) = ui_w.upgrade() {
                let next = ui.get_km_draft_mod_mask() ^ bit;
                ui.set_km_draft_mod_mask(next);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_select_catalog_key(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_key_idx(idx);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_capture_key_pressed(move |text, ctrl, shift, alt, meta| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            if let Some((entry_mod, entry_usage)) = keymap_ui::slint_key_to_hid_entry(&text) {
                if let Some(idx) = HID_KEY_CATALOG
                    .iter()
                    .position(|e| e.modifiers == entry_mod && e.hid_usage == entry_usage)
                {
                    ui.set_km_draft_key_idx(idx as i32);
                    if entry_usage != 0 {
                        let mask = keymap_ui::modifier_mask_from_flags(ctrl, shift, alt, meta);
                        ui.set_km_draft_mod_mask(mask as i32);
                    } else {
                        ui.set_km_draft_mod_mask(0);
                    }
                }
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_select_mouse_button(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_mouse_idx(idx);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_select_extra_action(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_extra_idx(idx);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_select_macro(move |idx| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_macro_idx(idx);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_select_macro_mode(move |mode| {
            if let Some(ui) = ui_w.upgrade() {
                ui.set_km_draft_macro_mode(mode);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        main_window.on_km_change_macro_repeat(move |delta| {
            if let Some(ui) = ui_w.upgrade() {
                let next = (ui.get_km_draft_macro_repeat() + delta).clamp(1, 255);
                ui.set_km_draft_macro_repeat(next);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_km_apply_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let layout_idx = ui.get_selected_key_idx().clamp(0, (VISIBLE_KEYS_COUNT - 1) as i32) as usize;
            let phys_led = AK820_KEY_LAYOUT[layout_idx].phys_led as u16;
            let assignment_opt = {
                let mut st = st_c.lock().unwrap();
                let Some(assignment) = keymap_ui::build_assignment_from_ui(&ui, &st.profile.macros) else {
                    return;
                };
                st.profile.keymap.insert(phys_led, assignment.clone());
                Some(assignment)
            };
            let st = st_c.lock().unwrap();
            keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            let _ = tx_c.send(IpcRequest::SetKeyAssignment {
                phys_led,
                assignment: assignment_opt,
            });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_km_reset_key_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let layout_idx = ui.get_selected_key_idx().clamp(0, (VISIBLE_KEYS_COUNT - 1) as i32) as usize;
            let phys_led = AK820_KEY_LAYOUT[layout_idx].phys_led as u16;
            {
                let mut st = st_c.lock().unwrap();
                st.profile.keymap.remove(&phys_led);
            }
            let st = st_c.lock().unwrap();
            keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            let _ = tx_c.send(IpcRequest::SetKeyAssignment {
                phys_led,
                assignment: None,
            });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_km_reset_all_clicked(move || {
            {
                let mut st = st_c.lock().unwrap();
                st.profile.keymap.clear();
            }
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::ResetKeymap);
        });
    }

    // --- Callbacks de Macros ---
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_select(move |idx| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let mut st = st_c.lock().unwrap();
            if let Some(mac) = st.profile.macros.get(idx.max(0) as usize).cloned() {
                st.selected_macro_idx = idx;
                st.draft_macro_id = mac.id;
                st.draft_macro_name = mac.name;
                st.draft_macro_steps = mac.steps;
                st.last_record_instant = None;
                keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_new_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let mut st = st_c.lock().unwrap();
            let (id, name) = keymap_ui::next_macro_id(&st.profile.macros);
            st.selected_macro_idx = -1;
            st.draft_macro_id = id;
            st.draft_macro_name = name;
            st.draft_macro_steps.clear();
            st.last_record_instant = None;
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_macro_delete_clicked(move || {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let deleted_id = {
                let mut st = st_c.lock().unwrap();
                let idx = st.selected_macro_idx;
                if idx < 0 || (idx as usize) >= st.profile.macros.len() {
                    return;
                }
                let removed = st.profile.macros.remove(idx as usize);
                st.profile.keymap.retain(|_, a| {
                    !matches!(a, KeyAssignment::Macro { macro_id, .. } if macro_id == &removed.id)
                });
                if let Some(first) = st.profile.macros.first().cloned() {
                    st.selected_macro_idx = 0;
                    st.draft_macro_id = first.id;
                    st.draft_macro_name = first.name;
                    st.draft_macro_steps = first.steps;
                } else {
                    st.selected_macro_idx = -1;
                    st.draft_macro_id.clear();
                    st.draft_macro_name.clear();
                    st.draft_macro_steps.clear();
                }
                removed.id
            };
            let st = st_c.lock().unwrap();
            keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            let _ = tx_c.send(IpcRequest::DeleteMacro { id: deleted_id });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_macro_save_clicked(move |name_raw| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let macro_def = {
                let mut st = st_c.lock().unwrap();
                let clean_name = name_raw.trim();
                let (id, fallback_name) = if st.draft_macro_id.is_empty() {
                    keymap_ui::next_macro_id(&st.profile.macros)
                } else {
                    (st.draft_macro_id.clone(), st.draft_macro_name.clone())
                };
                let final_name = if clean_name.is_empty() {
                    if fallback_name.is_empty() { "Macro".to_string() } else { fallback_name }
                } else {
                    clean_name.to_string()
                };
                let def = MacroDefinition {
                    id: id.clone(),
                    name: final_name.clone(),
                    steps: st.draft_macro_steps.clone(),
                };
                if let Some(pos) = st.profile.macros.iter().position(|m| m.id == id) {
                    st.profile.macros[pos] = def.clone();
                    st.selected_macro_idx = pos as i32;
                } else {
                    st.profile.macros.push(def.clone());
                    st.selected_macro_idx = (st.profile.macros.len() - 1) as i32;
                }
                st.draft_macro_id = id;
                st.draft_macro_name = final_name;
                def
            };
            let st = st_c.lock().unwrap();
            keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            let _ = tx_c.send(IpcRequest::UpsertMacro { macro_def });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_clear_steps_clicked(move || {
            if let Some(ui) = ui_w.upgrade() {
                let mut st = st_c.lock().unwrap();
                st.draft_macro_steps.clear();
                st.last_record_instant = None;
                keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            }
        });
    }
    {
        let st_c = Arc::clone(&state);
        main_window.on_macro_toggle_recording(move |_rec| {
            let mut st = st_c.lock().unwrap();
            st.last_record_instant = None;
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_record_key_event(move |text, down, _c, _s, _a, _m| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let Some(action) = keymap_ui::macro_action_from_slint_key(&text, down) else {
                return;
            };
            let use_fixed = ui.get_macro_use_fixed_delay();
            let fixed_txt = ui.get_macro_fixed_delay_text().to_string();
            let mut st = st_c.lock().unwrap();
            let delay_ms = keymap_ui::compute_record_delay_ms(&mut st, use_fixed, &fixed_txt);
            st.draft_macro_steps.push(MacroStep { delay_ms, action });
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_record_mouse_event(move |btn_idx, down| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let btn = match btn_idx {
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                _ => MouseButton::Left,
            };
            let use_fixed = ui.get_macro_use_fixed_delay();
            let fixed_txt = ui.get_macro_fixed_delay_text().to_string();
            let mut st = st_c.lock().unwrap();
            let delay_ms = keymap_ui::compute_record_delay_ms(&mut st, use_fixed, &fixed_txt);
            st.draft_macro_steps.push(MacroStep {
                delay_ms,
                action: MacroAction::MouseButton { button: btn, down },
            });
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_add_mouse_click(move |btn_idx| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let btn = match btn_idx {
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                _ => MouseButton::Left,
            };
            let mut st = st_c.lock().unwrap();
            st.draft_macro_steps.push(MacroStep {
                delay_ms: 25,
                action: MacroAction::MouseButton { button: btn, down: true },
            });
            st.draft_macro_steps.push(MacroStep {
                delay_ms: 25,
                action: MacroAction::MouseButton { button: btn, down: false },
            });
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_add_mouse_move(move |dx_str, dy_str| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let dx = dx_str.trim().parse::<i16>().unwrap_or(0).clamp(-127, 127) as i8;
            let dy = dy_str.trim().parse::<i16>().unwrap_or(0).clamp(-127, 127) as i8;
            let mut st = st_c.lock().unwrap();
            if dx != 0 {
                st.draft_macro_steps.push(MacroStep {
                    delay_ms: 15,
                    action: MacroAction::MouseMoveX { delta: dx },
                });
            }
            if dy != 0 {
                st.draft_macro_steps.push(MacroStep {
                    delay_ms: 15,
                    action: MacroAction::MouseMoveY { delta: dy },
                });
            }
            keymap_ui::sync_macro_draft_to_ui(&ui, &st);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_adjust_step_delay(move |idx, delta| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let mut st = st_c.lock().unwrap();
            if let Some(step) = st.draft_macro_steps.get_mut(idx.max(0) as usize) {
                let next = (step.delay_ms as i32 + delta).clamp(0, 65535) as u16;
                step.delay_ms = next;
                keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            }
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        main_window.on_macro_remove_step(move |idx| {
            let Some(ui) = ui_w.upgrade() else {
                return;
            };
            let mut st = st_c.lock().unwrap();
            let u = idx.max(0) as usize;
            if u < st.draft_macro_steps.len() {
                st.draft_macro_steps.remove(u);
                keymap_ui::sync_macro_draft_to_ui(&ui, &st);
            }
        });
    }

    // --- Callbacks de SystemStages & Sincronização ---
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_sys_toggle_win_lock(move |val| {
            let sys = {
                let mut st = st_c.lock().unwrap();
                st.profile.system.win_lock = val;
                st.profile.system
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetSystemStages { system: sys });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_sys_toggle_wasd_swap(move |val| {
            let sys = {
                let mut st = st_c.lock().unwrap();
                st.profile.system.wasd_swap = val;
                st.profile.system
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetSystemStages { system: sys });
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_sys_select_polling_rate(move |idx| {
            let rate = PollingRate::ALL
                .get(idx.max(0) as usize)
                .copied()
                .unwrap_or(PollingRate::Hz1000);
            let sys = {
                let mut st = st_c.lock().unwrap();
                st.profile.system.polling_rate = rate;
                st.profile.system
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SetSystemStages { system: sys });
        });
    }
    {
        let tx_c = tx.clone();
        main_window.on_sys_sync_from_device(move || {
            let _ = tx_c.send(IpcRequest::SyncFromDevice);
        });
    }
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_sys_reset_keymap_factory(move || {
            {
                let mut st = st_c.lock().unwrap();
                st.profile.keymap.clear();
            }
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                keymap_ui::sync_keymap_macros_system_to_ui(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::ResetKeymap);
        });
    }

    // --- Callbacks do Mouse AJ179 NL P ---
    mouse_ui::register_mouse_callbacks(&main_window, &state, &tx);

    main_window.run()?;
    Ok(())
}

#[cfg(test)]
mod feedback_tests {
    use super::*;

    #[test]
    fn bug3_hsv_pick_must_clear_multicolor_when_not_custom() {
        let ui = MainWindow::new().expect("MainWindow::new");
        let mut st = GuiState::new(UserProfile::default());
        sync_ui_from_state(&ui, &st);

        st.profile.lighting.mode = LightingMode::Wave;
        st.profile.lighting.multicolor = true;
        st.sat = 0.8;
        st.val = 0.9;
        let rgb = hsv_to_rgb(st.hue, st.sat, st.val);
        update_color_and_hsv(&mut st, rgb, false);
        assert!(
            !st.profile.lighting.multicolor,
            "Bug 3: Picking HSV color kept multicolor=true, so hardware ignores solid RGB color until user toggles multicolor or another mode!"
        );

        // Valida na mesma instância Slint (Winit permite 1 EventLoop por processo) os novos campos Audio/GIF.
        st.profile.lighting.mode = LightingMode::Audio;
        st.profile.lighting.audio_style = AudioStyle::ConcentricRings;
        st.profile.lighting.audio_bg_enabled = true;
        st.profile.lighting.audio_bg_color = RgbColor::new(12, 34, 56);
        st.gif_path = Some(r"C:\gifs\demo.gif".to_string());
        st.gif_fps = 30;
        st.is_playing_gif = true;
        sync_ui_from_state(&ui, &st);

        assert!(ui.get_is_audio_mode());
        assert_eq!(ui.get_audio_style(), AudioStyle::ConcentricRings as i32);
        assert!(ui.get_audio_bg_enabled());
        assert!(ui.get_gif_has_file());
        assert_eq!(ui.get_gif_file_name().as_str(), "demo.gif");
        assert_eq!(ui.get_gif_fps(), 30);
        assert!(ui.get_is_playing_gif());

        // Valida telemetria, DPI e flag dirty do Mouse AJ179 na mesma instância Slint.
        st.mouse_connected = true;
        st.mouse_status = Some(MouseStatus {
            connection_mode: ajazz_protocol::mouse::MouseConnectionMode::NearLink24G,
            online: true,
            charging: false,
            battery_percent: 50,
            mouse_fw: "1.0.14".into(),
            receiver_fw: Some("1.0.14".into()),
            raw_mode: 1,
            report_flag: 0,
            config: st.profile.mouse.config.clone(),
            raw_dpi_words: [400, 800, 1600, 3200, 6400, 12800],
        });
        st.draft_mouse_config.dpi_stages[0] = 600;
        sync_ui_from_state(&ui, &st);
        assert!(ui.get_ms_connected());
        assert_eq!(ui.get_ms_battery_percent(), 50);
        assert_eq!(ui.get_sidebar_mouse_subtitle().as_str(), "2.4G • 50%");
        assert!(ui.get_ms_dirty());
    }

    #[test]
    fn bug3_dispatch_custom_must_preserve_gui_brightness() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cfg = LightingConfig {
            mode: LightingMode::Custom,
            brightness: 2,
            custom_group: 1,
            ..LightingConfig::default()
        };
        dispatch_lighting(&tx, cfg);
        let sent = rx.try_recv().expect("IpcRequest dispatched");
        match sent {
            IpcRequest::SetLighting { config } => {
                assert_eq!(config.brightness, 2);
                assert_eq!(config.custom_group, 1);
                assert!(!config.multicolor);
            }
            other => panic!(
                "Bug 3/2: dispatch_lighting sent {:?} in Custom mode, dropping brightness={}!",
                other, cfg.brightness
            ),
        }
    }

    #[test]
    fn bug3_mode_normalization_clears_multicolor_for_custom_and_capture() {
        let mut custom = LightingConfig {
            mode: LightingMode::Custom,
            multicolor: true,
            ..LightingConfig::default()
        };
        custom.normalize();
        assert!(!custom.multicolor);

        let mut capture = LightingConfig {
            mode: LightingMode::Capture,
            multicolor: true,
            ..LightingConfig::default()
        };
        capture.normalize();
        assert!(!capture.multicolor);
    }

    #[test]
    fn bug3_ipc_source_must_not_block_tokio_select_with_std_mpsc() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).unwrap();
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        assert!(
            !prod_src.contains("std::sync::mpsc") && prod_src.contains("tokio::sync::mpsc"),
            "Bug 3: GUI IPC thread must use async tokio::sync::mpsc rather than blocking std::sync::mpsc in tokio::select!"
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let woke_timer = rt.block_on(async {
            let (_tx, mut rx) = mpsc::unbounded_channel::<IpcRequest>();
            tokio::select! {
                _ = rx.recv() => false,
                _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => true,
            }
        });
        assert!(woke_timer, "tokio::sync::mpsc::UnboundedReceiver::recv must not block timer branches in tokio::select!");
    }
}
