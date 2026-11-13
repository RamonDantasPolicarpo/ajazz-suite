slint::include_modules!();

mod ipc_client;

use ajazz_protocol::{LightingMode, RgbColor};
use ipc_client::{send_request, IpcRequest, IpcResponse};
use slint::ComponentHandle;
use std::sync::mpsc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = MainWindow::new()?;
    let ui_weak = main_window.as_weak();

    // Canal para despachar comandos para o runtime assíncrono IPC
    let (tx, rx) = mpsc::channel::<IpcRequest>();

    let ui_weak_status = ui_weak.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Falha ao criar runtime Tokio");

        rt.block_on(async move {
            // Checa status inicial do Daemon
            if let Ok(IpcResponse::Status { device_connected, is_playing_gif, .. }) = send_request(IpcRequest::GetStatus).await {
                let _ = ui_weak_status.upgrade_in_event_loop(move |ui| {
                    ui.set_daemon_online(device_connected);
                    ui.set_is_playing_gif(is_playing_gif);
                });
            } else {
                let _ = ui_weak_status.upgrade_in_event_loop(|ui| {
                    ui.set_daemon_online(false);
                });
            }

            // Loop de processamento de comandos IPC com polling de status
            let rx = rx;
            loop {
                // Aguarda comando da UI ou timeout de 1s para atualizar status
                tokio::select! {
                    Some(req) = async { rx.recv().ok() } => {
                        let resp = send_request(req).await;
                        let online = resp.is_ok();
                        let is_playing = match &resp {
                            Ok(IpcResponse::Status { is_playing_gif, .. }) => Some(*is_playing_gif),
                            _ => None,
                        };
                        let ui_w = ui_weak_status.clone();
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(online);
                            if let Some(playing) = is_playing {
                                ui.set_is_playing_gif(playing);
                            }
                        });
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {
                        if let Ok(IpcResponse::Status { device_connected, is_playing_gif, .. }) = send_request(IpcRequest::GetStatus).await {
                            let ui_w = ui_weak_status.clone();
                            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                                ui.set_daemon_online(device_connected);
                                ui.set_is_playing_gif(is_playing_gif);
                            });
                        } else {
                            let ui_w = ui_weak_status.clone();
                            let _ = ui_w.upgrade_in_event_loop(|ui| {
                                ui.set_daemon_online(false);
                            });
                        }
                    }
                }
            }
        });
    });

    // Callback disparado quando o usuário escolhe ou aplica uma cor
    let tx_color = tx.clone();
    main_window.on_apply_color(move |color| {
        let r = color.red();
        let g = color.green();
        let b = color.blue();
        let _ = tx_color.send(IpcRequest::SetStaticColor { r, g, b });
    });

    // Callback para troca de modo (0 = Estático, 1 = Chuva, 2 = GIF)
    let tx_mode = tx.clone();
    main_window.on_set_mode(move |mode_idx| {
        let mode = match mode_idx {
            1 => LightingMode::Rain,
            2 => LightingMode::Custom,
            _ => LightingMode::Static,
        };
        let _ = tx_mode.send(IpcRequest::SetLighting {
            mode,
            brightness: 4,
            speed: 4,
            color: RgbColor::new(255, 0, 0),
        });
    });

    // Callback para reconectar ao Daemon
    let tx_reconnect = tx.clone();
    main_window.on_reconnect_daemon(move || {
        let _ = tx_reconnect.send(IpcRequest::GetStatus);
    });

    // Callback para abrir caixa de seleção de arquivo GIF nativa
    let ui_weak_file = ui_weak.clone();
    main_window.on_select_gif(move || {
        let ui_w = ui_weak_file.clone();
        std::thread::spawn(move || {
            if let Some(file_path) = rfd::FileDialog::new()
                .add_filter("GIF Animations", &["gif"])
                .set_title("Selecionar Animação GIF para Teclado AK820")
                .pick_file()
            {
                let path_str = file_path.to_string_lossy().to_string();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_gif_path(path_str.into());
                });
            }
        });
    });

    // Callback para iniciar reprodução de GIF
    let tx_play = tx.clone();
    let ui_weak_play = ui_weak.clone();
    main_window.on_play_gif(move |path, fps| {
        let _ = tx_play.send(IpcRequest::PlayGif {
            path: path.to_string(),
            fps: Some(fps as u8),
        });
        let _ = ui_weak_play.upgrade_in_event_loop(|ui| {
            ui.set_is_playing_gif(true);
        });
    });

    // Callback para parar reprodução de GIF
    let tx_stop = tx.clone();
    let ui_weak_stop = ui_weak.clone();
    main_window.on_stop_gif(move || {
        let _ = tx_stop.send(IpcRequest::StopGif);
        let _ = ui_weak_stop.upgrade_in_event_loop(|ui| {
            ui.set_is_playing_gif(false);
        });
    });

    main_window.run()?;
    Ok(())
}
