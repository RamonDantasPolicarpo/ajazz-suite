mod device_manager;
mod gif_decoder;
mod ipc;
mod profile_store;

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
            Ok(IpcRequest::PlayGif { path, fps }) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                let _ = cmd_tx.send(DeviceCommand::PlayGif(path, fps.unwrap_or(25), reply_tx)).await;
                match reply_rx.await {
                    Ok(Ok(())) => IpcResponse::Ok {
                        message: "Reprodução de GIF iniciada a 25 FPS".into(),
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
