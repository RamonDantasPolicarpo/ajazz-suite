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
    // Canvas RGB persistente: GIFs otimizados enviam apenas sub-retângulos por frame.
    let mut rgb_buf = vec![0u8; width * height * 3];
    let mut prev_rect: Option<(usize, usize, usize, usize)> = None;
    let mut prev_bg = false;

    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|e| format!("Erro ao decodificar frame: {e}"))?
    {
        // Disposal do frame anterior: restaurar fundo preto na área ocupada.
        if let (true, Some((l, t, w, h))) = (prev_bg, prev_rect) {
            for y in t..(t + h).min(height) {
                for x in l..(l + w).min(width) {
                    let o = (y * width + x) * 3;
                    rgb_buf[o..o + 3].fill(0);
                }
            }
        }

        let (l, t) = (frame.left as usize, frame.top as usize);
        let (fw, fh) = (frame.width as usize, frame.height as usize);
        for fy in 0..fh {
            let y = t + fy;
            if y >= height {
                break;
            }
            for fx in 0..fw {
                let x = l + fx;
                if x >= width {
                    break;
                }
                let p = (fy * fw + fx) * 4;
                // Pixels transparentes mantêm o conteúdo anterior do canvas.
                if frame.buffer[p + 3] >= 128 {
                    let o = (y * width + x) * 3;
                    rgb_buf[o] = frame.buffer[p];
                    rgb_buf[o + 1] = frame.buffer[p + 1];
                    rgb_buf[o + 2] = frame.buffer[p + 2];
                }
            }
        }
        prev_rect = Some((l, t, fw, fh));
        prev_bg = frame.dispose == gif::DisposalMethod::Background;

        frames.push(project_image_to_matrix(width, height, &rgb_buf));
    }

    if frames.is_empty() {
        return Err("O arquivo GIF não contém frames válidos".to_string());
    }

    Ok(frames)
}
