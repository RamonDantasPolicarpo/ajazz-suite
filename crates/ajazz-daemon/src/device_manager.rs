use crate::gif_decoder::decode_gif_to_matrix_frames;
use crate::profile_store::{load_profile, save_profile};
use crate::screen_capture::ScreenCapturer;
use ajazz_protocol::hidapi;
use ajazz_protocol::{
    Ak820Device, KeyAssignment, LightingConfig, LightingMode, MacroDefinition, RgbColor,
    SystemStages, UserProfile, PROFILE_AK820, TOTAL_PHYSICAL_LEDS,
};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
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
    PlayGif(String, u8, oneshot::Sender<Result<(), String>>),
    StopGif(oneshot::Sender<Result<(), String>>),
    RenderFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    CaptureFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    GetStatus(oneshot::Sender<DeviceStatus>),
}

fn cancel_active_streams(
    gif_cancel_tx: &mut Option<watch::Sender<bool>>,
    capture_stop: &mut Option<Arc<AtomicBool>>,
) {
    if let Some(cancel) = gif_cancel_tx.take() {
        let _ = cancel.send(true);
    }
    if let Some(stop) = capture_stop.take() {
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

/// Actor dedicado para controle exclusivo do hardware USB HID e estado do User Profile.
pub async fn run_device_actor(
    cmd_tx: mpsc::Sender<DeviceCommand>,
    mut rx: mpsc::Receiver<DeviceCommand>,
) {
    info!("Iniciando gerenciador de hardware AK820...");

    let mut hid_api = hidapi::HidApi::new().ok();
    let mut device: Option<Ak820Device> = None;
    let mut profile = load_profile();
    let mut gif_cancel_tx: Option<watch::Sender<bool>> = None;
    let mut capture_stop: Option<Arc<AtomicBool>> = None;

    // Tenta conexão inicial
    if let Some(ref api) = hid_api {
        if let Ok(dev) = Ak820Device::open(api) {
            info!("Teclado AK820 conectado com sucesso na inicialização.");
            device = Some(dev);
        }
    }

    if device.is_some() && profile.lighting.mode == LightingMode::Capture {
        capture_stop = Some(start_capture_streamer(cmd_tx.clone()));
    }

    while let Some(cmd) = rx.recv().await {
        match cmd {
            DeviceCommand::SetStaticColor(color, resp) => {
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop);

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Static;
                cfg.color = color;
                cfg.multicolor = false;

                let result = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                let _ = resp.send(result);
            }

            DeviceCommand::SetLighting(mut cfg, resp) => {
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop);
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
                    if res.is_ok() && cfg.mode == LightingMode::Capture {
                        capture_stop = Some(start_capture_streamer(cmd_tx.clone()));
                    }
                    res
                };
                let _ = resp.send(result);
            }

            DeviceCommand::SetCustomBank(bank, colors, resp) => {
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop);

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
                cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop);

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
                            cancel_active_streams(&mut gif_cancel_tx, &mut capture_stop);
                            profile.lighting = cfg;
                            profile.system = sys;
                            profile.normalize();
                            save_profile(&profile);
                            if profile.lighting.mode == LightingMode::Capture {
                                capture_stop = Some(start_capture_streamer(cmd_tx.clone()));
                            }
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

            DeviceCommand::PlayGif(path, fps, resp) => {
                // BLOQUEADO: testes no hardware mostraram que cada escrita 0x0B causa
                // piscada (o MCU reprocessa a iluminação). Sem evidência de que 0x0B
                // opere somente em RAM, o streaming fica desativado (risco de flash).
                const GIF_STREAMING_ENABLED: bool = false;
                if !GIF_STREAMING_ENABLED {
                    let _ = resp.send(Err(
                        "GIF Player desativado: streaming por 0x0B pisca e não há prova de segurança da flash".into(),
                    ));
                    continue;
                }
                // Interrompe GIF anterior
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let dev_res = ensure_connected(&mut device, &mut hid_api);
                match dev_res {
                    Ok(dev) => {
                        // Decodifica antecipadamente os quadros projetados
                        match decode_gif_to_matrix_frames(&path) {
                            Ok(frames) => {
                                let total = frames.len();
                                if let Err(e) = dev.enable_custom_lighting_mode() {
                                    let _ = resp.send(Err(format!("Falha ao habilitar modo customizado: {e}")));
                                    continue;
                                }

                                profile.lighting.mode = LightingMode::Custom;
                                let (cancel_tx, mut cancel_rx) = watch::channel(false);
                                gif_cancel_tx = Some(cancel_tx);

                                let fps = fps.clamp(1, 60);
                                let interval_ms = (1000 / fps as u64).max(16);
                                let tx_clone = cmd_tx.clone();

                                tokio::spawn(async move {
                                    let mut interval =
                                        tokio::time::interval(Duration::from_millis(interval_ms));
                                    // Nunca acumula ticks atrasados (evita rajadas e fila de frames).
                                    interval.set_missed_tick_behavior(
                                        tokio::time::MissedTickBehavior::Skip,
                                    );
                                    let mut frame_idx = 0;

                                    loop {
                                        tokio::select! {
                                            _ = interval.tick() => {
                                                let frame = frames[frame_idx];
                                                match tx_clone.try_send(DeviceCommand::RenderFrame(frame)) {
                                                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                                                    // Fila cheia: descarta o frame para manter comandos responsivos.
                                                    _ => {}
                                                }
                                                frame_idx = (frame_idx + 1) % total;
                                            }
                                            _ = cancel_rx.changed() => {
                                                if *cancel_rx.borrow() {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                });

                                info!("Reprodução de GIF iniciada: '{}' ({} frames @ {} FPS)", path, total, fps);
                                let _ = resp.send(Ok(()));
                            }
                            Err(e) => {
                                let _ = resp.send(Err(e));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = resp.send(Err(e));
                    }
                }
            }

            DeviceCommand::StopGif(resp) => {
                let stopped = gif_cancel_tx.take().map(|c| { let _ = c.send(true); }).is_some();
                if stopped {
                    info!("Reprodução de GIF interrompida.");
                    let mut cfg = profile.lighting;
                    cfg.mode = LightingMode::Static;
                    let _ = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                }
                let _ = resp.send(Ok(()));
            }

            DeviceCommand::RenderFrame(frame) => {
                // Descarta frames atrasados que ficaram na fila após o Stop.
                if gif_cancel_tx.is_none() {
                    continue;
                }
                if let Some(ref mut dev) = device {
                    if let Err(e) = dev.set_custom_matrix(&frame) {
                        warn!("Falha ao renderizar frame USB (desconexão?): {e}");
                        device = None;
                    }
                }
            }

            DeviceCommand::CaptureFrame(frame) => {
                if capture_stop.is_none() || profile.lighting.mode != LightingMode::Capture {
                    continue;
                }
                if let Some(ref mut dev) = device {
                    if let Err(e) = dev.send_live_matrix(&frame, profile.lighting.brightness) {
                        warn!("Falha ao enviar quadro Capture (0x12, desconexão?): {e}");
                        device = None;
                        if let Some(stop) = capture_stop.take() {
                            stop.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }

            DeviceCommand::GetStatus(resp) => {
                let is_connected =
                    device.is_some() || try_connect_silent(&mut device, &mut hid_api);
                if is_connected
                    && profile.lighting.mode == LightingMode::Capture
                    && capture_stop.is_none()
                {
                    let cfg = profile.lighting;
                    let _ = apply_lighting_command(
                        &mut device,
                        &mut hid_api,
                        &mut profile,
                        cfg,
                    );
                    if device.is_some() {
                        capture_stop = Some(start_capture_streamer(cmd_tx.clone()));
                    }
                }
                let is_playing = gif_cancel_tx.is_some();
                let status = DeviceStatus {
                    connected: is_connected,
                    device_name: PROFILE_AK820.name.to_string(),
                    profile: profile.clone(),
                    is_playing_gif: is_playing,
                };
                let _ = resp.send(status);
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
