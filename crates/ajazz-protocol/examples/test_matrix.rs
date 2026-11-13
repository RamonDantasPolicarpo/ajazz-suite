use ajazz_protocol::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Teste de Matriz RGB Customizada (Opcode 0x0B) no AK820 ===");
    let api = hidapi::HidApi::new()?;
    let mut dev = Ak820Device::open(&api)?;
    println!("Teclado AK820 conectado com sucesso!");

    // 1. Iniciar transação (Opcode 0x01)
    let p_begin = build_begin_transaction_packet();
    dev.device_handle().write(&p_begin)?;
    println!("1. Transação iniciada (Opcode 0x01)");

    // 2. Mudar modo para Custom Lighting (Mode 20 / 0x14) via Opcode 0x06
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
    println!("2. Modo de iluminação alterado para Custom (20 / 0x14)");

    // 3. Montar buffer de 128 teclas * 3 bytes = 384 bytes
    let mut rgb_buffer = [0u8; 384];

    // Vamos acender WASD em VERMELHO e Setas em AZUL
    // E todas as teclas de F1 a F12 em VERDE
    for key_idx in 0..128 {
        // Exemplo: acender alternado ou específico
        // Teclas 0 a 15: VERMELHO
        // Teclas 16 a 31: VERDE
        // Teclas 32 a 47: AZUL
        // Teclas 48 a 63: AMARELO
        // Teclas 64 a 79: CIANO
        // Teclas 80 a 95: ROXO
        // Teclas 96 a 111: BRANCO
        // Teclas 112 a 127: LARANJA
        let (r, g, b) = match key_idx / 16 {
            0 => (255, 0, 0),      // Vermelho
            1 => (0, 255, 0),      // Verde
            2 => (0, 0, 255),      // Azul
            3 => (255, 255, 0),    // Amarelo
            4 => (0, 255, 255),    // Ciano
            5 => (255, 0, 255),    // Roxo
            6 => (255, 255, 255),  // Branco
            _ => (255, 128, 0),    // Laranja
        };
        rgb_buffer[key_idx * 3] = r;
        rgb_buffer[key_idx * 3 + 1] = g;
        rgb_buffer[key_idx * 3 + 2] = b;
    }

    // 4. Enviar em chunks de 56 bytes com Opcode 0x0B
    const CHUNK_SIZE: usize = 56;
    let mut offset = 0usize;
    let total_len = rgb_buffer.len(); // 384
    let mut chunk_idx = 0;

    while offset < total_len {
        let chunk_len = (total_len - offset).min(CHUNK_SIZE);
        let mut pkt = [0u8; 64];
        pkt[0] = REPORT_ID_COMMAND;
        pkt[3] = 0x0B; // Opcode 0x0B
        pkt[4] = chunk_len as u8;
        pkt[5] = (offset & 0xFF) as u8;
        pkt[6] = ((offset >> 8) & 0xFF) as u8;
        pkt[7] = 0x00;
        pkt[8..8 + chunk_len].copy_from_slice(&rgb_buffer[offset..offset + chunk_len]);

        let sum = calculate_checksum(&pkt);
        pkt[1] = (sum & 0xFF) as u8;
        pkt[2] = ((sum >> 8) & 0xFF) as u8;

        dev.device_handle().write(&pkt)?;
        println!("   Chunk {}: offset={}, len={}", chunk_idx, offset, chunk_len);

        offset += chunk_len;
        chunk_idx += 1;
    }

    println!("3. Matriz de LEDs enviada com sucesso via Opcode 0x0B!");
    println!("OBS: Opcode 0x02 NÃO foi enviado (buffer volátil em RAM preservando a memória Flash).");
    Ok(())
}
