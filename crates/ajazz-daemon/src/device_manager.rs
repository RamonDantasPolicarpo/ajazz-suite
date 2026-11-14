use crate::audio_capture::AudioSyncEngine;
use crate::gif_decoder::decode_gif_to_matrix_frames;
use crate::profile_store::{load_profile, save_profile};
use crate::screen_capture::ScreenCapturer;
use ajazz_protocol::hidapi;
use ajazz_protocol::mouse::{
    Aj179MouseDevice, MouseAsyncEvent, MouseButtonSlot, MouseConfig, MouseProfile, MouseStatus,
    MOUSE_BUTTON_COUNT,
};
use ajazz_protocol::{
    Ak820Device, GifCrop, KeyAssignment, LightingConfig, LightingMode, MacroDefinition, RgbColor,
    SystemStages, UserProfile, PROFILE_AK820, TOTAL_PHYSICAL_LEDS,
};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub connected: bool,
    pub device_name: String,
    pub profile: UserProfile,
    pub is_playing_gif: bool,
}

pub enum DeviceCommand {
    SetStaticColor(RgbColor, oneshot::Sender<Result<(), String>>),
    SetLighting(LightingConfig, oneshot::Sender<Result<(), String>>),
    SetCustomBank(u8, Vec<RgbColor>, oneshot::Sender<Result<(), String>>),
    SelectCustomBank(u8, oneshot::Sender<Result<(), String>>),
    SetSystemStages(SystemStages, oneshot::Sender<Result<(), String>>),
    SetKeyAssignment(
        u16,
        Option<KeyAssignment>,
        oneshot::Sender<Result<(), String>>,
    ),
    ResetKeymap(oneshot::Sender<Result<(), String>>),
    UpsertMacro(MacroDefinition, oneshot::Sender<Result<(), String>>),
    DeleteMacro(String, oneshot::Sender<Result<(), String>>),
    SyncFromDevice(oneshot::Sender<Result<(), String>>),
    PlayGif(String, Option<u8>, GifCrop, oneshot::Sender<Result<(), String>>),
    StopGif(oneshot::Sender<Result<(), String>>),
    RenderFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    CaptureFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    AudioFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    GetStatus(oneshot::Sender<DeviceStatus>),
    GetMouseStatus(oneshot::Sender<(bool, Option<MouseStatus>, MouseProfile)>),
    SetMouseConfig(MouseConfig, oneshot::Sender<Result<(), String>>),
    SetMouseButton(u8, MouseButtonSlot, oneshot::Sender<Result<(), String>>),
    ResetMouseButtons(oneshot::Sender<Result<(), String>>),
    SyncMouseFromDevice(oneshot::Sender<Result<(), String>>),
    StartMousePairing(oneshot::Sender<Result<(), String>>),
}

fn cancel_active_streams(
    gif_cancel_tx: &mut Option<watch::Sender<bool>>,
    capture_stop: &mut Option<Arc<AtomicBool>>,
    audio_stop: &mut Option<Arc<AtomicBool>>,
) {
    if let Some(cancel) = gif_cancel_tx.take() {
        let _ = cancel.send(true);
    }
    if let Some(stop) = capture_stop.take() {
        stop.store(true, Ordering::Relaxed);
    }
    if let Some(stop) = audio_stop.take() {
        stop.store(true, Ordering::Relaxed);
    }
}

