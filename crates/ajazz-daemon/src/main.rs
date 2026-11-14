mod audio_capture;
mod chattering;
mod device_manager;
mod gif_decoder;
mod ipc;
mod profile_store;
mod screen_capture;

use device_manager::{run_device_actor, DeviceCommand};
use ipc::{IpcRequest, IpcResponse, IPC_PIPE_NAME};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== AJAZZ Background Daemon ===");
    info!("Canal IPC Named Pipe: {}", IPC_PIPE_NAME);

    // Canal MPSC para isolar o hardware USB em um Actor exclusivo
    let (cmd_tx, cmd_rx) = mpsc::channel::<DeviceCommand>(64);

    // Inicia o Actor de controle do hardware
    let actor_tx = cmd_tx.clone();
    tokio::spawn(async move {
        run_device_actor(actor_tx, cmd_rx).await;
    });

    // Inicia o servidor IPC Named Pipe do Windows
    run_ipc_server(cmd_tx).await?;

    Ok(())
}

async fn run_ipc_server(
    cmd_tx: mpsc::Sender<DeviceCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(IPC_PIPE_NAME)?;

    info!("Aguardando conexões no Named Pipe...");

    loop {
        // Aguarda conexão de cliente (GUI ou scripts)
        if let Err(e) = server.connect().await {
            warn!("Erro ao aguardar conexão no pipe: {e}");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            continue;
        }

        let connected_client = server;

        // Cria a próxima instância do pipe para permitir novos clientes
        server = match ServerOptions::new().create(IPC_PIPE_NAME) {
            Ok(s) => s,
            Err(e) => {
                error!("Falha ao criar próxima instância do Named Pipe: {e}");
                break;
            }
        };

        let tx = cmd_tx.clone();
        tokio::spawn(async move {
            handle_client(connected_client, tx).await;
        });
    }

    Ok(())
}

async fn dispatch_unit_command(
    cmd_tx: &mpsc::Sender<DeviceCommand>,
    make_cmd: impl FnOnce(tokio::sync::oneshot::Sender<Result<(), String>>) -> DeviceCommand,
    ok_msg: &str,
) -> IpcResponse {
    let (reply_tx, reply_rx) = oneshot::channel();
    let _ = cmd_tx.send(make_cmd(reply_tx)).await;
    match reply_rx.await {
        Ok(Ok(())) => IpcResponse::Ok {
            message: ok_msg.to_string(),
        },
        Ok(Err(err)) => IpcResponse::Error { error: err },
        Err(_) => IpcResponse::Error {
            error: "Falha interna no subsistema de hardware".into(),
        },
    }
}

