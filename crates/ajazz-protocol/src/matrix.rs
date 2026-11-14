use crate::RgbColor;
use serde::{Deserialize, Serialize};

/// Razão de aspecto física aproximada da matriz de teclas do AK820 (~572x210 px).
pub const AK820_MATRIX_ASPECT: f32 = 572.0 / 210.0;
pub const GIF_CROP_MIN_SIZE: f32 = 0.08;

/// Janela normalizada (0.0..1.0) usada para recortar/enquadrar quadros de GIF na matriz física.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GifCrop {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for GifCrop {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl GifCrop {
    pub fn normalized(self) -> Self {
        let w = if self.width.is_finite() {
            self.width.clamp(GIF_CROP_MIN_SIZE, 1.0)
        } else {
            1.0
        };
        let h = if self.height.is_finite() {
            self.height.clamp(GIF_CROP_MIN_SIZE, 1.0)
        } else {
            1.0
        };
        let x = if self.x.is_finite() {
            self.x.clamp(0.0, 1.0 - w)
        } else {
            0.0
        };
        let y = if self.y.is_finite() {
            self.y.clamp(0.0, 1.0 - h)
        } else {
            0.0
        };
        Self {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// Calcula recorte centralizado respeitando a proporção física do teclado (`AK820_MATRIX_ASPECT`).
    pub fn keyboard_aspect_fit(img_width: usize, img_height: usize) -> Self {
        if img_width == 0 || img_height == 0 {
            return Self::default();
        }
        let img_aspect = img_width as f32 / img_height as f32;
        if img_aspect > AK820_MATRIX_ASPECT {
            let w = (AK820_MATRIX_ASPECT / img_aspect).clamp(GIF_CROP_MIN_SIZE, 1.0);
            Self {
                x: (1.0 - w) * 0.5,
                y: 0.0,
                width: w,
                height: 1.0,
            }
        } else {
            let h = (img_aspect / AK820_MATRIX_ASPECT).clamp(GIF_CROP_MIN_SIZE, 1.0);
            Self {
                x: 0.0,
                y: (1.0 - h) * 0.5,
                width: 1.0,
                height: h,
            }
        }
    }
}

/// Total de posições de LEDs endereçáveis na matriz de controle do microcontrolador.
pub const TOTAL_PHYSICAL_LEDS: usize = 128;

/// Quantidade de teclas físicas visíveis no layout 75% do AK820.
pub const VISIBLE_KEYS_COUNT: usize = 82;

/// Posição normalizada e índice de LED físico de cada tecla.
#[derive(Debug, Clone, Copy)]
pub struct KeyLayoutPos {
    pub name: &'static str,
    pub light_id: usize,
    pub phys_led: usize,
    pub norm_x: f32,
    pub norm_y: f32,
}

/// Mapeamento espacial exato das 82 teclas do AK820 extraído por engenharia reversa.
pub const AK820_KEY_LAYOUT: [KeyLayoutPos; VISIBLE_KEYS_COUNT] = [
    // Fileira 0 (Funções e Esc)
    KeyPos { name: "Esc", light_id: 0, phys_led: 0, norm_x: 0.0, norm_y: 0.0063 },
    KeyPos { name: "F1", light_id: 1, phys_led: 8, norm_x: 0.0805, norm_y: 0.0 },
    KeyPos { name: "F2", light_id: 2, phys_led: 16, norm_x: 0.1454, norm_y: 0.0 },
    KeyPos { name: "F3", light_id: 3, phys_led: 24, norm_x: 0.2103, norm_y: 0.0 },
    KeyPos { name: "F4", light_id: 4, phys_led: 32, norm_x: 0.2752, norm_y: 0.0 },
    KeyPos { name: "F5", light_id: 5, phys_led: 40, norm_x: 0.3557, norm_y: 0.0 },
    KeyPos { name: "F6", light_id: 6, phys_led: 48, norm_x: 0.4206, norm_y: 0.0 },
    KeyPos { name: "F7", light_id: 7, phys_led: 56, norm_x: 0.4855, norm_y: 0.0 },
    KeyPos { name: "F8", light_id: 8, phys_led: 64, norm_x: 0.5503, norm_y: 0.0 },
    KeyPos { name: "F9", light_id: 9, phys_led: 72, norm_x: 0.6309, norm_y: 0.0 },
    KeyPos { name: "F10", light_id: 10, phys_led: 80, norm_x: 0.6957, norm_y: 0.0 },
    KeyPos { name: "F11", light_id: 11, phys_led: 88, norm_x: 0.7606, norm_y: 0.0 },
    KeyPos { name: "F12", light_id: 12, phys_led: 96, norm_x: 0.8255, norm_y: 0.0 },
    KeyPos { name: "Del", light_id: 56, phys_led: 114, norm_x: 0.9060, norm_y: 0.0 },

    // Fileira 1 (Números e Home)
    KeyPos { name: "`", light_id: 21, phys_led: 1, norm_x: 0.0, norm_y: 0.2264 },
    KeyPos { name: "1", light_id: 22, phys_led: 9, norm_x: 0.0649, norm_y: 0.2327 },
    KeyPos { name: "2", light_id: 23, phys_led: 17, norm_x: 0.1298, norm_y: 0.2327 },
    KeyPos { name: "3", light_id: 24, phys_led: 25, norm_x: 0.1946, norm_y: 0.2327 },
    KeyPos { name: "4", light_id: 25, phys_led: 33, norm_x: 0.2595, norm_y: 0.2327 },
    KeyPos { name: "5", light_id: 26, phys_led: 41, norm_x: 0.3244, norm_y: 0.2327 },
    KeyPos { name: "6", light_id: 27, phys_led: 49, norm_x: 0.3893, norm_y: 0.2327 },
    KeyPos { name: "7", light_id: 28, phys_led: 57, norm_x: 0.4541, norm_y: 0.2327 },
    KeyPos { name: "8", light_id: 29, phys_led: 65, norm_x: 0.5190, norm_y: 0.2327 },
    KeyPos { name: "9", light_id: 30, phys_led: 73, norm_x: 0.5839, norm_y: 0.2327 },
    KeyPos { name: "0", light_id: 31, phys_led: 81, norm_x: 0.6488, norm_y: 0.2327 },
    KeyPos { name: "-", light_id: 32, phys_led: 89, norm_x: 0.7136, norm_y: 0.2327 },
    KeyPos { name: "=", light_id: 33, phys_led: 97, norm_x: 0.7785, norm_y: 0.2327 },
    KeyPos { name: "Backspace", light_id: 34, phys_led: 105, norm_x: 0.8412, norm_y: 0.2327 },
    KeyPos { name: "Home", light_id: 36, phys_led: 121, norm_x: 1.0, norm_y: 0.2264 },

    // Fileira 2 (QWERTY e PgUp)
    KeyPos { name: "Tab", light_id: 42, phys_led: 2, norm_x: 0.0, norm_y: 0.4088 },
    KeyPos { name: "Q", light_id: 43, phys_led: 10, norm_x: 0.0962, norm_y: 0.4088 },
    KeyPos { name: "W", light_id: 44, phys_led: 18, norm_x: 0.1611, norm_y: 0.4088 },
    KeyPos { name: "E", light_id: 45, phys_led: 26, norm_x: 0.2260, norm_y: 0.4088 },
    KeyPos { name: "R", light_id: 46, phys_led: 34, norm_x: 0.2908, norm_y: 0.4088 },
    KeyPos { name: "T", light_id: 47, phys_led: 42, norm_x: 0.3557, norm_y: 0.4088 },
    KeyPos { name: "Y", light_id: 48, phys_led: 50, norm_x: 0.4206, norm_y: 0.4088 },
    KeyPos { name: "U", light_id: 49, phys_led: 58, norm_x: 0.4855, norm_y: 0.4088 },
    KeyPos { name: "I", light_id: 50, phys_led: 66, norm_x: 0.5503, norm_y: 0.4088 },
    KeyPos { name: "O", light_id: 51, phys_led: 74, norm_x: 0.6152, norm_y: 0.4088 },
    KeyPos { name: "P", light_id: 52, phys_led: 82, norm_x: 0.6801, norm_y: 0.4088 },
    KeyPos { name: "[", light_id: 53, phys_led: 90, norm_x: 0.7450, norm_y: 0.4088 },
    KeyPos { name: "]", light_id: 54, phys_led: 98, norm_x: 0.8098, norm_y: 0.4088 },
    KeyPos { name: "\\", light_id: 55, phys_led: 106, norm_x: 0.8747, norm_y: 0.4088 },
    KeyPos { name: "PgUp", light_id: 37, phys_led: 122, norm_x: 1.0, norm_y: 0.4088 },

    // Fileira 3 (ASDF e PgDn)
    KeyPos { name: "Caps", light_id: 63, phys_led: 3, norm_x: 0.0, norm_y: 0.5912 },
    KeyPos { name: "A", light_id: 64, phys_led: 11, norm_x: 0.1119, norm_y: 0.5912 },
    KeyPos { name: "S", light_id: 65, phys_led: 19, norm_x: 0.1767, norm_y: 0.5912 },
    KeyPos { name: "D", light_id: 66, phys_led: 27, norm_x: 0.2416, norm_y: 0.5912 },
    KeyPos { name: "F", light_id: 67, phys_led: 35, norm_x: 0.3065, norm_y: 0.5912 },
    KeyPos { name: "G", light_id: 68, phys_led: 43, norm_x: 0.3714, norm_y: 0.5912 },
    KeyPos { name: "H", light_id: 69, phys_led: 51, norm_x: 0.4362, norm_y: 0.5912 },
    KeyPos { name: "J", light_id: 70, phys_led: 59, norm_x: 0.5011, norm_y: 0.5912 },
    KeyPos { name: "K", light_id: 71, phys_led: 67, norm_x: 0.5660, norm_y: 0.5912 },
    KeyPos { name: "L", light_id: 72, phys_led: 75, norm_x: 0.6309, norm_y: 0.5912 },
    KeyPos { name: ";", light_id: 73, phys_led: 83, norm_x: 0.6957, norm_y: 0.5912 },
    KeyPos { name: "'", light_id: 74, phys_led: 91, norm_x: 0.7606, norm_y: 0.5912 },
    KeyPos { name: "Enter", light_id: 75, phys_led: 107, norm_x: 0.8255, norm_y: 0.5912 },
    KeyPos { name: "PgDn", light_id: 58, phys_led: 115, norm_x: 1.0, norm_y: 0.5912 },

    // Fileira 4 (ZXCV, Shift, Seta Cima e End)
    KeyPos { name: "LShift", light_id: 84, phys_led: 4, norm_x: 0.0, norm_y: 0.7736 },
    KeyPos { name: "Z", light_id: 85, phys_led: 20, norm_x: 0.1454, norm_y: 0.7736 },
    KeyPos { name: "X", light_id: 86, phys_led: 28, norm_x: 0.2103, norm_y: 0.7736 },
    KeyPos { name: "C", light_id: 87, phys_led: 36, norm_x: 0.2752, norm_y: 0.7736 },
    KeyPos { name: "V", light_id: 88, phys_led: 44, norm_x: 0.3400, norm_y: 0.7736 },
    KeyPos { name: "B", light_id: 89, phys_led: 52, norm_x: 0.4049, norm_y: 0.7736 },
    KeyPos { name: "N", light_id: 90, phys_led: 60, norm_x: 0.4698, norm_y: 0.7736 },
    KeyPos { name: "M", light_id: 91, phys_led: 68, norm_x: 0.5347, norm_y: 0.7736 },
    KeyPos { name: ",", light_id: 92, phys_led: 76, norm_x: 0.5996, norm_y: 0.7736 },
    KeyPos { name: ".", light_id: 93, phys_led: 84, norm_x: 0.6644, norm_y: 0.7736 },
    KeyPos { name: "/", light_id: 94, phys_led: 92, norm_x: 0.7293, norm_y: 0.7736 },
    KeyPos { name: "RShift", light_id: 95, phys_led: 108, norm_x: 0.7919, norm_y: 0.7736 },
    KeyPos { name: "End", light_id: 57, phys_led: 123, norm_x: 1.0, norm_y: 0.7736 },
    KeyPos { name: "Up", light_id: 96, phys_led: 116, norm_x: 0.9217, norm_y: 0.8176 },

    // Fileira 5 (Ctrl, Alt, Espaço, Setas)
    KeyPos { name: "LCtrl", light_id: 105, phys_led: 5, norm_x: 0.0, norm_y: 0.9623 },
    KeyPos { name: "LWin", light_id: 106, phys_led: 13, norm_x: 0.0805, norm_y: 0.9623 },
    KeyPos { name: "LAlt", light_id: 107, phys_led: 21, norm_x: 0.1611, norm_y: 0.9623 },
    KeyPos { name: "Space", light_id: 108, phys_led: 45, norm_x: 0.2416, norm_y: 0.9623 },
    KeyPos { name: "RAlt", light_id: 109, phys_led: 77, norm_x: 0.6465, norm_y: 0.9623 },
    KeyPos { name: "Fn", light_id: 110, phys_led: 85, norm_x: 0.7114, norm_y: 0.9623 },
    KeyPos { name: "RCtrl", light_id: 112, phys_led: 93, norm_x: 0.7763, norm_y: 0.9623 },
    KeyPos { name: "Left", light_id: 113, phys_led: 109, norm_x: 0.8568, norm_y: 1.0 },
    KeyPos { name: "Down", light_id: 114, phys_led: 117, norm_x: 0.9217, norm_y: 1.0 },
    KeyPos { name: "Right", light_id: 115, phys_led: 125, norm_x: 0.9866, norm_y: 1.0 },
];

type KeyPos = KeyLayoutPos;

/// Projeta um quadro de imagem RGB na matriz física de 128 LEDs do teclado.
/// Sem alocações de heap no loop de renderização (Zero-Allocation).
#[inline]
pub fn project_image_to_matrix(
    img_width: usize,
    img_height: usize,
    rgb_data: &[u8],
) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
    project_image_to_matrix_with_crop(img_width, img_height, rgb_data, GifCrop::default())
}