fn start_capture_streamer(cmd_tx: mpsc::Sender<DeviceCommand>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        let mut capturer = ScreenCapturer::new();
        while !stop_flag.load(Ordering::Relaxed) {
            if let Some(frame) = capturer.capture_matrix() {
                match cmd_tx.try_send(DeviceCommand::CaptureFrame(frame)) {
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    stop
}

fn start_audio_streamer(
    cmd_tx: mpsc::Sender<DeviceCommand>,
    cfg: LightingConfig,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        let mut engine = AudioSyncEngine::new();
        let mut last_frame: Option<[RgbColor; TOTAL_PHYSICAL_LEDS]> = None;
        while !stop_flag.load(Ordering::Relaxed) {
            let frame = engine.poll_frame(&cfg);
            let changed = last_frame != Some(frame);
            if changed {
                last_frame = Some(frame);
                if let Err(mpsc::error::TrySendError::Closed(_)) =
                    cmd_tx.try_send(DeviceCommand::AudioFrame(frame))
                {
                    break;
                }
            }
            // ~30 FPS sem alocar memória dinâmica nem inundar HID em silêncio absoluto.
            std::thread::sleep(Duration::from_millis(33));
        }
    });
    stop
}

fn sync_live_streamers(
    device_connected: bool,
    profile: &UserProfile,
    cmd_tx: &mpsc::Sender<DeviceCommand>,
    capture_stop: &mut Option<Arc<AtomicBool>>,
    audio_stop: &mut Option<Arc<AtomicBool>>,
    gif_playing: bool,
) {
    if !device_connected || gif_playing {
        return;
    }
    match profile.lighting.mode {
        LightingMode::Capture if capture_stop.is_none() => {
            *capture_stop = Some(start_capture_streamer(cmd_tx.clone()));
        }
        LightingMode::Audio if audio_stop.is_none() => {
            *audio_stop = Some(start_audio_streamer(cmd_tx.clone(), profile.lighting));
        }
        _ => {}
    }
}

fn forward_live_frame(
    device: &mut Option<Ak820Device>,
    profile: &UserProfile,
    expected_mode: LightingMode,
    stop_handle: &mut Option<Arc<AtomicBool>>,
    frame: &[RgbColor; TOTAL_PHYSICAL_LEDS],
    label: &str,
) {
    if stop_handle.is_none() || profile.lighting.mode != expected_mode {
        return;
    }
    if let Some(ref mut dev) = device {
        if let Err(e) = dev.send_live_matrix(frame, profile.lighting.brightness) {
            warn!("Falha ao enviar quadro {label} (0x12, desconexão?): {e}");
            *device = None;
            if let Some(stop) = stop_handle.take() {
                stop.store(true, Ordering::Relaxed);
            }
        }
    }
}

/// Actor dedicado para controle exclusivo do hardware USB HID e estado do User Profile.
pub async fn run_device_actor(
    cmd_tx: mpsc::Sender<DeviceCommand>,
    mut rx: mpsc::Receiver<DeviceCommand>,
) {
    info!("Iniciando gerenciador de hardware AK820...");

    let mut hid_api = hidapi::HidApi::new().ok();
    let mut device: Option<Ak820Device> = None;
    let mut mouse_dev: Option<Aj179MouseDevice> = None;
    let mut mouse_status: Option<MouseStatus> = None;
    let mut last_mouse_poll: Option<Instant> = None;
    let mut profile = load_profile();
    let mut gif_cancel_tx: Option<watch::Sender<bool>> = None;
    let mut pre_gif_lighting: Option<LightingConfig> = None;
    let mut capture_stop: Option<Arc<AtomicBool>> = None;
    let mut audio_stop: Option<Arc<AtomicBool>> = None;

    // Tenta conexão inicial (Teclado AK820 e Mouse AJ179 NL P)
    if let Some(ref api) = hid_api {
        if let Ok(dev) = Ak820Device::open(api) {
            info!("Teclado AK820 conectado com sucesso na inicialização.");
            device = Some(dev);
        }
        if let Ok(mut mdev) = Aj179MouseDevice::open(api) {
            info!(
                "Mouse AJ179 NL P conectado ({:?}) na inicialização.",
                mdev.connection_mode()
            );
            if let Ok(st) = mdev.read_status() {
                mouse_status = Some(st);
                last_mouse_poll = Some(Instant::now());
            }
            mouse_dev = Some(mdev);
        }
    }

    sync_live_streamers(
        device.is_some(),
        &profile,
        &cmd_tx,
        &mut capture_stop,
        &mut audio_stop,
        false,
    );

    while let Some(cmd) = rx.recv().await {
        match cmd {
            DeviceCommand::SetStaticColor(color, resp) => {
                pre_gif_lighting = None;
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop, &mut audio_stop);

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Static;
                cfg.color = color;
                cfg.multicolor = false;

                let result = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                let _ = resp.send(result);
            }

            DeviceCommand::SetLighting(mut cfg, resp) => {
                pre_gif_lighting = None;
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop, &mut audio_stop);
                cfg.normalize();

                let result = if cfg.mode == LightingMode::Custom {
                    apply_custom_command(
                        &mut device,
                        &mut hid_api,
                        &mut profile,
                        cfg,
                        cfg.custom_group,
                        None,
                    )
                } else {
                    let res = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                    sync_live_streamers(
                        res.is_ok() && device.is_some(),
                        &profile,
                        &cmd_tx,
                        &mut capture_stop,
                        &mut audio_stop,
                        false,
                    );
                    res
                };
                let _ = resp.send(result);
            }

            DeviceCommand::SetCustomBank(bank, colors, resp) => {
                pre_gif_lighting = None;
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop, &mut audio_stop);

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Custom;
                cfg.custom_group = bank.min(2);
                cfg.normalize();

                let result = apply_custom_command(
                    &mut device,
                    &mut hid_api,
                    &mut profile,
                    cfg,
                    bank,
                    Some(&colors),
                );
                let _ = resp.send(result);
            }

            DeviceCommand::SelectCustomBank(bank, resp) => {
                pre_gif_lighting = None;
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop, &mut audio_stop);

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Custom;
                cfg.custom_group = bank.min(2);
                cfg.normalize();

                let result = apply_custom_command(
                    &mut device,
                    &mut hid_api,
                    &mut profile,
                    cfg,
                    bank,
                    None,
                );
                let _ = resp.send(result);
            }

            DeviceCommand::SetSystemStages(sys, resp) => {
                let result = apply_system_command(&mut device, &mut hid_api, &mut profile, sys);
                let _ = resp.send(result);
            }

            DeviceCommand::SetKeyAssignment(phys_led, assignment_opt, resp) => {
                let mut next_keymap = profile.keymap.clone();
                if let Some(a) = assignment_opt {
                    next_keymap.insert(phys_led, a);
                } else {
                    next_keymap.remove(&phys_led);
                }
                let next_macros = profile.macros.clone();
                let result = apply_keymap_command(
                    &mut device,
                    &mut hid_api,
                    &mut profile,
                    next_keymap,
                    next_macros,
                );
                let _ = resp.send(result);
            }

            DeviceCommand::ResetKeymap(resp) => {
                let next_macros = profile.macros.clone();
                let result = apply_keymap_command(
                    &mut device,
                    &mut hid_api,
                    &mut profile,
                    BTreeMap::new(),
                    next_macros,
                );
                let _ = resp.send(result);
            }

            DeviceCommand::UpsertMacro(macro_def, resp) => {
                let target_id = macro_def.id.clone();
                let mut next_macros = profile.macros.clone();
                if let Some(existing) = next_macros.iter_mut().find(|m| m.id == target_id) {
                    *existing = macro_def;
                } else {
                    next_macros.push(macro_def);
                }
                let is_referenced = profile.keymap.values().any(|a| {
                    matches!(a, KeyAssignment::Macro { macro_id, .. } if macro_id == &target_id)
                });
                let result = if is_referenced {
                    let next_keymap = profile.keymap.clone();
                    apply_keymap_command(
                        &mut device,
                        &mut hid_api,
                        &mut profile,
                        next_keymap,
                        next_macros,
                    )
                } else {
                    profile.macros = next_macros;
                    save_profile(&profile);
                    Ok(())
                };
                let _ = resp.send(result);
            }

            DeviceCommand::DeleteMacro(id, resp) => {
                let next_macros: Vec<MacroDefinition> = profile
                    .macros
                    .iter()
                    .filter(|m| m.id != id)
                    .cloned()
                    .collect();
                let mut next_keymap = profile.keymap.clone();
                let had_ref = next_keymap.values().any(
                    |a| matches!(a, KeyAssignment::Macro { macro_id, .. } if macro_id == &id),
                );
                if had_ref {
                    next_keymap.retain(|_, a| {
                        !matches!(a, KeyAssignment::Macro { macro_id, .. } if macro_id == &id)
                    });
                    let result = apply_keymap_command(
                        &mut device,
                        &mut hid_api,
                        &mut profile,
                        next_keymap,
                        next_macros,
                    );
                    let _ = resp.send(result);
                } else {
                    profile.macros = next_macros;
                    save_profile(&profile);
                    let _ = resp.send(Ok(()));
                }
            }

            DeviceCommand::SyncFromDevice(resp) => {
                let result = match ensure_connected(&mut device, &mut hid_api) {
                    Ok(dev) => match dev.read_config() {
                        Ok((cfg, sys)) => {
                            pre_gif_lighting = None;
                            cancel_active_streams(
                                &mut gif_cancel_tx,
                                &mut capture_stop,
                                &mut audio_stop,
                            );
                            profile.lighting = cfg;
                            profile.system = sys;
                            profile.normalize();
                            save_profile(&profile);
                            sync_live_streamers(
                                true,
                                &profile,
                                &cmd_tx,
                                &mut capture_stop,
                                &mut audio_stop,
                                false,
                            );
                            info!("Configuração sincronizada a partir do teclado via 0x05.");
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Falha ao ler configuração do teclado via 0x05: {e}");
                            device = None;
                            Err(format!("Falha ao sincronizar com o teclado: {e}"))
                        }
                    },
                    Err(e) => Err(e),
                };
                let _ = resp.send(result);
            }

            DeviceCommand::PlayGif(path, fps, crop, resp) => {
                let saved_lighting = pre_gif_lighting.unwrap_or(profile.lighting);
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop, &mut audio_stop);

                let result = match decode_gif_to_matrix_frames(&path, crop) {
                    Ok(frames) if frames.is_empty() => {
                        Err("O arquivo GIF não contém nenhum frame válido.".to_string())
                    }
                    Ok(frames) => match ensure_connected(&mut device, &mut hid_api) {
                        Ok(dev) => {
                            // Ativa o modo escravo volátil (`0xFE`) UMA ÚNICA VEZ; todos os frames usam RAM (`0x12`).
                            let mut live_cfg = saved_lighting;
                            live_cfg.mode = LightingMode::Capture;
                            live_cfg.normalize();

                            match dev.apply_config(&live_cfg, &profile.system) {
                                Ok(()) => {
                                    pre_gif_lighting = Some(saved_lighting);
                                    let total = frames.len();
                                    let (cancel_tx, mut cancel_rx) = watch::channel(false);
                                    gif_cancel_tx = Some(cancel_tx);

                                    let eff_fps = fps.filter(|&v| v > 0).map(|v| v.clamp(1, 60));
                                    let tx_clone = cmd_tx.clone();

                                    tokio::spawn(async move {
                                        if total == 1 {
                                            let _ = tx_clone
                                                .try_send(DeviceCommand::RenderFrame(frames[0].matrix));
                                            let _ = cancel_rx.changed().await;
                                            return;
                                        }

                                        let mut frame_idx = 0usize;
                                        loop {
                                            let frame = frames[frame_idx];
                                            if let Err(mpsc::error::TrySendError::Closed(_)) = tx_clone
                                                .try_send(DeviceCommand::RenderFrame(frame.matrix))
                                            {
                                                break;
                                            }
                                            let wait_ms = match eff_fps {
                                                Some(f) => (1000 / f as u64).max(16),
                                                None => frame.delay_ms.max(20),
                                            };
                                            frame_idx = (frame_idx + 1) % total;

                                            tokio::select! {
                                                _ = tokio::time::sleep(Duration::from_millis(wait_ms)) => {}
                                                _ = cancel_rx.changed() => {
                                                    if *cancel_rx.borrow() {
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    });

                                    info!(
                                        "Reprodução de GIF iniciada via 0x12: '{}' ({} frames, fps={:?}, crop={:?})",
                                        path, total, eff_fps, crop.normalized()
                                    );
                                    Ok(())
                                }
                                Err(e) => {
                                    warn!("Falha ao ativar modo live 0xFE para GIF: {e}");
                                    device = None;
                                    Err(format!("Falha ao preparar teclado para GIF: {e}"))
                                }
                            }
                        }
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(e),
                };
                let _ = resp.send(result);
            }

            DeviceCommand::StopGif(resp) => {
                let stopped = gif_cancel_tx
                    .take()
                    .map(|c| {
                        let _ = c.send(true);
                    })
                    .is_some();
                let restore_cfg = pre_gif_lighting.take();
                if stopped {
                    info!("Reprodução de GIF interrompida.");
                    let mut cfg = restore_cfg.unwrap_or(profile.lighting);
                    if cfg.mode == LightingMode::Custom {
                        let _ = apply_custom_command(
                            &mut device,
                            &mut hid_api,
                            &mut profile,
                            cfg,
                            cfg.custom_group,
                            None,
                        );
                    } else {
                        cfg.normalize();
                        if apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg)
                            .is_ok()
                        {
                            sync_live_streamers(
                                device.is_some(),
                                &profile,
                                &cmd_tx,
                                &mut capture_stop,
                                &mut audio_stop,
                                false,
                            );
                        }
                    }
                }
                let _ = resp.send(Ok(()));
            }

            DeviceCommand::RenderFrame(frame) => {
                // Descarta frames atrasados após Stop e envia somente via RAM volátil (Opcode 0x12).
                if gif_cancel_tx.is_none() {
                    continue;
                }
                if let Some(ref mut dev) = device {
                    if let Err(e) = dev.send_live_matrix(&frame, profile.lighting.brightness) {
                        warn!("Falha ao enviar quadro GIF (0x12, desconexão?): {e}");
                        device = None;
                        if let Some(cancel) = gif_cancel_tx.take() {
                            let _ = cancel.send(true);
                        }
                    }
                }
            }

            DeviceCommand::CaptureFrame(frame) => {
                forward_live_frame(
                    &mut device,
                    &profile,
                    LightingMode::Capture,
                    &mut capture_stop,
                    &frame,
                    "Capture",
                );
            }

            DeviceCommand::AudioFrame(frame) => {
                forward_live_frame(
                    &mut device,
                    &profile,
                    LightingMode::Audio,
                    &mut audio_stop,
                    &frame,
                    "Audio Sync",
                );
            }

            DeviceCommand::GetStatus(resp) => {
                let is_connected =
                    device.is_some() || try_connect_silent(&mut device, &mut hid_api);
                let is_playing = gif_cancel_tx.is_some();
                let needs_stream_init = is_connected
                    && !is_playing
                    && ((profile.lighting.mode == LightingMode::Capture && capture_stop.is_none())
                        || (profile.lighting.mode == LightingMode::Audio && audio_stop.is_none()));
                if needs_stream_init {
                    let cfg = profile.lighting;
                    let _ = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                    sync_live_streamers(
                        device.is_some(),
                        &profile,
                        &cmd_tx,
                        &mut capture_stop,
                        &mut audio_stop,
                        is_playing,
                    );
                }
                let status = DeviceStatus {
                    connected: is_connected,
                    device_name: PROFILE_AK820.name.to_string(),
                    profile: profile.clone(),
                    is_playing_gif: is_playing,
                };
                let _ = resp.send(status);
            }

            DeviceCommand::GetMouseStatus(resp) => {
                poll_or_connect_mouse(
                    &mut mouse_dev,
                    &mut mouse_status,
                    &mut last_mouse_poll,
                    &mut hid_api,
                    &mut profile,
                );
                let _ = resp.send((
                    mouse_dev.is_some(),
                    mouse_status.clone(),
                    profile.mouse.clone(),
                ));
            }

            DeviceCommand::SetMouseConfig(mut cfg, resp) => {
                cfg.normalize();
                let res = apply_mouse_config_command(
                    &mut mouse_dev,
                    &mut mouse_status,
                    &mut hid_api,
                    &mut profile,
                    cfg,
                );
                let _ = resp.send(res);
            }

            DeviceCommand::SetMouseButton(slot, assignment, resp) => {
                let res = apply_mouse_button_command(
                    &mut mouse_dev,
                    &mut mouse_status,
                    &mut hid_api,
                    &mut profile,
                    slot,
                    assignment,
                );
                let _ = resp.send(res);
            }

            DeviceCommand::ResetMouseButtons(resp) => {
                let res = apply_mouse_reset_buttons_command(
                    &mut mouse_dev,
                    &mut hid_api,
                    &mut profile,
                );
                let _ = resp.send(res);
            }

            DeviceCommand::SyncMouseFromDevice(resp) => {
                let res = sync_mouse_from_device_command(
                    &mut mouse_dev,
                    &mut mouse_status,
                    &mut hid_api,
                    &mut profile,
                );
                let _ = resp.send(res);
            }

            DeviceCommand::StartMousePairing(resp) => {
                let res = start_mouse_pairing_command(&mut mouse_dev, &mut mouse_status, &mut hid_api);
                let _ = resp.send(res);
            }
        }
    }
}

