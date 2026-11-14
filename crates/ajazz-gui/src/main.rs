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
            let is_online = match send_request(IpcRequest::GetStatus).await {
                Ok(IpcResponse::Status { device_connected, .. }) => device_connected,
                _ => false,
            };
            let _ = ui_weak_status.upgrade_in_event_loop(move |ui| {
                ui.set_daemon_online(is_online);
            });

            // Loop de processamento de comandos IPC com polling de status
            let rx = rx;
            loop {
                tokio::select! {
                    Some(req) = async { rx.recv().ok() } => {
                        let resp = send_request(req).await;
                        let online = resp.is_ok();
                        let ui_w = ui_weak_status.clone();
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(online);
                        });
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {
                        let is_online = match send_request(IpcRequest::GetStatus).await {
                            Ok(IpcResponse::Status { device_connected, .. }) => device_connected,
                            _ => false,
                        };
                        let ui_w = ui_weak_status.clone();
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_daemon_online(is_online);
                        });
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

    // Callback para troca de modo (0 = Estático, 1 = Chuva)
    let tx_mode = tx.clone();
    main_window.on_set_mode(move |mode_idx| {
        let mode = match mode_idx {
            1 => LightingMode::Rain,
            0 => LightingMode::Static,
            _ => return,
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

    main_window.run()?;
    Ok(())
}
