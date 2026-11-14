use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ajazz_protocol::{GifCrop, GIF_CROP_MIN_SIZE};
use gif::{DecodeOptions, DisposalMethod};

const MAX_PREVIEW_DIM: u32 = 360;

#[derive(Debug, Clone)]
pub struct GifPreviewFrame {
    pub rgba: Vec<u8>,
    pub delay_ms: u64,
}

#[derive(Debug, Clone)]
pub struct GifPreviewData {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<GifPreviewFrame>,
}

/// Decodes a `.gif` file into RGBA8 composited preview frames, downscaling if larger than `MAX_PREVIEW_DIM`.
pub fn load_gif_preview(path: &Path) -> Result<GifPreviewData, String> {
    let file = File::open(path).map_err(|e| format!("Falha ao abrir GIF: {e}"))?;
    let mut opts = DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = opts
        .read_info(BufReader::new(file))
        .map_err(|e| format!("Falha ao ler cabeçalho GIF: {e}"))?;

    let src_w = decoder.width() as usize;
    let src_h = decoder.height() as usize;
    if src_w == 0 || src_h == 0 {
        return Err("Dimensões do GIF inválidas".to_string());
    }

    let (dst_w, dst_h) = fit_dimensions(src_w as u32, src_h as u32, MAX_PREVIEW_DIM);
    let mut canvas = vec![0u8; src_w * src_h * 4];
    let mut prev_canvas = canvas.clone();
    let mut frames = Vec::new();

    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|e| format!("Falha ao decodificar frame GIF: {e}"))?
    {
        if frame.dispose == DisposalMethod::Previous {
            prev_canvas.copy_from_slice(&canvas);
        }

        let fx = frame.left as usize;
        let fy = frame.top as usize;
        let fw = frame.width as usize;
        let fh = frame.height as usize;

        for row in 0..fh {
            let cy = fy + row;
            if cy >= src_h {
                break;
            }
            for col in 0..fw {
                let cx = fx + col;
                if cx >= src_w {
                    break;
                }
                let src_idx = (row * fw + col) * 4;
                if src_idx + 3 >= frame.buffer.len() {
                    continue;
                }
                let alpha = frame.buffer[src_idx + 3];
                if alpha == 0 {
                    continue;
                }
                let dst_idx = (cy * src_w + cx) * 4;
                if alpha == 255 {
                    canvas[dst_idx..dst_idx + 4].copy_from_slice(&frame.buffer[src_idx..src_idx + 4]);
                } else {
                    let a = alpha as u16;
                    let inv = 255 - a;
                    for c in 0..3 {
                        canvas[dst_idx + c] =
                            ((frame.buffer[src_idx + c] as u16 * a + canvas[dst_idx + c] as u16 * inv)
                                / 255) as u8;
                    }
                    canvas[dst_idx + 3] = 255;
                }
            }
        }

        let rgba = downscale_rgba(&canvas, src_w, src_h, dst_w as usize, dst_h as usize);
        let delay_ms = if frame.delay == 0 {
            100
        } else {
            (frame.delay as u64 * 10).max(20)
        };
        frames.push(GifPreviewFrame { rgba, delay_ms });

        match frame.dispose {
            DisposalMethod::Background => {
                for row in 0..fh {
                    let cy = fy + row;
                    if cy >= src_h {
                        break;
                    }
                    for col in 0..fw {
                        let cx = fx + col;
                        if cx >= src_w {
                            break;
                        }
                        let dst_idx = (cy * src_w + cx) * 4;
                        canvas[dst_idx..dst_idx + 4].fill(0);
                    }
                }
            }
            DisposalMethod::Previous => {
                canvas.copy_from_slice(&prev_canvas);
            }
            _ => {}
        }
    }

    if frames.is_empty() {
        return Err("Nenhum frame encontrado no GIF".to_string());
    }

    Ok(GifPreviewData {
        width: dst_w,
        height: dst_h,
        frames,
    })
}

fn fit_dimensions(w: u32, h: u32, max_dim: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_dim {
        return (w.max(1), h.max(1));
    }
    let scale = max_dim as f32 / longest as f32;
    (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    )
}

fn downscale_rgba(src: &[u8], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }
    let mut out = vec![0u8; dst_w * dst_h * 4];
    for dy in 0..dst_h {
        let sy = (dy * src_h / dst_h).min(src_h - 1);
        for dx in 0..dst_w {
            let sx = (dx * src_w / dst_w).min(src_w - 1);
            let s_idx = (sy * src_w + sx) * 4;
            let d_idx = (dy * dst_w + dx) * 4;
            out[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
        }
    }
    out
}