fn apply_lighting_command(
    device: &mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    cfg: LightingConfig,
) -> Result<(), String> {
    let dev = ensure_connected(device, hid_api)?;
    match dev.apply_config(&cfg, &profile.system) {
        Ok(()) => {
            profile.lighting = cfg;
            profile.normalize();
            save_profile(profile);
            info!(
                "Efeito {:?} aplicado (Brilho={}, Vel={}, Dir={:?}, Multi={}, R={},G={},B={})",
                cfg.mode,
                cfg.brightness,
                cfg.speed,
                cfg.direction,
                cfg.multicolor,
                cfg.color.r,
                cfg.color.g,
                cfg.color.b
            );
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao comunicar com USB (desconexão a quente?): {e}");
            *device = None;
            Err(format!("Erro de comunicação USB: {e}"))
        }
    }
}

fn apply_system_command(
    device: &mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    system: SystemStages,
) -> Result<(), String> {
    let dev = ensure_connected(device, hid_api)?;
    match dev.apply_config(&profile.lighting, &system) {
        Ok(()) => {
            profile.system = system;
            profile.normalize();
            save_profile(profile);
            info!(
                "SystemStages aplicados (WinLock={}, WFlag={}, Polling={})",
                system.win_lock,
                system.wasd_swap,
                system.polling_rate.label()
            );
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao aplicar SystemStages no USB: {e}");
            *device = None;
            Err(format!("Erro de comunicação USB: {e}"))
        }
    }
}

