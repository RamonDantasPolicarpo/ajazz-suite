use ajazz_protocol::{project_image_to_matrix, RgbColor, TOTAL_PHYSICAL_LEDS};
use gif::DecodeOptions;
use std::fs::File;

/// Decodifica um arquivo `.gif` e projeta todos os seus quadros
/// na matriz espacial física de 128 LEDs do teclado.
///
/// Todo o processamento de imagem é executado de forma antecipada (pre-rendering),
/// garantindo ZERO alocação de memória no loop de execução a 25 FPS.
pub fn decode_gif_to_matrix_frames(
    path: &str,
) -> Result<Vec<[RgbColor; TOTAL_PHYSICAL_LEDS]>, String> {
    let file = File::open(path).map_err(|e| format!("Falha ao abrir arquivo GIF: {e}"))?;
    let mut options = DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(file)
        .map_err(|e| format!("Erro no cabeçalho GIF: {e}"))?;

    let width = decoder.width() as usize;
    let height = decoder.height() as usize;
    if width == 0 || height == 0 {
        return Err("Dimensões do GIF inválidas (0x0)".to_string());
    }

    let mut frames = Vec::new();
    let mut rgb_buf = vec![0u8; width * height * 3];

    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|e| format!("Erro ao decodificar frame: {e}"))?
    {
        for (i, chunk) in frame.buffer.chunks_exact(4).enumerate() {
            if i < width * height {
                // Mistura alfa simples contra fundo preto
                let a = chunk[3] as u32;
                rgb_buf[i * 3] = ((chunk[0] as u32 * a) / 255) as u8;
                rgb_buf[i * 3 + 1] = ((chunk[1] as u32 * a) / 255) as u8;
                rgb_buf[i * 3 + 2] = ((chunk[2] as u32 * a) / 255) as u8;
            }
        }
        let matrix_frame = project_image_to_matrix(width, height, &rgb_buf);
        frames.push(matrix_frame);
    }

    if frames.is_empty() {
        return Err("O arquivo GIF não contém frames válidos".to_string());
    }

    Ok(frames)
}