/// Projeta uma janela recortada/enquadrada (`crop`) de um quadro RGB na matriz física do AK820.
#[inline]
pub fn project_image_to_matrix_with_crop(
    img_width: usize,
    img_height: usize,
    rgb_data: &[u8],
    crop: GifCrop,
) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
    let mut matrix = [RgbColor::default(); TOTAL_PHYSICAL_LEDS];
    if img_width == 0 || img_height == 0 || rgb_data.len() < img_width * img_height * 3 {
        return matrix;
    }

    let crop = crop.normalized();
    let w_max = (img_width - 1) as f32;
    let h_max = (img_height - 1) as f32;

    for key in &AK820_KEY_LAYOUT {
        let nx = (crop.x + key.norm_x * crop.width).clamp(0.0, 1.0);
        let ny = (crop.y + key.norm_y * crop.height).clamp(0.0, 1.0);
        let px = ((nx * w_max).round() as usize).min(img_width - 1);
        let py = ((ny * h_max).round() as usize).min(img_height - 1);
        let idx = (py * img_width + px) * 3;

        let r = rgb_data[idx];
        let g = rgb_data[idx + 1];
        let b = rgb_data[idx + 2];

        if key.phys_led < TOTAL_PHYSICAL_LEDS {
            matrix[key.phys_led] = RgbColor::new(r, g, b);
        }
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_normalization_and_projection_focus_quadrant() {
        let crop = GifCrop {
            x: 0.9,
            y: 0.8,
            width: 0.5,
            height: 0.5,
        }
        .normalized();
        assert!((crop.x - 0.5).abs() < 1e-5);
        assert!((crop.y - 0.5).abs() < 1e-5);

        // Imagem 4x4: quadrante inferior-direito azul, resto vermelho.
        let mut rgb = vec![0u8; 4 * 4 * 3];
        for y in 0..4 {
            for x in 0..4 {
                let idx = (y * 4 + x) * 3;
                if x >= 2 && y >= 2 {
                    rgb[idx + 2] = 255;
                } else {
                    rgb[idx] = 255;
                }
            }
        }
        let focused = GifCrop {
            x: 0.65,
            y: 0.65,
            width: 0.35,
            height: 0.35,
        };
        let matrix = project_image_to_matrix_with_crop(4, 4, &rgb, focused);
        // Esc (phys_led 0) e Seta Direita/Espaço ficam dentro do quadrante azul.
        assert_eq!(matrix[0], RgbColor::new(0, 0, 255));
    }

    #[test]
    fn keyboard_aspect_fit_centers_square_image_vertically() {
        let fit = GifCrop::keyboard_aspect_fit(200, 200);
        assert!((fit.width - 1.0).abs() < 1e-5);
        assert!(fit.height < 0.5);
        assert!((fit.y - (1.0 - fit.height) * 0.5).abs() < 1e-5);
    }
}
