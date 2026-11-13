use serde_json::Value;
use std::env;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

const PIPE_NAME: &str = r"\\.\pipe\ajazz-daemon";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    println!("Conectando ao pipe '{}'...", PIPE_NAME);
    let client = ClientOptions::new().open(PIPE_NAME)?;
    let (reader, mut writer) = tokio::io::split(client);
    let mut lines = BufReader::new(reader).lines();

    let request = match cmd {
        "ping" => r#"{"cmd":"ping"}"#.to_string(),
        "status" => r#"{"cmd":"get_status"}"#.to_string(),
        "red" => r#"{"cmd":"set_static_color","r":255,"g":0,"b":0}"#.to_string(),
        "green" => r#"{"cmd":"set_static_color","r":0,"g":255,"b":0}"#.to_string(),
        "blue" => r#"{"cmd":"set_static_color","r":0,"g":0,"b":255}"#.to_string(),
        "purple" => r#"{"cmd":"set_static_color","r":180,"g":0,"b":255}"#.to_string(),
        "white" => r#"{"cmd":"set_static_color","r":255,"g":255,"b":255}"#.to_string(),
        "play" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("sample_wave.gif");
            format!(r#"{{"cmd":"play_gif","path":"{}","fps":25}}"#, path.replace('\\', "\\\\"))
        }
        "stop" => r#"{"cmd":"stop_gif"}"#.to_string(),
        other => {
            eprintln!("Comando desconhecido: '{other}'. Use ping, status, red, green, blue, purple, white, play [path], stop.");
            return Ok(());
        }
    };

    println!("Enviando: {}", request);
    writer.write_all(format!("{}\n", request).as_bytes()).await?;

    tokio::select! {
        line = lines.next_line() => {
            if let Ok(Some(resp)) = line {
                let parsed: Value = serde_json::from_str(&resp)?;
                println!("Resposta do Daemon:\n{}", serde_json::to_string_pretty(&parsed)?);
            } else {
                println!("Nenhuma resposta recebida do Daemon.");
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(3)) => {
            println!("Timeout aguardando resposta do Daemon.");
        }
    }

    Ok(())
}
