use ajazz_protocol::{project_image_to_matrix_with_crop, GifCrop, RgbColor, TOTAL_PHYSICAL_LEDS};
use gif::DecodeOptions;
use std::fs::File;

/// Quadro pré-renderizado pronto para envio sem alocação em `0x12`.
#[derive(Debug, Clone, Copy)]
pub struct DecodedGifFrame {
    pub matrix: [RgbColor; TOTAL_PHYSICAL_LEDS],
    pub delay_ms: u64,
}

/// Decodifica um arquivo `.gif` e projeta todos os seus quadros (com `crop` opcional)
/// na matriz espacial física de 128 LEDs do teclado.
///
/// Todo o processamento de imagem é executado de forma antecipada (pre-rendering),
/// garantindo ZERO alocação de memória no loop de execução.
pub fn decode_gif_to_matrix_frames(
    path: &str,
    crop: GifCrop,
) -> Result<Vec<DecodedGifFrame>, String> {
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

        // `frame.delay` é em unidades de 10 ms; `0` segue convenção de browsers (100 ms).
        let delay_ms = if frame.delay == 0 {
            100
        } else {
            (frame.delay as u64 * 10).max(20)
        };

        frames.push(DecodedGifFrame {
            matrix: project_image_to_matrix_with_crop(width, height, &rgb_buf, crop),
            delay_ms,
        });
    }

    if frames.is_empty() {
        return Err("O arquivo GIF não contém frames válidos".to_string());
    }

    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_embedded_delays_and_applies_crop() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ajazz_test_gif_{}.gif", std::process::id()));

        {
            let mut file = File::create(&path).unwrap();
            let mut enc = gif::Encoder::new(&mut file, 4, 4, &[]).unwrap();
            // Frame 0: metade esquerda vermelha, metade direita azul; delay 25 (=250ms)
            let mut px0 = vec![0u8; 4 * 4 * 4];
            for y in 0..4 {
                for x in 0..4 {
                    let i = (y * 4 + x) * 4;
                    if x < 2 {
                        px0[i] = 255;
                    } else {
                        px0[i + 2] = 255;
                    }
                    px0[i + 3] = 255;
                }
            }
            let mut f0 = gif::Frame::from_rgba_speed(4, 4, &mut px0, 10);
            f0.delay = 25;
            enc.write_frame(&f0).unwrap();

            // Frame 1: delay 0 -> fallback 100ms
            let mut px1 = [0u8, 255, 0, 255].repeat(16);
            let mut f1 = gif::Frame::from_rgba_speed(4, 4, &mut px1, 10);
            f1.delay = 0;
            enc.write_frame(&f1).unwrap();
        }

        let crop = GifCrop {
            x: 0.65,
            y: 0.0,
            width: 0.35,
            height: 1.0,
        };
        let decoded = decode_gif_to_matrix_frames(path.to_str().unwrap(), crop).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].delay_ms, 250);
        assert_eq!(decoded[1].delay_ms, 100);
        // Com crop na metade direita, Esc (phys_led 0) recebe azul no frame 0 (quantizado pela paleta GIF).
        assert!(decoded[0].matrix[0].b > 200);
        assert!(decoded[0].matrix[0].r < 50);
    }
}