/// Scales the normalized crop rectangle around its center while preserving its aspect ratio.
/// `zoom_factor > 1.0` zooms in (smaller box); `< 1.0` zooms out (larger box).
pub fn crop_box_zoom(crop: GifCrop, zoom_factor: f32) -> GifCrop {
    if !zoom_factor.is_finite() || zoom_factor <= 0.0 {
        return crop.normalized();
    }
    let cur = crop.normalized();
    let cx = cur.x + cur.width * 0.5;
    let cy = cur.y + cur.height * 0.5;
    let aspect = (cur.width / cur.height).max(0.05);

    let mut new_w = cur.width / zoom_factor;
    let mut new_h = cur.height / zoom_factor;

    if new_w > 1.0 {
        new_w = 1.0;
        new_h = (new_w / aspect).clamp(GIF_CROP_MIN_SIZE, 1.0);
    }
    if new_h > 1.0 {
        new_h = 1.0;
        new_w = (new_h * aspect).clamp(GIF_CROP_MIN_SIZE, 1.0);
    }
    if new_w < GIF_CROP_MIN_SIZE {
        new_w = GIF_CROP_MIN_SIZE;
        new_h = (new_w / aspect).clamp(GIF_CROP_MIN_SIZE, 1.0);
    }
    if new_h < GIF_CROP_MIN_SIZE {
        new_h = GIF_CROP_MIN_SIZE;
        new_w = (new_h * aspect).clamp(GIF_CROP_MIN_SIZE, 1.0);
    }

    let new_x = (cx - new_w * 0.5).clamp(0.0, 1.0 - new_w);
    let new_y = (cy - new_h * 0.5).clamp(0.0, 1.0 - new_h);

    GifCrop {
        x: new_x,
        y: new_y,
        width: new_w,
        height: new_h,
    }
    .normalized()
}

/// Shifts the normalized crop rectangle by `(dx, dy)` within `[0.0, 1.0]`.
pub fn crop_box_pan(crop: GifCrop, dx: f32, dy: f32) -> GifCrop {
    let cur = crop.normalized();
    let max_x = (1.0 - cur.width).max(0.0);
    let max_y = (1.0 - cur.height).max(0.0);
    GifCrop {
        x: (cur.x + dx).clamp(0.0, max_x),
        y: (cur.y + dy).clamp(0.0, max_y),
        width: cur.width,
        height: cur.height,
    }
    .normalized()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn crop_zoom_and_pan_stay_within_unit_bounds() {
        let full = GifCrop::default();
        let zoomed = crop_box_zoom(full, 2.0);
        assert!((zoomed.width - 0.5).abs() < 1e-4);
        assert!((zoomed.height - 0.5).abs() < 1e-4);
        assert!((zoomed.x - 0.25).abs() < 1e-4);

        let panned = crop_box_pan(zoomed, 1.0, -1.0);
        assert!((panned.x - 0.5).abs() < 1e-4);
        assert!(panned.y.abs() < 1e-4);

        let zoomed_out = crop_box_zoom(panned, 0.2);
        assert!((zoomed_out.width - 1.0).abs() < 1e-4);
        assert!(zoomed_out.x.abs() < 1e-4);
    }

    #[test]
    fn loads_composited_gif_preview_frames() {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 2, 2, &[255, 0, 0, 0, 255, 0]).unwrap();
            let f1 = gif::Frame {
                width: 2,
                height: 2,
                delay: 20,
                buffer: std::borrow::Cow::Borrowed(&[0, 0, 0, 0]),
                ..Default::default()
            };
            encoder.write_frame(&f1).unwrap();

            let f2 = gif::Frame {
                width: 2,
                height: 2,
                delay: 0,
                buffer: std::borrow::Cow::Borrowed(&[1, 1, 1, 1]),
                ..Default::default()
            };
            encoder.write_frame(&f2).unwrap();
        }

        let tmp = std::env::temp_dir().join("ajazz_gui_preview_test.gif");
        let mut file = File::create(&tmp).unwrap();
        file.write_all(&bytes).unwrap();
        drop(file);

        let preview = load_gif_preview(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(preview.width, 2);
        assert_eq!(preview.height, 2);
        assert_eq!(preview.frames.len(), 2);
        assert_eq!(preview.frames[0].delay_ms, 200);
        assert_eq!(preview.frames[1].delay_ms, 100);
        assert_eq!(&preview.frames[0].rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&preview.frames[1].rgba[0..4], &[0, 255, 0, 255]);
    }
}