async fn handle_client(pipe: NamedPipeServer, cmd_tx: mpsc::Sender<DeviceCommand>) {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let req: Result<IpcRequest, _> = serde_json::from_str(line);
        let resp = match req {
            Ok(IpcRequest::Ping) => IpcResponse::Pong,
            Ok(IpcRequest::SetStaticColor { r, g, b }) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(DeviceCommand::SetStaticColor(
                        ajazz_protocol::RgbColor::new(r, g, b),
                        reply_tx,
                    ))
                    .await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: format!("Cor estática definida para R={r}, G={g}, B={b}"),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha de comunicação interna com o gerenciador de hardware".into(),
                    },
                }
            }
            Ok(IpcRequest::SetLighting { config }) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(DeviceCommand::SetLighting(config, reply_tx))
                    .await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: "Modo de iluminação aplicado com sucesso".into(),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha interna no subsistema de hardware".into(),
                    },
                }
            }
            Ok(IpcRequest::SetCustomBank { bank, colors }) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(DeviceCommand::SetCustomBank(bank, colors, reply_tx))
                    .await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: format!("Banco Custom {} atualizado", bank.min(2) + 1),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha interna ao atualizar banco Custom".into(),
                    },
                }
            }
            Ok(IpcRequest::SelectCustomBank { bank }) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(DeviceCommand::SelectCustomBank(bank, reply_tx))
                    .await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: format!("Banco Custom {} selecionado", bank.min(2) + 1),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha interna ao selecionar banco Custom".into(),
                    },
                }
            }
            Ok(IpcRequest::SetSystemStages { system }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::SetSystemStages(system, tx),
                    "Configurações de sistema aplicadas com sucesso",
                )
                .await
            }
            Ok(IpcRequest::SetKeyAssignment {
                phys_led,
                assignment,
            }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::SetKeyAssignment(phys_led, assignment, tx),
                    "Mapeamento de tecla atualizado com sucesso",
                )
                .await
            }
            Ok(IpcRequest::ResetKeymap) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::ResetKeymap,
                    "Mapeamento restaurado para padrão de fábrica",
                )
                .await
            }
            Ok(IpcRequest::UpsertMacro { macro_def }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::UpsertMacro(macro_def, tx),
                    "Macro salva com sucesso",
                )
                .await
            }
            Ok(IpcRequest::DeleteMacro { id }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::DeleteMacro(id, tx),
                    "Macro removida com sucesso",
                )
                .await
            }
            Ok(IpcRequest::SyncFromDevice) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::SyncFromDevice,
                    "Perfil sincronizado a partir do teclado",
                )
                .await
            }
            Ok(IpcRequest::PlayGif { path, fps, crop }) => {
                let eff_fps = fps.filter(|&v| v > 0).map(|v| v.clamp(1, 60));
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(DeviceCommand::PlayGif(path, eff_fps, crop, reply_tx))
                    .await;
                let rate_label = match eff_fps {
                    Some(f) => format!("{f} FPS"),
                    None => "AUTO (ritmo do GIF)".to_string(),
                };
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: format!(
                            "Reprodução de GIF iniciada em {rate_label} (RAM volátil 0x12)"
                        ),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha interna ao iniciar GIF".into(),
                    },
                }
            }
            Ok(IpcRequest::StopGif) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx.send(DeviceCommand::StopGif(reply_tx)).await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: "Reprodução de GIF interrompida".into(),
                    },
                    Ok(Err(err)) => IpcResponse::Error { error: err },
                    Err(_) => IpcResponse::Error {
                        error: "Falha interna ao parar GIF".into(),
                    },
                }
            }
            Ok(IpcRequest::GetStatus) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx.send(DeviceCommand::GetStatus(reply_tx)).await;
                match reply_rx.await {
                    Ok(st) => IpcResponse::Status {
                        device_connected: st.connected,
                        device_name: st.device_name,
                        profile: st.profile,
                        is_playing_gif: st.is_playing_gif,
                    },
                    Err(_) => IpcResponse::Error {
                        error: "Falha ao consultar status do dispositivo".into(),
                    },
                }
            }
            Ok(IpcRequest::GetMouseStatus) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx.send(DeviceCommand::GetMouseStatus(reply_tx)).await;
                match reply_rx.await {
                    Ok((mouse_connected, mouse_status, profile, chattering)) => {
                        IpcResponse::MouseStatus {
                            mouse_connected,
                            mouse_status,
                            profile,
                            chattering,
                        }
                    }
                    Err(_) => IpcResponse::Error {
                        error: "Falha ao consultar status do mouse".into(),
                    },
                }
            }
            Ok(IpcRequest::SetMouseConfig { config }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::SetMouseConfig(config, tx),
                    "Configuração do mouse aplicada com sucesso",
                )
                .await
            }
            Ok(IpcRequest::SetMouseButton { slot, assignment }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::SetMouseButton(slot, assignment, tx),
                    "Botão do mouse atualizado com sucesso",
                )
                .await
            }
            Ok(IpcRequest::ResetMouseButtons) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::ResetMouseButtons,
                    "Botões do mouse restaurados para o padrão",
                )
                .await
            }
            Ok(IpcRequest::SyncMouseFromDevice) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::SyncMouseFromDevice,
                    "Perfil sincronizado a partir do mouse",
                )
                .await
            }
            Ok(IpcRequest::StartMousePairing) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::StartMousePairing,
                    "Pareamento NearLink 2.4G iniciado no receptor",
                )
                .await
            }
            Ok(IpcRequest::UploadMouseMacros) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::UploadMouseMacros,
                    "Macros gravadas no mouse AJ179 via 06 31",
                )
                .await
            }
            Ok(IpcRequest::SetChatteringConfig { config }) => {
                dispatch_unit_command(
                    &cmd_tx,
                    |tx| DeviceCommand::SetChatteringConfig(config, tx),
                    "Configuração do detector de Switch Chattering atualizada",
                )
                .await
            }
            Ok(IpcRequest::ResetChatteringStats) => {
                dispatch_unit_command(
                    &cmd_tx,
                    DeviceCommand::ResetChatteringStats,
                    "Contadores de Switch Chattering zerados",
                )
                .await
            }
            Err(e) => IpcResponse::Error {
                error: format!("Payload JSON inválido: {e}"),
            },
        };

        if let Ok(mut json) = serde_json::to_string(&resp) {
            json.push('\n');
            if writer.write_all(json.as_bytes()).await.is_err() {
                break;
            }
        }
    }
}
