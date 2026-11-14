slint::include_modules!();

mod ipc_client;

use ajazz_protocol::{
    DirectionKind, LightingConfig, LightingDirection, LightingMode, RgbColor, UserProfile,
    AK820_KEY_LAYOUT, TOTAL_PHYSICAL_LEDS, VISIBLE_KEYS_COUNT,
};
use ipc_client::{send_request, IpcRequest, IpcResponse};
use slint::{Color, ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

struct GuiState {
    profile: UserProfile,
    synced_initial: bool,
    hue: f32,
    sat: f32,
    val: f32,
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

    ui.set_supports_color(caps.supports_color);
    ui.set_supports_multicolor(caps.supports_multicolor);
    ui.set_supports_speed(caps.supports_speed);
    ui.set_supports_direction(caps.direction_kind != DirectionKind::None);
    ui.set_dir_primary_label(SharedString::from(dir_p));
    ui.set_dir_reverse_label(SharedString::from(dir_r));
    ui.set_supports_zzcc(caps.supports_zzcc_mode);
    ui.set_is_custom_mode(caps.is_custom_per_key);

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
}

fn update_color_and_hsv(st: &mut GuiState, color: RgbColor, update_hsv: bool) {
    st.profile.lighting.color = color;
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
    tx: &mpsc::Sender<IpcRequest>,
    rgb: RgbColor,
) {
    let (is_custom, cfg) = {
        let mut st = state.lock().unwrap();
        update_color_and_hsv(&mut st, rgb, true);
        if st.profile.lighting.mode != LightingMode::Custom {
            st.profile.lighting.multicolor = false;
        }
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

fn dispatch_lighting(tx: &mpsc::Sender<IpcRequest>, cfg: LightingConfig) {
    if cfg.mode == LightingMode::Custom {
        let _ = tx.send(IpcRequest::SelectCustomBank {
            bank: cfg.custom_group,
        });
    } else {
        let _ = tx.send(IpcRequest::SetLighting { config: cfg });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = MainWindow::new()?;
    let initial_profile = UserProfile::default();
    let (h0, s0, v0) = rgb_to_hsv(initial_profile.lighting.color);
    let state = Arc::new(Mutex::new(GuiState {
        profile: initial_profile,
        synced_initial: false,
        hue: h0,
        sat: s0,
        val: v0,
    }));

    main_window.set_hsv_hue_image(render_hue_strip());
    main_window.set_hsv_sv_image(render_sv_square(h0));
    main_window.set_hsv_h(h0);
    main_window.set_hsv_s(s0);
    main_window.set_hsv_v(v0);
    {
        let st = state.lock().unwrap();
        sync_ui_from_state(&main_window, &st);
    }

    let (tx, rx) = mpsc::channel::<IpcRequest>();

    let ui_weak_status = main_window.as_weak();
    let state_ipc = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Falha ao criar runtime Tokio");

        rt.block_on(async move {
            let apply_status = |resp: Result<IpcResponse, String>, state_ref: &Arc<Mutex<GuiState>>, ui_w: &slint::Weak<MainWindow>| {
                match resp {
                    Ok(IpcResponse::Status { device_connected, profile, .. }) => {
                        let mut initial_sync = false;
                        {
                            let mut st = state_ref.lock().unwrap();
                            if !st.synced_initial {
                                st.profile = profile;
                                st.profile.normalize();
                                let (h, s, v) = rgb_to_hsv(st.profile.lighting.color);
                                st.hue = h;
                                st.sat = s;
                                st.val = v;
                                st.synced_initial = true;
                                initial_sync = true;
                            }
                        }
                        let state_ui = Arc::clone(state_ref);
                        let ui_clone = ui_w.clone();
                        let _ = ui_clone.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(true);
                            ui.set_device_connected(device_connected);
                            if initial_sync {
                                let st = state_ui.lock().unwrap();
                                sync_ui_from_state(&ui, &st);
                            }
                        });
                    }
                    Ok(_) => {
                        let _ = ui_w.upgrade_in_event_loop(|ui| ui.set_daemon_online(true));
                    }
                    Err(_) => {
                        let _ = ui_w.upgrade_in_event_loop(|ui| {
                            ui.set_daemon_online(false);
                            ui.set_device_connected(false);
                        });
                    }
                }
            };

            apply_status(send_request(IpcRequest::GetStatus).await, &state_ipc, &ui_weak_status);

            loop {
                tokio::select! {
                    Some(req) = async { rx.recv().ok() } => {
                        let resp = send_request(req).await;
                        apply_status(resp, &state_ipc, &ui_weak_status);
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {
                        let resp = send_request(IpcRequest::GetStatus).await;
                        apply_status(resp, &state_ipc, &ui_weak_status);
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
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.mode = mode;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // Brilho 0..4
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_brightness_changed(move |val| {
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.brightness = val.clamp(0, 4) as u8;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // Velocidade 0..4
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_speed_changed(move |val| {
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.speed = val.clamp(0, 4) as u8;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
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
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.direction = dir;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // MultiColor (7-Color)
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_multicolor_toggled(move |enabled| {
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.multicolor = enabled;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // Surmount ZZCC Preset 0..3
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_zzcc_changed(move |val| {
            let cfg = {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.zzcc_mode = val.clamp(0, 3) as u8;
                st.profile.lighting
            };
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            dispatch_lighting(&tx_c, cfg);
        });
    }

    // Seleção de Banco Custom 1..3
    {
        let ui_w = main_window.as_weak();
        let st_c = Arc::clone(&state);
        let tx_c = tx.clone();
        main_window.on_custom_bank_selected(move |bank| {
            let bank_u8 = bank.clamp(0, 2) as u8;
            {
                let mut st = st_c.lock().unwrap();
                st.profile.lighting.mode = LightingMode::Custom;
                st.profile.lighting.custom_group = bank_u8;
            }
            if let Some(ui) = ui_w.upgrade() {
                let st = st_c.lock().unwrap();
                sync_ui_from_state(&ui, &st);
            }
            let _ = tx_c.send(IpcRequest::SelectCustomBank { bank: bank_u8 });
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
        });
    }

    main_window.run()?;
    Ok(())
}
