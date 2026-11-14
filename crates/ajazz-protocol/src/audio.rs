use crate::{
    AK820_KEY_LAYOUT, BRIGHTNESS_LUT, LightingConfig, RgbColor, TOTAL_PHYSICAL_LEDS,
};
use std::f64::consts::PI;

/// Sentinela de célula vazia nas tabelas espaciais `.rdata` (`0x6828c0`, `0x6826c8`).
pub const AUDIO_CELL_EMPTY: u8 = 0xAA;

/// Quantidade de bandas de espectro recebidas pelos renderizadores de áudio (`0x19`).
pub const AUDIO_BAND_COUNT: usize = 25;

/// Tamanho da janela PCM e FFT usado pelo `AudioRecord.exe` (`0x400`).
pub const AUDIO_FFT_SIZE: usize = 1024;

/// Colunas x Linhas da matriz espacial `.rdata 0x6828c0`.
pub const AUDIO_GRID_COLS: usize = 23;
pub const AUDIO_GRID_ROWS: usize = 6;

/// Tabela espacial `23 × 6` extraída de `.rdata 0x6828c0` (`col * 6 + row -> light_id`).
#[rustfmt::skip]
pub const AUDIO_GRID_23X6: [u8; AUDIO_GRID_COLS * AUDIO_GRID_ROWS] = [
    0xaa, 0x78, 0x79, 0x7a, 0x7b, 0x7c, // col 0
    0xaa, 0x15, 0x2a, 0x3f, 0x54, 0x69, // col 1
    0x00, 0x16, 0x2b, 0x40, 0x66, 0x6a, // col 2
    0x01, 0x17, 0x2c, 0x41, 0x55, 0x14, // col 3
    0x02, 0x18, 0x2d, 0x42, 0x56, 0x6b, // col 4
    0x03, 0x19, 0x2e, 0x43, 0x57, 0x7e, // col 5
    0x04, 0x1a, 0x2f, 0x44, 0x58, 0x7f, // col 6
    0x12, 0x1b, 0x30, 0x45, 0x59, 0x6c, // col 7
    0x05, 0x1c, 0x31, 0x46, 0x5a, 0x80, // col 8
    0x06, 0x1d, 0x32, 0x47, 0x5b, 0x81, // col 9
    0x07, 0x1e, 0x33, 0x48, 0x5c, 0x6d, // col 10
    0x08, 0x1f, 0x34, 0x49, 0x5d, 0x7d, // col 11
    0x09, 0x20, 0x35, 0x4a, 0x5e, 0x6e, // col 12
    0x0a, 0x21, 0x36, 0x3e, 0x64, 0x6f, // col 13
    0x0b, 0x22, 0x37, 0x4b, 0x13, 0x82, // col 14
    0x0c, 0x65, 0x67, 0x4f, 0x5f, 0x70, // col 15
    0x0d, 0x23, 0xaa, 0x52, 0x50, 0x71, // col 16
    0x0e, 0xaa, 0x39, 0x53, 0x60, 0x72, // col 17
    0x0f, 0xaa, 0xaa, 0x11, 0x51, 0x73, // col 18
    0x38, 0x26, 0x3b, 0x4c, 0x61, 0x74, // col 19
    0x3a, 0x27, 0x3c, 0x4d, 0x62, 0x68, // col 20
    0x25, 0x28, 0x3d, 0x4e, 0x63, 0x75, // col 21
    0x24, 0x29, 0x10, 0x77, 0x76, 0x83, // col 22
];

