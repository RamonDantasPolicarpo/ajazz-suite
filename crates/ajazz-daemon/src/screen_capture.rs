use ajazz_protocol::{project_image_to_matrix, RgbColor, TOTAL_PHYSICAL_LEDS};
use std::ffi::c_void;
use std::ptr::null_mut;

const CAPTURE_W: usize = 32;
const CAPTURE_H: usize = 12;

#[cfg(windows)]
mod win32 {
    use super::c_void;

    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;
    pub const SRCCOPY: u32 = 0x00CC_0020;
    pub const HALFTONE: i32 = 4;
    pub const BI_RGB: u32 = 0;
    pub const DIB_RGB_COLORS: u32 = 0;

    #[repr(C)]
    pub struct BitmapInfoHeader {
        pub bi_size: u32,
        pub bi_width: i32,
        pub bi_height: i32,
        pub bi_planes: u16,
        pub bi_bit_count: u16,
        pub bi_compression: u32,
        pub bi_size_image: u32,
        pub bi_x_pels_per_meter: i32,
        pub bi_y_pels_per_meter: i32,
        pub bi_clr_used: u32,
        pub bi_clr_important: u32,
    }

    #[repr(C)]
    pub struct BitmapInfo {
        pub bmi_header: BitmapInfoHeader,
        pub bmi_colors: [u32; 1],
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetSystemMetrics(n_index: i32) -> i32;
        pub fn GetDC(h_wnd: *mut c_void) -> *mut c_void;
        pub fn ReleaseDC(h_wnd: *mut c_void, h_dc: *mut c_void) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
        pub fn DeleteDC(hdc: *mut c_void) -> i32;
        pub fn CreateCompatibleBitmap(hdc: *mut c_void, cx: i32, cy: i32) -> *mut c_void;
        pub fn SelectObject(hdc: *mut c_void, h: *mut c_void) -> *mut c_void;
        pub fn DeleteObject(ho: *mut c_void) -> i32;
        pub fn SetStretchBltMode(hdc: *mut c_void, mode: i32) -> i32;
        pub fn StretchBlt(
            hdc_dest: *mut c_void,
            x_dest: i32,
            y_dest: i32,
            w_dest: i32,
            h_dest: i32,
            hdc_src: *mut c_void,
            x_src: i32,
            y_src: i32,
            w_src: i32,
            h_src: i32,
            rop: u32,
        ) -> i32;
        pub fn GetDIBits(
            hdc: *mut c_void,
            hbm: *mut c_void,
            start: u32,
            c_lines: u32,
            lpv_bits: *mut u8,
            lpbmi: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
    }
}

/// Capturador leve de tela via Win32 GDI com buffer fixo (zero alocações no loop quente).
pub struct ScreenCapturer {
    bgra_buf: [u8; CAPTURE_W * CAPTURE_H * 4],
    rgb_buf: [u8; CAPTURE_W * CAPTURE_H * 3],
}

impl ScreenCapturer {
    pub fn new() -> Self {
        Self {
            bgra_buf: [0u8; CAPTURE_W * CAPTURE_H * 4],
            rgb_buf: [0u8; CAPTURE_W * CAPTURE_H * 3],
        }
    }

    /// Captura o monitor primário reduzido para `32x12` e projeta na matriz de LEDs do AK820.
    #[cfg(windows)]
    pub fn capture_matrix(&mut self) -> Option<[RgbColor; TOTAL_PHYSICAL_LEDS]> {
        use win32::*;

        unsafe {
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            if screen_w <= 0 || screen_h <= 0 {
                return None;
            }

            let screen_dc = GetDC(null_mut());
            if screen_dc.is_null() {
                return None;
            }

            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return None;
            }

            let bitmap = CreateCompatibleBitmap(screen_dc, CAPTURE_W as i32, CAPTURE_H as i32);
            if bitmap.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return None;
            }

            let old_obj = SelectObject(mem_dc, bitmap);
            SetStretchBltMode(mem_dc, HALFTONE);
            let blt_ok = StretchBlt(
                mem_dc,
                0,
                0,
                CAPTURE_W as i32,
                CAPTURE_H as i32,
                screen_dc,
                0,
                0,
                screen_w,
                screen_h,
                SRCCOPY,
            );
            SelectObject(mem_dc, old_obj);

            let mut bmi = BitmapInfo {
                bmi_header: BitmapInfoHeader {
                    bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                    bi_width: CAPTURE_W as i32,
                    bi_height: -(CAPTURE_H as i32), // Top-down para casar norm_y=0 no topo
                    bi_planes: 1,
                    bi_bit_count: 32,
                    bi_compression: BI_RGB,
                    bi_size_image: 0,
                    bi_x_pels_per_meter: 0,
                    bi_y_pels_per_meter: 0,
                    bi_clr_used: 0,
                    bi_clr_important: 0,
                },
                bmi_colors: [0],
            };

            let scanlines = if blt_ok != 0 {
                GetDIBits(
                    mem_dc,
                    bitmap,
                    0,
                    CAPTURE_H as u32,
                    self.bgra_buf.as_mut_ptr(),
                    &mut bmi,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };

            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);

            if scanlines <= 0 {
                return None;
            }
        }

        for (px_idx, chunk) in self.bgra_buf.chunks_exact(4).enumerate() {
            let dst = px_idx * 3;
            self.rgb_buf[dst] = chunk[2]; // R
            self.rgb_buf[dst + 1] = chunk[1]; // G
            self.rgb_buf[dst + 2] = chunk[0]; // B
        }

        Some(project_image_to_matrix(CAPTURE_W, CAPTURE_H, &self.rgb_buf))
    }

    #[cfg(not(windows))]
    pub fn capture_matrix(&mut self) -> Option<[RgbColor; TOTAL_PHYSICAL_LEDS]> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_capturer_smoke_never_panics() {
        let mut cap = ScreenCapturer::new();
        if let Some(matrix) = cap.capture_matrix() {
            assert_eq!(matrix.len(), TOTAL_PHYSICAL_LEDS);
        }
    }
}