fn apply_keymap_command(
    device: &mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    keymap: BTreeMap<u16, KeyAssignment>,
    macros: Vec<MacroDefinition>,
) -> Result<(), String> {
    let dev = ensure_connected(device, hid_api)?;
    match dev.apply_keymap_and_macros(&keymap, &macros) {
        Ok(()) => {
            profile.keymap = keymap;
            profile.macros = macros;
            profile.normalize();
            save_profile(profile);
            info!(
                "Keymap ({} overrides) e Macros ({}) aplicados no teclado.",
                profile.keymap.len(),
                profile.macros.len()
            );
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao gravar Keymap/Macros no USB: {e}");
            *device = None;
            Err(format!("Erro ao gravar Keymap/Macros: {e}"))
        }
    }
}

fn apply_custom_command(
    device: &mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    mut cfg: LightingConfig,
    bank: u8,
    new_colors: Option<&[RgbColor]>,
) -> Result<(), String> {
    let bank_idx = bank.min(2);
    cfg.mode = LightingMode::Custom;
    cfg.custom_group = bank_idx;
    if let Some(colors) = new_colors {
        profile.set_bank_colors(bank_idx, colors);
    }
    let matrix = profile.bank_array(bank_idx);

    let dev = ensure_connected(device, hid_api)?;
    match dev.apply_custom_bank_with_system(&cfg, &profile.system, &matrix) {
        Ok(()) => {
            profile.lighting = cfg;
            profile.normalize();
            save_profile(profile);
            info!("Banco Custom {} aplicado via transação fechada.", bank_idx + 1);
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao gravar banco Custom no USB: {e}");
            *device = None;
            Err(format!("Erro ao enviar matriz USB: {e}"))
        }
    }
}

