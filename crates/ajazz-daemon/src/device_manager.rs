use ajazz_protocol::hidapi;
use ajazz_protocol::{Ak820Device, LightingMode, RgbColor, PROFILE_AK820};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub connected: bool,
    pub device_name: String,
    pub current_color: Option<RgbColor>,
    pub current_mode: Option<LightingMode>,
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
    GetStatus(oneshot::Sender<DeviceStatus>),
}

/// Actor dedicado para controle exclusivo do hardware USB HID.
pub async fn run_device_actor(mut rx: mpsc::Receiver<DeviceCommand>) {
    info!("Iniciando gerenciador de hardware AK820...");

    let mut hid_api = hidapi::HidApi::new().ok();
    let mut device: Option<Ak820Device> = None;
    let mut current_color: Option<RgbColor> = None;
    let mut current_mode: Option<LightingMode> = None;

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
            DeviceCommand::GetStatus(resp) => {
                let is_connected = device.is_some() || try_connect_silent(&mut device, &mut hid_api);
                let status = DeviceStatus {
                    connected: is_connected,
                    device_name: PROFILE_AK820.name.to_string(),
                    current_color,
                    current_mode,
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
        // Atualiza ou inicializa hidapi
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

    device.as_mut().ok_or_else(|| "Dispositivo não disponível".to_string())
}
