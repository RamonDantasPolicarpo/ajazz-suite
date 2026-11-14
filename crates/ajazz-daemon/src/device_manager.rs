use crate::gif_decoder::decode_gif_to_matrix_frames;
use crate::profile_store::{load_profile, save_profile};
use ajazz_protocol::hidapi;
use ajazz_protocol::{
    Ak820Device, LightingConfig, LightingMode, RgbColor, UserProfile, PROFILE_AK820,
    TOTAL_PHYSICAL_LEDS,
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
    PlayGif(String, u8, oneshot::Sender<Result<(), String>>),
    StopGif(oneshot::Sender<Result<(), String>>),
    RenderFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    GetStatus(oneshot::Sender<DeviceStatus>),
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

    // Tenta conexão inicial
    if let Some(ref api) = hid_api {
        if let Ok(dev) = Ak820Device::open(api) {
            info!("Teclado AK820 conectado com sucesso na inicialização.");
            device = Some(dev);
        }
    }

    while let Some(cmd) = rx.recv().await {
        match cmd {
            DeviceCommand::SetStaticColor(color, resp) => {
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Static;
                cfg.color = color;
                cfg.multicolor = false;

                let result = apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg);
                let _ = resp.send(result);
            }

            DeviceCommand::SetLighting(cfg, resp) => {
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

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
                    apply_lighting_command(&mut device, &mut hid_api, &mut profile, cfg)
                };
                let _ = resp.send(result);
            }

            DeviceCommand::SetCustomBank(bank, colors, resp) => {
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Custom;
                cfg.custom_group = bank.min(2);

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
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let mut cfg = profile.lighting;
                cfg.mode = LightingMode::Custom;
                cfg.custom_group = bank.min(2);

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

            DeviceCommand::GetStatus(resp) => {
                let is_connected =
                    device.is_some() || try_connect_silent(&mut device, &mut hid_api);
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
    match dev.apply_lighting(&cfg) {
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
    match dev.apply_custom_bank(&cfg, &matrix) {
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
