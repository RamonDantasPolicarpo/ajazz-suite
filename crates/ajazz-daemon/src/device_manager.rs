use crate::gif_decoder::decode_gif_to_matrix_frames;
use ajazz_protocol::hidapi;
use ajazz_protocol::{
    Ak820Device, LightingMode, RgbColor, PROFILE_AK820, TOTAL_PHYSICAL_LEDS,
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub connected: bool,
    pub device_name: String,
    pub current_color: Option<RgbColor>,
    pub current_mode: Option<LightingMode>,
    pub is_playing_gif: bool,
}

pub enum DeviceCommand {
    SetStaticColor(RgbColor, oneshot::Sender<Result<(), String>>),
    SetLighting(
        LightingMode,
        u8,
        u8,
        RgbColor,
        oneshot::Sender<Result<(), String>>,
    ),
    SetKeyMatrix(
        [RgbColor; TOTAL_PHYSICAL_LEDS],
        oneshot::Sender<Result<(), String>>,
    ),
    PlayGif(String, u8, oneshot::Sender<Result<(), String>>),
    StopGif(oneshot::Sender<Result<(), String>>),
    RenderFrame([RgbColor; TOTAL_PHYSICAL_LEDS]),
    GetStatus(oneshot::Sender<DeviceStatus>),
}

/// Actor dedicado para controle exclusivo do hardware USB HID.
pub async fn run_device_actor(
    cmd_tx: mpsc::Sender<DeviceCommand>,
    mut rx: mpsc::Receiver<DeviceCommand>,
) {
    info!("Iniciando gerenciador de hardware AK820...");

    let mut hid_api = hidapi::HidApi::new().ok();
    let mut device: Option<Ak820Device> = None;
    let mut current_color: Option<RgbColor> = None;
    let mut current_mode: Option<LightingMode> = None;
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
                // Para qualquer animação em andamento
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let dev_res = ensure_connected(&mut device, &mut hid_api);
                let result = match dev_res {
                    Ok(dev) => match dev.set_static_color(color) {
                        Ok(()) => {
                            current_color = Some(color);
                            current_mode = Some(LightingMode::Static);
                            info!(
                                "Cor estática aplicada: R={}, G={}, B={}",
                                color.r, color.g, color.b
                            );
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Erro ao enviar comando USB (desconexão a quente?): {e}");
                            device = None;
                            Err(format!("Erro ao comunicar com o teclado: {e}"))
                        }
                    },
                    Err(e) => Err(e),
                };
                let _ = resp.send(result);
            }

            DeviceCommand::SetLighting(mode, brightness, speed, color, resp) => {
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let dev_res = ensure_connected(&mut device, &mut hid_api);
                let result = match dev_res {
                    Ok(dev) => match dev.set_lighting(mode, brightness, speed, color) {
                        Ok(()) => {
                            current_color = Some(color);
                            current_mode = Some(mode);
                            info!(
                                "Modo {:?} aplicado (Brilho={}, Vel={}): R={}, G={}, B={}",
                                mode, brightness, speed, color.r, color.g, color.b
                            );
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Erro ao comunicar com USB: {e}");
                            device = None;
                            Err(format!("Erro de comunicação USB: {e}"))
                        }
                    },
                    Err(e) => Err(e),
                };
                let _ = resp.send(result);
            }

            DeviceCommand::SetKeyMatrix(matrix, resp) => {
                if let Some(cancel) = gif_cancel_tx.take() {
                    let _ = cancel.send(true);
                }

                let dev_res = ensure_connected(&mut device, &mut hid_api);
                let result = match dev_res {
                    Ok(dev) => {
                        let _ = dev.enable_custom_lighting_mode();
                        match dev.set_custom_matrix(&matrix) {
                            Ok(()) => {
                                current_mode = Some(LightingMode::Custom);
                                Ok(())
                            }
                            Err(e) => {
                                device = None;
                                Err(format!("Erro ao enviar matriz USB: {e}"))
                            }
                        }
                    }
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

                                current_mode = Some(LightingMode::Custom);
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
                    // Sai do modo Custom restaurando a última cor estática (uma vez).
                    let color = current_color.unwrap_or(RgbColor::new(255, 255, 255));
                    if let Ok(dev) = ensure_connected(&mut device, &mut hid_api) {
                        match dev.set_lighting(LightingMode::Static, 4, 4, color) {
                            Ok(()) => {
                                current_color = Some(color);
                                current_mode = Some(LightingMode::Static);
                            }
                            Err(e) => {
                                warn!("Erro ao restaurar modo estático: {e}");
                                device = None;
                            }
                        }
                    }
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
                    current_color,
                    current_mode,
                    is_playing_gif: is_playing,
                };
                let _ = resp.send(status);
            }
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