/// Tabela de anéis concêntricos `12 × 12` extraída de `.rdata 0x6826c8` (`ring * 12 + slot -> light_id`).
#[rustfmt::skip]
pub const AUDIO_RING_12X12: [u8; 12 * 12] = [
    0x31, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x1c, 0x1d, 0x30, 0x32, 0x46, 0x45, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x06, 0x07, 0x08, 0x1b, 0x1e, 0x2f, 0x33, 0x44, 0x47, 0x59, 0x5a, 0x5b,
    0x05, 0x09, 0x1a, 0x1f, 0x2e, 0x43, 0x58, 0x6c, 0x5c, 0x48, 0x34, 0xff,
    0x04, 0x19, 0x2d, 0x42, 0x57, 0x0a, 0x20, 0x35, 0x49, 0x5d, 0x6d, 0xff,
    0x03, 0x18, 0x2c, 0x56, 0x0b, 0x21, 0x36, 0x4a, 0x5e, 0x6e, 0x3e, 0x41,
    0x02, 0x17, 0x2b, 0x40, 0x55, 0x6b, 0x0c, 0x22, 0x37, 0x4b, 0x5f, 0x70,
    0x01, 0x16, 0x66, 0x6a, 0x71, 0x60, 0x38, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x00, 0x15, 0x2a, 0x3f, 0x54, 0x69, 0x72, 0x3a, 0x39, 0x25, 0x24, 0xff,
    0x73, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

const RAMP_UP: [u8; 32] = [
    0x01, 0x03, 0x05, 0x07, 0x0a, 0x0e, 0x12, 0x16, 0x1a, 0x20, 0x26, 0x2c, 0x32, 0x39, 0x41, 0x49,
    0x51, 0x59, 0x63, 0x6d, 0x77, 0x81, 0x8c, 0x98, 0xa4, 0xb0, 0xbc, 0xc8, 0xd5, 0xe3, 0xf1, 0xff,
];

const RAMP_DOWN: [u8; 32] = [
    0xf8, 0xea, 0xdc, 0xce, 0xc2, 0xb6, 0xaa, 0x9e, 0x92, 0x86, 0x7c, 0x72, 0x68, 0x5e, 0x55, 0x4d,
    0x45, 0x3d, 0x35, 0x2f, 0x29, 0x23, 0x1d, 0x18, 0x14, 0x10, 0x0c, 0x08, 0x06, 0x04, 0x02, 0x00,
];

const fn build_rainbow_lut() -> [RgbColor; 192] {
    let mut lut = [RgbColor::new(0, 0, 0); 192];
    let mut i = 0;
    while i < 32 {
        lut[i] = RgbColor::new(255, RAMP_UP[i], 0);
        lut[32 + i] = RgbColor::new(RAMP_DOWN[i], 255, 0);
        lut[64 + i] = RgbColor::new(0, 255, RAMP_UP[i]);
        lut[96 + i] = RgbColor::new(0, RAMP_DOWN[i], 255);
        lut[128 + i] = RgbColor::new(RAMP_UP[i], 0, 255);
        lut[160 + i] = RgbColor::new(255, 0, RAMP_DOWN[i]);
        i += 1;
    }
    lut
}

/// Paleta arco-íris de 192 cores derivada das curvas gama-corrigidas de `.rdata 0x681cf8`.
pub const AUDIO_RAINBOW_192: [RgbColor; 192] = build_rainbow_lut();

const fn build_light_id_map() -> [u8; AUDIO_CELL_EMPTY as usize] {
    let mut map = [0xFF; AUDIO_CELL_EMPTY as usize];
    let mut idx = 0;
    while idx < AK820_KEY_LAYOUT.len() {
        let key = AK820_KEY_LAYOUT[idx];
        if key.light_id < AUDIO_CELL_EMPTY as usize && key.phys_led < TOTAL_PHYSICAL_LEDS {
            map[key.light_id] = key.phys_led as u8;
        }
        idx += 1;
    }
    map
}

const LIGHT_ID_TO_PHYS_LED: [u8; AUDIO_CELL_EMPTY as usize] = build_light_id_map();

/// Sub-estilos de reação de áudio suportados pelo protocolo (`sub_4966c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum AudioStyle {
    #[default]
    VerticalBars = 0,
    RainbowPulse = 1,
    CenterWave = 2,
    ConcentricRings = 3,
    PeakDrop = 4,
    HorizontalBars = 5,
    ZigZagBars = 6,
}