fn try_connect_silent(
    device: &mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
) -> bool {
    ensure_connected(device, hid_api).is_ok()
}

fn ensure_connected<'a>(
    device: &'a mut Option<Ak820Device>,
    hid_api: &mut Option<hidapi::HidApi>,
) -> Result<&'a mut Ak820Device, String> {
    if device.is_none() {
        if hid_api.is_none() {
            *hid_api = hidapi::HidApi::new().ok();
        }

        if let Some(ref mut api) = hid_api {
            let _ = api.refresh_devices();
            match Ak820Device::open(api) {
                Ok(new_dev) => {
                    info!("Teclado AK820 reconectado com sucesso.");
                    *device = Some(new_dev);
                }
                Err(_) => {
                    return Err(
                        "Teclado AK820 não encontrado. Verifique se o cabo USB está conectado."
                            .to_string(),
                    );
                }
            }
        } else {
            return Err("Falha ao inicializar subsistema HID do sistema.".to_string());
        }
    }

    device
        .as_mut()
        .ok_or_else(|| "Dispositivo não disponível".to_string())
}

fn ensure_mouse_connected<'a>(
    mouse_dev: &'a mut Option<Aj179MouseDevice>,
    hid_api: &mut Option<hidapi::HidApi>,
) -> Result<&'a mut Aj179MouseDevice, String> {
    if mouse_dev.is_none() {
        if hid_api.is_none() {
            *hid_api = hidapi::HidApi::new().ok();
        }
        if let Some(ref mut api) = hid_api {
            let _ = api.refresh_devices();
            match Aj179MouseDevice::open(api) {
                Ok(new_dev) => {
                    info!("Mouse AJ179 NL P conectado ({:?}).", new_dev.connection_mode());
                    *mouse_dev = Some(new_dev);
                }
                Err(_) => {
                    return Err(
                        "Mouse AJ179 NL P não encontrado (UsagePage 0xFFB2). Verifique o cabo ou receptor 2.4G."
                            .to_string(),
                    );
                }
            }
        } else {
            return Err("Falha ao inicializar subsistema HID do sistema.".to_string());
        }
    }
    mouse_dev
        .as_mut()
        .ok_or_else(|| "Mouse não disponível".to_string())
}

