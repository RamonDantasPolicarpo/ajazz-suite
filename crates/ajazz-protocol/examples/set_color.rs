use ajazz_protocol::{Ak820Device, RgbColor};
use std::env;

fn parse_color(arg: &str) -> Option<RgbColor> {
    match arg.to_lowercase().as_str() {
        "red" | "vermelho" => Some(RgbColor::new(255, 0, 0)),
        "green" | "verde" => Some(RgbColor::new(0, 255, 0)),
        "blue" | "azul" => Some(RgbColor::new(0, 0, 255)),
        "white" | "branco" => Some(RgbColor::new(255, 255, 255)),
        "yellow" | "amarelo" => Some(RgbColor::new(255, 255, 0)),
        "cyan" | "ciano" => Some(RgbColor::new(0, 255, 255)),
        "magenta" | "roxo" | "purple" => Some(RgbColor::new(255, 0, 255)),
        "orange" | "laranja" => Some(RgbColor::new(255, 128, 0)),
        hex if hex.len() == 6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(RgbColor::new(r, g, b))
        }
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let color_name = args.get(1).map(|s| s.as_str()).unwrap_or("vermelho");

    let color = match parse_color(color_name) {
        Some(c) => c,
        None => {
            eprintln!(
                "Cor desconhecida: '{}'. Use red, green, blue, yellow, purple, ou hex RRGGBB.",
                color_name
            );
            return Ok(());
        }
    };

    println!("Inicializando API HID...");
    let api = hidapi::HidApi::new()?;

    println!("Conectando ao teclado AK820...");
    let mut dev = Ak820Device::open(&api)?;

    println!(
        "Enviando cor R={}, G={}, B={} (Hex: #{:02X}{:02X}{:02X})...",
        color.r, color.g, color.b, color.r, color.g, color.b
    );
    dev.set_static_color(color)?;

    println!("Sucesso! Iluminação estática aplicada ao teclado AK820.");
    Ok(())
}
