use ajazz_protocol::*;
use std::time::{Duration, Instant};

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h as u32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Teste de Animação RGB a 25 FPS no AK820 (Sem Gravação em Flash) ===");
    let api = hidapi::HidApi::new()?;
    let mut dev = Ak820Device::open(&api)?;
    println!("Teclado AK820 conectado!");

    // 1. Iniciar transação (Opcode 0x01)
    let p_begin = build_begin_transaction_packet();
    dev.device_handle().write(&p_begin)?;

    // 2. Mudar modo para Custom Lighting (Mode 20)
    let mut p_mode = [0u8; 64];
    p_mode[0] = REPORT_ID_COMMAND;
    p_mode[3] = OPCODE_SET_LIGHTING;
    p_mode[4] = 0x1D;
    p_mode[9] = 20; // 0x14 = Custom Mode
    p_mode[10] = 4; // Brightness max
    p_mode[11] = 4; // Speed
    let sum = calculate_checksum(&p_mode);
    p_mode[1] = (sum & 0xFF) as u8;
    p_mode[2] = ((sum >> 8) & 0xFF) as u8;
    dev.device_handle().write(&p_mode)?;

    // 3. Loop de animação a 25 FPS (40ms por frame) durante 5 segundos = 125 frames
    let frame_duration = Duration::from_millis(40);
    let total_frames = 125;
    let mut rgb_buffer = [0u8; 384];
    const CHUNK_SIZE: usize = 56;

    println!("Iniciando renderização de 125 frames a 25 FPS...");
    let start_time = Instant::now();

    for frame in 0..total_frames {
        let frame_start = Instant::now();

        // Calcular arco-íris se deslocando horizontalmente
        let shift = (frame as f32) * 5.0;
        for key_idx in 0..128 {
            let hue = (shift + (key_idx as f32) * 10.0) % 360.0;
            let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
            rgb_buffer[key_idx * 3] = r;
            rgb_buffer[key_idx * 3 + 1] = g;
            rgb_buffer[key_idx * 3 + 2] = b;
        }

        // Enviar os 7 chunks de 56 bytes via Opcode 0x0B
        let mut offset = 0usize;
        while offset < 384 {
            let chunk_len = (384 - offset).min(CHUNK_SIZE);
            let mut pkt = [0u8; 64];
            pkt[0] = REPORT_ID_COMMAND;
            pkt[3] = 0x0B;
            pkt[4] = chunk_len as u8;
            pkt[5] = (offset & 0xFF) as u8;
            pkt[6] = ((offset >> 8) & 0xFF) as u8;
            pkt[7] = 0x00;
            pkt[8..8 + chunk_len].copy_from_slice(&rgb_buffer[offset..offset + chunk_len]);

            let sum = calculate_checksum(&pkt);
            pkt[1] = (sum & 0xFF) as u8;
            pkt[2] = ((sum >> 8) & 0xFF) as u8;

            dev.device_handle().write(&pkt)?;
            offset += chunk_len;
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    let total_elapsed = start_time.elapsed();
    let actual_fps = (total_frames as f64) / total_elapsed.as_secs_f64();
    println!("Animação concluída!");
    println!("Tempo total: {:.2}s | FPS real medido: {:.2}", total_elapsed.as_secs_f64(), actual_fps);
    println!("Nenhum Opcode 0x02 emitido - 0 desgaste de Flash!");
    Ok(())
}