const MOUSE_STATUS_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

/// `sub_42f930` só sobrescreve bateria/config quando `rx[12] != 0` (mouse ativo).
fn merge_mouse_status_poll(prev: Option<&MouseStatus>, mut next: MouseStatus) -> MouseStatus {
    if !next.online {
        if let Some(prev) = prev {
            if next.battery_percent == 0 {
                next.battery_percent = prev.battery_percent;
            }
            next.config = prev.config.clone();
        }
    }
    next
}

fn poll_or_connect_mouse(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    mouse_status: &mut Option<MouseStatus>,
    last_mouse_poll: &mut Option<Instant>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
) {
    if mouse_dev.is_none() && ensure_mouse_connected(mouse_dev, hid_api).is_err() {
        *mouse_status = None;
        *last_mouse_poll = None;
        return;
    }

    let mut need_resync = mouse_status.is_none()
        || last_mouse_poll.is_none_or(|t| t.elapsed() >= MOUSE_STATUS_REFRESH_INTERVAL);
    if let Some(ref mut dev) = mouse_dev {
        for _ in 0..8 {
            match dev.poll_notification(0) {
                Ok(Some(MouseAsyncEvent::Reconnect)) => need_resync = true,
                Ok(Some(MouseAsyncEvent::DpiStageChanged { index })) => {
                    if let Some(ref mut st) = mouse_status {
                        st.config.dpi_index = index.min(st.config.dpi_count.saturating_sub(1));
                    }
                    profile.mouse.config.dpi_index =
                        index.min(profile.mouse.config.dpi_count.saturating_sub(1));
                }
                Ok(Some(MouseAsyncEvent::BatteryUpdated {
                    charging,
                    battery_percent,
                })) => {
                    if let Some(ref mut st) = mouse_status {
                        st.charging = charging;
                        st.battery_percent = battery_percent;
                    }
                }
                Ok(Some(MouseAsyncEvent::Special { .. })) => {}
                Ok(None) => break,
                Err(e) => {
                    warn!("Mouse desconectado ao ler notificações: {e}");
                    *mouse_dev = None;
                    *mouse_status = None;
                    *last_mouse_poll = None;
                    return;
                }
            }
        }
    }

    if need_resync {
        if let Some(ref mut dev) = mouse_dev {
            match dev.read_status() {
                Ok(st) => {
                    *mouse_status = Some(merge_mouse_status_poll(mouse_status.as_ref(), st));
                    *last_mouse_poll = Some(Instant::now());
                }
                Err(e) => {
                    warn!("Falha ao consultar status 06 05 do mouse: {e}");
                    *mouse_dev = None;
                    *mouse_status = None;
                    *last_mouse_poll = None;
                }
            }
        }
    }
}