impl AudioStyle {
    pub const ALL: [AudioStyle; 7] = [
        Self::VerticalBars,
        Self::RainbowPulse,
        Self::CenterWave,
        Self::ConcentricRings,
        Self::PeakDrop,
        Self::HorizontalBars,
        Self::ZigZagBars,
    ];

    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::RainbowPulse,
            2 => Self::CenterWave,
            3 => Self::ConcentricRings,
            4 => Self::PeakDrop,
            5 => Self::HorizontalBars,
            6 => Self::ZigZagBars,
            _ => Self::VerticalBars,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::VerticalBars => "Barras Verticais",
            Self::RainbowPulse => "Pulso Arco-Íris",
            Self::CenterWave => "Onda Central",
            Self::ConcentricRings => "Anéis Concêntricos",
            Self::PeakDrop => "Barras com Pico",
            Self::HorizontalBars => "Barras Horizontais",
            Self::ZigZagBars => "Zig-Zag Horizontal",
        }
    }
}

/// Estado pré-alocado para captura espectral e renderização de áudio em tempo real (Zero-Allocation).
pub struct AudioVisualizer {
    energy: [u8; AUDIO_CELL_EMPTY as usize],
    key_hue: [u8; AUDIO_CELL_EMPTY as usize],
    peaks: [bool; AUDIO_GRID_COLS],
    color_idx: u8,
    band_offset: u8,
    scroll_ticks: u8,
    prev_bins: [u32; 500],
    fft_re: [f64; AUDIO_FFT_SIZE],
    fft_im: [f64; AUDIO_FFT_SIZE],
    fft_bins: [u32; 500],
}

impl Default for AudioVisualizer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioVisualizer {
    pub const fn new() -> Self {
        Self {
            energy: [0; AUDIO_CELL_EMPTY as usize],
            key_hue: [0; AUDIO_CELL_EMPTY as usize],
            peaks: [false; AUDIO_GRID_COLS],
            color_idx: 0,
            band_offset: 0,
            scroll_ticks: 0,
            prev_bins: [0; 500],
            fft_re: [0.0; AUDIO_FFT_SIZE],
            fft_im: [0.0; AUDIO_FFT_SIZE],
            fft_bins: [0; 500],
        }
    }

    pub fn reset(&mut self) {
        self.energy.fill(0);
        self.key_hue.fill(0);
        self.peaks.fill(false);
        self.prev_bins.fill(0);
        self.color_idx = 0;
        self.band_offset = 0;
        self.scroll_ticks = 0;
    }