fn sync_mouse_from_device_command(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    mouse_status: &mut Option<MouseStatus>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
) -> Result<(), String> {
    let dev = ensure_mouse_connected(mouse_dev, hid_api)?;
    match dev.read_status() {
        Ok(st) => {
            profile.mouse.config = st.config.clone();
            profile.normalize();
            save_profile(profile);
            *mouse_status = Some(st);
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao sincronizar mouse (desconexão a quente?): {e}");
            *mouse_dev = None;
            *mouse_status = None;
            Err(format!("Erro ao ler estado do mouse: {e}"))
        }
    }
}

fn apply_mouse_config_diff(
    dev: &mut Aj179MouseDevice,
    prev: Option<&MouseConfig>,
    next: &MouseConfig,
) -> Result<(), ajazz_protocol::ProtocolError> {
    let dpi_changed = prev.is_none_or(|p| {
        p.dpi_count != next.dpi_count
            || p.dpi_index != next.dpi_index
            || p.dpi_stages != next.dpi_stages
    });
    if dpi_changed {
        let mut active_sent = false;
        for idx in 0..(next.dpi_count as usize) {
            let changed_slot = prev.is_none_or(|p| {
                p.dpi_stages[idx] != next.dpi_stages[idx] || p.dpi_count != next.dpi_count
            });
            if changed_slot {
                dev.set_dpi_stage(next.dpi_count, idx as u8, next.dpi_stages[idx])?;
                if idx as u8 == next.dpi_index {
                    active_sent = true;
                }
            }
        }
        if !active_sent || prev.is_none_or(|p| p.dpi_index != next.dpi_index) {
            dev.set_dpi_stage(
                next.dpi_count,
                next.dpi_index,
                next.dpi_stages[next.dpi_index as usize],
            )?;
        }
    }

    if prev.is_none_or(|p| p.polling_rate != next.polling_rate) {
        dev.set_polling_rate(next.polling_rate)?;
    }
    if prev.is_none_or(|p| p.scroll_direction != next.scroll_direction) {
        dev.set_scroll_direction(next.scroll_direction)?;
    }
    if prev.is_none_or(|p| p.lod != next.lod) {
        dev.set_lod(next.lod)?;
    }
    if prev.is_none_or(|p| p.sensor != next.sensor) {
        dev.set_sensor_flags(next.sensor)?;
    }
    if prev.is_none_or(|p| p.highspeed_mode != next.highspeed_mode) {
        dev.set_highspeed_mode(next.highspeed_mode)?;
    }
    if prev.is_none_or(|p| p.debounce_ms != next.debounce_ms) {
        dev.set_debounce_ms(next.debounce_ms)?;
    }
    if prev.is_none_or(|p| p.light_sleep != next.light_sleep || p.deep_sleep != next.deep_sleep) {
        dev.set_sleep_timers(next.light_sleep, next.deep_sleep)?;
    }
    Ok(())
}

fn apply_mouse_config_command(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    mouse_status: &mut Option<MouseStatus>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    next: MouseConfig,
) -> Result<(), String> {
    let dev = ensure_mouse_connected(mouse_dev, hid_api)?;
    let prev = mouse_status.as_ref().map(|s| &s.config);
    match apply_mouse_config_diff(dev, prev, &next) {
        Ok(()) => {
            profile.mouse.config = next;
            profile.normalize();
            save_profile(profile);
            if let Ok(st) = dev.read_status() {
                *mouse_status = Some(st);
            }
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao aplicar configuração no mouse (desconexão?): {e}");
            *mouse_dev = None;
            *mouse_status = None;
            Err(format!("Erro de comunicação USB com o mouse: {e}"))
        }
    }
}