    /// Converte 1024 amostras PCM (`-1.0..=1.0`) em quadro pronto da matriz física sem alocar heap.
    pub fn process_pcm_frame(
        &mut self,
        samples: &[f32; AUDIO_FFT_SIZE],
        cfg: &LightingConfig,
    ) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
        self.compute_fft_bins(samples);
        let bands = self.extract_bands_from_bins();
        self.render_frame(&bands, cfg)
    }

    /// Executa FFT radix-2 (`N = 1024`) compatível com `sub_401570` / `sub_401930` do `AudioRecord.exe`.
    pub fn compute_fft_bins(&mut self, samples: &[f32; AUDIO_FFT_SIZE]) {
        for (i, &s) in samples.iter().enumerate() {
            // Equivalente a `(u16 - 0x8000)` normalizado para amplitude de 16 bits.
            self.fft_re[i] = (s.clamp(-1.0, 1.0) as f64) * 32768.0;
            self.fft_im[i] = 0.0;
        }
        radix2_fft_1024(&mut self.fft_re, &mut self.fft_im);

        // Ignora DC (bin 0) e calcula 499 bins com escala `1 / 1024` (`0x4160e8`).
        const INV_N: f64 = 1.0 / (AUDIO_FFT_SIZE as f64);
        self.fft_bins.fill(0);
        for k in 0..499 {
            let re = self.fft_re[k + 1];
            let im = self.fft_im[k + 1];
            let mag = ((re * re + im * im) * INV_N).sqrt();
            self.fft_bins[k] = mag.clamp(0.0, u32::MAX as f64) as u32;
        }
    }

    /// Implementa `sub_46ed10` + `sub_46f630`: calcula fluxo espectral `|curr - prev|` e o desvio-padrão em 25 bandas de 5 bins.
    pub fn extract_bands_from_bins(&mut self) -> [u32; AUDIO_BAND_COUNT] {
        let mut bands = [0u32; AUDIO_BAND_COUNT];
        for (b, band_out) in bands.iter_mut().enumerate() {
            let start = b * 5;
            let mut diffs = [0u32; 5];
            let mut sum = 0u64;
            for (j, slot) in diffs.iter_mut().enumerate() {
                let d = self.fft_bins[start + j].abs_diff(self.prev_bins[start + j]);
                *slot = d;
                sum += d as u64;
            }
            let mean = sum / 5;
            let mut var_sum = 0u64;
            for d in diffs {
                let delta = d as i64 - mean as i64;
                var_sum += (delta * delta) as u64;
            }
            // Desvio-padrão amostral `(n - 1 = 4)`, exatamente como `sub_46f630`.
            *band_out = ((var_sum / 4) as f64).sqrt() as u32;
        }
        self.prev_bins.copy_from_slice(&self.fft_bins);
        bands
    }

    /// Renderiza as 25 bandas espectrais sobre o layout físico do AK820 (`sub_4966c0` + `sub_492ee0`).
    pub fn render_frame(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
    ) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
        let mut out = [RgbColor::default(); TOTAL_PHYSICAL_LEDS];
        let master_bri = BRIGHTNESS_LUT[(cfg.brightness as usize).min(4)] as u16;
        if master_bri == 0 {
            return out;
        }

        let speed_step = speed_increment(cfg.speed);
        self.color_idx = advance_wrap(self.color_idx, speed_step, 192);

        match cfg.audio_style {
            AudioStyle::VerticalBars => self.render_vertical_bars(bands, cfg, master_bri, &mut out),
            AudioStyle::RainbowPulse => self.render_rainbow_pulse(bands, cfg, master_bri, &mut out),
            AudioStyle::CenterWave => self.render_center_wave(bands, cfg, master_bri, &mut out),
            AudioStyle::ConcentricRings => {
                self.render_concentric_rings(bands, cfg, master_bri, &mut out)
            }
            AudioStyle::PeakDrop => self.render_peak_drop(bands, cfg, master_bri, &mut out),
            AudioStyle::HorizontalBars => {
                self.render_horizontal_bars(bands, cfg, master_bri, &mut out)
            }
            AudioStyle::ZigZagBars => self.render_zigzag_bars(bands, cfg, master_bri, &mut out),
        }

        out
    }

    fn decay_key(&mut self, light_id: u8) {
        let idx = light_id as usize;
        if idx >= AUDIO_CELL_EMPTY as usize {
            return;
        }
        let e = self.energy[idx];
        // Curva exata de decaimento de `sub_4b4c70` para `mode == 0x1d`.
        self.energy[idx] = if (e & 0x90) != 0 {
            e.saturating_sub(0x2D)
        } else if (e & 0x80) != 0 {
            e.saturating_sub(0x28)
        } else if (e & 0x70) != 0 {
            e.saturating_sub(0x23)
        } else if (e & 0x60) != 0 {
            e.saturating_sub(0x1E)
        } else if (e & 0x50) != 0 {
            e.saturating_sub(0x19)
        } else if (e & 0x40) != 0 {
            e.saturating_sub(0x14)
        } else if (e & 0x20) != 0 {
            e.saturating_sub(10)
        } else if e > 16 {
            e.saturating_sub(5)
        } else {
            e.saturating_sub(1)
        };
    }

    fn write_cell(
        &mut self,
        light_id: u8,
        active: bool,
        rainbow_idx: u8,
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        if light_id >= AUDIO_CELL_EMPTY {
            return;
        }
        let lid = light_id as usize;
        if active {
            self.energy[lid] = 0xFF;
        }
        let e = self.energy[lid];

        let (base_rgb, key_bri) = if e >= 10 {
            let rgb = if cfg.multicolor {
                AUDIO_RAINBOW_192[(rainbow_idx as usize) % 192]
            } else {
                cfg.color
            };
            (rgb, ((master_bri * e as u16) >> 8) as u8)
        } else if cfg.audio_bg_enabled {
            (cfg.audio_bg_color, master_bri as u8)
        } else {
            (RgbColor::default(), 0)
        };

        let phys = LIGHT_ID_TO_PHYS_LED[lid];
        if (phys as usize) < TOTAL_PHYSICAL_LEDS {
            let scale = key_bri as u16;
            out[phys as usize] = RgbColor::new(
                ((base_rgb.r as u16 * scale) >> 8) as u8,
                ((base_rgb.g as u16 * scale) >> 8) as u8,
                ((base_rgb.b as u16 * scale) >> 8) as u8,
            );
        }
    }

    fn render_vertical_bars(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        self.scroll_ticks += 1;
        if self.scroll_ticks > 30 {
            self.scroll_ticks = 0;
            self.band_offset = advance_wrap(self.band_offset, speed_increment(cfg.speed), 25);
        }

        let mut hue = self.color_idx;
        let mut band_idx = self.band_offset;

        for col in 0..AUDIO_GRID_COLS {
            band_idx = (band_idx + 1) % 25;
            hue = (hue + 10) % 192;
            let threshold = ((level_inv(bands[band_idx as usize]) + 100) * 6) / 600;
            for row in 0..AUDIO_GRID_ROWS {
                let lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + row];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                let active = row as u32 >= threshold;
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }

    fn render_rainbow_pulse(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        let avg: u32 = bands.iter().sum::<u32>() / (AUDIO_BAND_COUNT as u32);
        let beat = avg >= 280;
        let step = 7u8.saturating_add(cfg.speed.min(4) * 2);

        for lid in 0..(AUDIO_CELL_EMPTY as usize) {
            self.decay_key(lid as u8);
            self.key_hue[lid] = (self.key_hue[lid].wrapping_add(step)) % 192;
            let hue = self.key_hue[lid];
            self.write_cell(lid as u8, beat, hue, cfg, master_bri, out);
        }
    }

    fn render_center_wave(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        self.band_offset = advance_wrap(self.band_offset, speed_increment(cfg.speed), 25);
        let mut band_idx = self.band_offset;

        for row in 0..AUDIO_GRID_ROWS {
            band_idx = (band_idx + 3) % 25;
            let bar_right = (((level_inv(bands[band_idx as usize]) + 100) * 12) / 600) as i32;
            let bar_left = (((level_inv(bands[(row * 2).min(24)]) + 100) * 12) / 600) as i32;
            let mut hue = self.color_idx;

            for col in 0..AUDIO_GRID_COLS {
                hue = (hue + 7) % 192;
                let lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + row];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                let c = col as i32;
                let active = if c >= 9 {
                    (c - 9) <= (12 - bar_left)
                } else {
                    c >= bar_right
                };
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }

    fn render_concentric_rings(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        let avg_inv: u32 = bands.iter().map(|&b| level_inv(b)).sum::<u32>() / 25;
        let radius = (avg_inv * 20) / 600;
        let mut hue = self.color_idx;

        for ring in 0..12 {
            hue = (hue + 9) % 192;
            let active = (ring as u32) < 20u32.saturating_sub(radius);
            for slot in 0..12 {
                let lid = AUDIO_RING_12X12[ring * 12 + slot];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }

    fn render_peak_drop(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        self.scroll_ticks += 1;
        if self.scroll_ticks > 10 {
            self.scroll_ticks = 0;
            self.band_offset = advance_wrap(self.band_offset, 1, 25);
        }
        let mut hue = self.color_idx;
        let mut band_idx = self.band_offset;

        for col in 0..AUDIO_GRID_COLS {
            band_idx = (band_idx + 1) % 25;
            hue = (hue + 7) % 192;
            let threshold = ((level_inv(bands[band_idx as usize]) + 100) * 6) / 600;

            let bottom_lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + 5];
            if bottom_lid < AUDIO_CELL_EMPTY && self.peaks[col] && self.energy[bottom_lid as usize] < 10 {
                self.peaks[col] = false;
            }

            for row in 0..AUDIO_GRID_ROWS {
                let lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + row];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                let active = row as u32 >= threshold && !self.peaks[col];
                if active {
                    self.peaks[col] = true;
                }
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }

    fn render_horizontal_bars(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        const GROUPS: [usize; 6] = [2, 3, 3, 4, 5, 7];
        let mut row_levels = [0u32; 6];
        let mut cursor = 0;
        for (r, &len) in GROUPS.iter().enumerate() {
            let sum: u32 = bands[cursor..cursor + len].iter().sum();
            cursor += len;
            row_levels[r] = sum / 3;
        }

        for row in 0..AUDIO_GRID_ROWS {
            let threshold = 23 - (((level_inv(row_levels[5 - row]) + 100) * 23) / 600) as i32;
            let mut hue = self.color_idx;
            for col in 0..AUDIO_GRID_COLS {
                hue = (hue + 3) % 192;
                let lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + row];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                let active = (col as i32) <= threshold;
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }

    fn render_zigzag_bars(
        &mut self,
        bands: &[u32; AUDIO_BAND_COUNT],
        cfg: &LightingConfig,
        master_bri: u16,
        out: &mut [RgbColor; TOTAL_PHYSICAL_LEDS],
    ) {
        const GROUPS: [usize; 6] = [2, 2, 3, 4, 5, 5];
        let mut row_levels = [0u32; 6];
        let mut cursor = 0;
        for (r, &len) in GROUPS.iter().enumerate() {
            let sum: u32 = bands[cursor..cursor + len].iter().sum();
            cursor += len;
            row_levels[r] = sum / 3;
        }

        self.band_offset = advance_wrap(self.band_offset, speed_increment(cfg.speed), 25);

        for row in 0..AUDIO_GRID_ROWS {
            let bar_len = ((level_inv(row_levels[row]) * 23) / 600) as i32;
            let mut hue = self.color_idx;
            for col in 0..AUDIO_GRID_COLS {
                hue = (hue + 2) % 192;
                let lid = AUDIO_GRID_23X6[col * AUDIO_GRID_ROWS + row];
                if lid >= AUDIO_CELL_EMPTY {
                    continue;
                }
                self.decay_key(lid);
                let c = col as i32;
                let active = if row % 2 == 1 {
                    c < (23 - bar_len)
                } else {
                    c >= bar_len
                };
                self.write_cell(lid, active, hue, cfg, master_bri, out);
            }
        }
    }
}

#[inline]
fn level_inv(level: u32) -> u32 {
    let clamped = level.min(2000);
    600 - (clamped * 600) / 2000
}

#[inline]
const fn speed_increment(speed: u8) -> u8 {
    match speed {
        0 => 1,
        1 => 2,
        2 => 3,
        3 => 4,
        _ => 6,
    }
}

#[inline]
const fn advance_wrap(val: u8, delta: u8, modulo: u8) -> u8 {
    ((val as u16 + delta as u16) % (modulo as u16)) as u8
}

/// FFT radix-2 decimation-in-time sobre `N = 1024` (sem alocações de heap).
fn radix2_fft_1024(re: &mut [f64; AUDIO_FFT_SIZE], im: &mut [f64; AUDIO_FFT_SIZE]) {
    let n = AUDIO_FFT_SIZE;
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * PI / (len as f64);
        let wlen_re = ang.cos();
        let wlen_im = ang.sin();
        let half = len / 2;
        for start in (0..n).step_by(len) {
            let mut w_re = 1.0f64;
            let mut w_im = 0.0f64;
            for offset in 0..half {
                let u = start + offset;
                let v = u + half;
                let tr = w_re * re[v] - w_im * im[v];
                let ti = w_re * im[v] + w_im * re[v];
                re[v] = re[u] - tr;
                im[v] = im[u] - ti;
                re[u] += tr;
                im[u] += ti;
                let next_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = next_re;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LightingMode, AK820_KEY_LAYOUT};

    #[test]
    fn light_id_map_matches_known_ak820_keys() {
        for key_name in ["Esc", "W", "Space", "Left"] {
            let key = AK820_KEY_LAYOUT
                .iter()
                .find(|k| k.name == key_name)
                .expect("expected AK820 key in layout");
            assert_eq!(
                usize::from(LIGHT_ID_TO_PHYS_LED[key.light_id]),
                key.phys_led,
                "LIGHT_ID_TO_PHYS_LED mismatch for {key_name}"
            );
            assert!(
                key.phys_led < TOTAL_PHYSICAL_LEDS,
                "phys_led out of bounds for {key_name}"
            );
        }
    }

    #[test]
    fn silence_renders_all_off_without_background() {
        let cfg = LightingConfig {
            mode: LightingMode::Audio,
            brightness: 4,
            speed: 2,
            audio_bg_enabled: false,
            ..LightingConfig::default()
        };
        let silence = [0.0f32; AUDIO_FFT_SIZE];
        for &style in &AudioStyle::ALL {
            let mut viz = AudioVisualizer::new();
            let mut style_cfg = cfg;
            style_cfg.audio_style = style;
            let frame = viz.process_pcm_frame(&silence, &style_cfg);
            assert!(
                frame.iter().all(|c| c.r == 0 && c.g == 0 && c.b == 0),
                "expected all LEDs off on silence for {:?}",
                style
            );
        }
    }

    #[test]
    fn sine_wave_peaks_expected_bin_and_lights_all_styles() {
        let mut samples = [0.0f32; AUDIO_FFT_SIZE];
        let target_bin = 10usize;
        for (i, s) in samples.iter_mut().enumerate() {
            let phase = 2.0 * PI * (target_bin as f64) * (i as f64) / (AUDIO_FFT_SIZE as f64);
            *s = phase.sin() as f32;
        }

        let mut viz = AudioVisualizer::new();
        viz.compute_fft_bins(&samples);
        let peak_bin = viz
            .fft_bins
            .iter()
            .enumerate()
            .max_by_key(|&(_, v)| v)
            .map(|(idx, _)| idx)
            .unwrap();
        // `fft_bins` pula DC (bin 0), portanto a frequência `target_bin` fica no índice `target_bin - 1`.
        assert_eq!(peak_bin, target_bin - 1);

        // Excita múltiplas bandas do espectro (como áudio real) para cobrir colunas reais do AK820 em todos os estilos.
        let mut chord = [0.0f32; AUDIO_FFT_SIZE];
        for (i, s) in chord.iter_mut().enumerate() {
            let t = 2.0 * PI * (i as f64) / (AUDIO_FFT_SIZE as f64);
            let sum = (6.0 * t).sin()
                + (16.0 * t).sin()
                + (28.0 * t).sin()
                + (45.0 * t).sin()
                + (70.0 * t).sin()
                + (95.0 * t).sin();
            *s = (sum / 6.0) as f32;
        }

        for &style in &AudioStyle::ALL {
            let mut style_viz = AudioVisualizer::new();
            let cfg = LightingConfig {
                mode: LightingMode::Audio,
                brightness: 4,
                speed: 3,
                multicolor: true,
                color: RgbColor::new(255, 64, 32),
                audio_style: style,
                audio_bg_enabled: false,
                ..LightingConfig::default()
            };
            // Envia silêncio antes do transiente para excitar `|curr - prev|` nas bandas.
            let _ = style_viz.process_pcm_frame(&[0.0f32; AUDIO_FFT_SIZE], &cfg);
            let frame = style_viz.process_pcm_frame(&chord, &cfg);
            let lit = frame.iter().filter(|c| c.r > 0 || c.g > 0 || c.b > 0).count();
            assert!(lit > 0, "expected active LEDs for audio style {:?}", style);
        }
    }

    #[test]
    fn audio_mode_wire_id_and_defaults() {
        assert_eq!(LightingMode::Audio.wire_id(), 0xFE);
        assert_eq!(LightingMode::from_wire(0x1D), LightingMode::Audio);
        assert!(LightingMode::ALL.contains(&LightingMode::Audio));
        let cfg = LightingConfig::default();
        assert_eq!(cfg.audio_style, AudioStyle::VerticalBars);
        assert!(!cfg.audio_bg_enabled);
    }
}