fn apply_mouse_button_command(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    mouse_status: &mut Option<MouseStatus>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
    slot: u8,
    assignment: MouseButtonSlot,
) -> Result<(), String> {
    if (slot as usize) >= MOUSE_BUTTON_COUNT {
        return Err(format!("Botão inválido: {slot}"));
    }
    let dev = ensure_mouse_connected(mouse_dev, hid_api)?;
    let mut updated = profile.mouse.buttons.clone();
    updated[slot as usize] = assignment;
    match dev.set_keymap(&updated) {
        Ok(()) => {
            profile.mouse.buttons = updated;
            save_profile(profile);
            let _ = mouse_status;
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao atualizar botão do mouse: {e}");
            *mouse_dev = None;
            *mouse_status = None;
            Err(format!("Erro de comunicação USB com o mouse: {e}"))
        }
    }
}

fn apply_mouse_reset_buttons_command(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    hid_api: &mut Option<hidapi::HidApi>,
    profile: &mut UserProfile,
) -> Result<(), String> {
    let dev = ensure_mouse_connected(mouse_dev, hid_api)?;
    let defaults = MouseButtonSlot::DEFAULT_FIVE;
    match dev.set_keymap(&defaults) {
        Ok(()) => {
            profile.mouse.buttons = defaults;
            save_profile(profile);
            Ok(())
        }
        Err(e) => {
            warn!("Erro ao restaurar botões do mouse: {e}");
            *mouse_dev = None;
            Err(format!("Erro de comunicação USB com o mouse: {e}"))
        }
    }
}

fn start_mouse_pairing_command(
    mouse_dev: &mut Option<Aj179MouseDevice>,
    mouse_status: &mut Option<MouseStatus>,
    hid_api: &mut Option<hidapi::HidApi>,
) -> Result<(), String> {
    let dev = ensure_mouse_connected(mouse_dev, hid_api)?;
    match dev.start_pairing() {
        Ok(()) => Ok(()),
        Err(e) => {
            warn!("Erro ao iniciar pareamento NearLink: {e}");
            *mouse_dev = None;
            *mouse_status = None;
            Err(format!("Erro de comunicação USB com o mouse: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_active_streams_stops_gif_capture_and_audio() {
        let (tx, rx) = watch::channel(false);
        let cap = Arc::new(AtomicBool::new(false));
        let aud = Arc::new(AtomicBool::new(false));
        let mut gif_tx = Some(tx);
        let mut cap_opt = Some(Arc::clone(&cap));
        let mut aud_opt = Some(Arc::clone(&aud));

        cancel_active_streams(&mut gif_tx, &mut cap_opt, &mut aud_opt);

        assert!(gif_tx.is_none());
        assert!(cap_opt.is_none());
        assert!(aud_opt.is_none());
        assert!(*rx.borrow());
        assert!(cap.load(Ordering::Relaxed));
        assert!(aud.load(Ordering::Relaxed));
    }

    #[test]
    fn sync_live_streamers_is_idle_when_disconnected_or_gif_playing() {
        let (tx, _rx) = mpsc::channel(4);
        let mut profile = UserProfile::default();
        profile.lighting.mode = LightingMode::Audio;
        let mut cap = None;
        let mut aud = None;

        sync_live_streamers(false, &profile, &tx, &mut cap, &mut aud, false);
        assert!(aud.is_none());

        sync_live_streamers(true, &profile, &tx, &mut cap, &mut aud, true);
        assert!(aud.is_none());
    }

    #[test]
    fn merge_mouse_status_poll_preserves_battery_and_config_while_sleeping() {
        use ajazz_protocol::mouse::MouseConnectionMode;

        let prev_cfg = MouseConfig {
            debounce_ms: 4,
            ..Default::default()
        };
        let prev = MouseStatus {
            connection_mode: MouseConnectionMode::NearLink24G,
            online: true,
            charging: false,
            battery_percent: 50,
            mouse_fw: "1.0.14".into(),
            receiver_fw: Some("1.0.14".into()),
            raw_mode: 1,
            report_flag: 15,
            config: prev_cfg,
            raw_dpi_words: [7, 15, 32, 63, 47, 63],
        };
        let sleeping = MouseStatus {
            connection_mode: MouseConnectionMode::NearLink24G,
            online: false,
            charging: false,
            battery_percent: 0,
            mouse_fw: "1.0.14".into(),
            receiver_fw: Some("1.0.14".into()),
            raw_mode: 1,
            report_flag: 0,
            config: MouseConfig::default(),
            raw_dpi_words: [0; 6],
        };

        let merged = merge_mouse_status_poll(Some(&prev), sleeping);
        assert!(!merged.online);
        assert_eq!(merged.battery_percent, 50);
        assert_eq!(merged.config.debounce_ms, 4);
    }
}
