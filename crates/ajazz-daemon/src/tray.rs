use std::sync::atomic::{AtomicBool, AtomicI16, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

/// Estado leve compartilhado entre o Device Actor e o menu da bandeja (zero alocações/locks pesados).
#[derive(Default)]
pub struct TrayStatus {
    pub kb_connected: AtomicBool,
    pub mouse_connected: AtomicBool,
    /// `-1` se indisponível, `0..=100` quando online ou em cache.
    pub mouse_battery: AtomicI16,
}

impl TrayStatus {
    pub fn new() -> Self {
        Self {
            kb_connected: AtomicBool::new(false),
            mouse_connected: AtomicBool::new(false),
            mouse_battery: AtomicI16::new(-1),
        }
    }

    pub fn set_keyboard(&self, connected: bool) {
        self.kb_connected.store(connected, Ordering::Relaxed);
    }

    pub fn set_mouse(&self, connected: bool, battery: Option<u8>) {
        self.mouse_connected.store(connected, Ordering::Relaxed);
        self.mouse_battery
            .store(battery.map(|b| i16::from(b.min(100))).unwrap_or(-1), Ordering::Relaxed);
    }

    pub fn summary_text(&self) -> String {
        let kb = if self.kb_connected.load(Ordering::Relaxed) {
            "AK820: OK"
        } else {
            "AK820: Offline"
        };
        let ms = if self.mouse_connected.load(Ordering::Relaxed) {
            let bat = self.mouse_battery.load(Ordering::Relaxed);
            if bat >= 0 {
                format!("AJ179: {bat}%")
            } else {
                "AJ179: OK".to_string()
            }
        } else {
            "AJ179: Offline".to_string()
        };
        format!("{kb}  |  {ms}")
    }
}

pub fn launch_gui_process() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Falha ao localizar daemon: {e}"))?;
    let gui_exe = exe.with_file_name(if cfg!(windows) {
        "ajazz-gui.exe"
    } else {
        "ajazz-gui"
    });
    if !gui_exe.exists() {
        return Err(format!("Executável da GUI não encontrado em {}", gui_exe.display()));
    }
    std::process::Command::new(&gui_exe)
        .spawn()
        .map_err(|e| format!("Falha ao abrir ajazz-gui: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn is_autostart_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_autostart_enabled(_enabled: bool) -> Result<(), String> {
    Err("Inicialização automática disponível apenas no Windows.".into())
}

#[cfg(not(windows))]
pub fn spawn_tray(_status: Arc<TrayStatus>) {}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_APP: u32 = 0x8000;
    pub const WM_TRAY: u32 = WM_APP + 1;
    pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
    pub const WM_RBUTTONUP: u32 = 0x0205;

    pub const NIM_ADD: u32 = 0x0000_0000;
    pub const NIM_DELETE: u32 = 0x0000_0002;
    pub const NIF_MESSAGE: u32 = 0x0000_0001;
    pub const NIF_ICON: u32 = 0x0000_0002;
    pub const NIF_TIP: u32 = 0x0000_0004;

    pub const MF_STRING: u32 = 0x0000_0000;
    pub const MF_GRAYED: u32 = 0x0000_0001;
    pub const MF_CHECKED: u32 = 0x0000_0008;
    pub const MF_SEPARATOR: u32 = 0x0000_0800;

    pub const TPM_RIGHTALIGN: u32 = 0x0008;
    pub const TPM_BOTTOMALIGN: u32 = 0x0020;

    pub const GWLP_USERDATA: i32 = -21;

    pub const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize; // 0x80000001
    pub const KEY_READ: u32 = 0x0002_0019;
    pub const KEY_SET_VALUE: u32 = 0x0002;
    pub const REG_SZ: u32 = 1;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Guid {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    #[repr(C)]
    pub struct NotifyIconDataW {
        pub cb_size: u32,
        pub h_wnd: *mut c_void,
        pub u_id: u32,
        pub u_flags: u32,
        pub u_callback_message: u32,
        pub h_icon: *mut c_void,
        pub sz_tip: [u16; 128],
        pub dw_state: u32,
        pub dw_state_mask: u32,
        pub sz_info: [u16; 256],
        pub u_timeout_or_version: u32,
        pub sz_info_title: [u16; 64],
        pub dw_info_flags: u32,
        pub guid_item: Guid,
        pub h_balloon_icon: *mut c_void,
    }

    #[repr(C)]
    pub struct WndClassW {
        pub style: u32,
        pub lpfn_wnd_proc: unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize,
        pub cb_cls_extra: i32,
        pub cb_wnd_extra: i32,
        pub h_instance: *mut c_void,
        pub h_icon: *mut c_void,
        pub h_cursor: *mut c_void,
        pub hbr_background: *mut c_void,
        pub lpsz_menu_name: *const u16,
        pub lpsz_class_name: *const u16,
    }

    #[repr(C)]
    pub struct IconInfo {
        pub f_icon: i32,
        pub x_hotspot: u32,
        pub y_hotspot: u32,
        pub hbm_mask: *mut c_void,
        pub hbm_color: *mut c_void,
    }

    #[repr(C)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[repr(C)]
    pub struct Msg {
        pub hwnd: *mut c_void,
        pub message: u32,
        pub w_param: usize,
        pub l_param: isize,
        pub time: u32,
        pub pt: Point,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleW(lp_module_name: *const u16) -> *mut c_void;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateBitmap(
            n_width: i32,
            n_height: i32,
            n_planes: u32,
            n_bit_count: u32,
            lp_bits: *const c_void,
        ) -> *mut c_void;
        pub fn DeleteObject(ho: *mut c_void) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn RegisterClassW(lp_wnd_class: *const WndClassW) -> u16;
        pub fn RegisterWindowMessageW(lp_string: *const u16) -> u32;
        pub fn CreateWindowExW(
            dw_ex_style: u32,
            lp_class_name: *const u16,
            lp_window_name: *const u16,
            dw_style: u32,
            x: i32,
            y: i32,
            n_width: i32,
            n_height: i32,
            h_wnd_parent: *mut c_void,
            h_menu: *mut c_void,
            h_instance: *mut c_void,
            lp_param: *mut c_void,
        ) -> *mut c_void;
        pub fn DefWindowProcW(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize;
        pub fn DestroyWindow(h_wnd: *mut c_void) -> i32;
        pub fn SetWindowLongPtrW(h_wnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
        pub fn GetWindowLongPtrW(h_wnd: *mut c_void, n_index: i32) -> isize;
        pub fn PostQuitMessage(n_exit_code: i32);
        pub fn GetMessageW(lp_msg: *mut Msg, h_wnd: *mut c_void, min: u32, max: u32) -> i32;
        pub fn TranslateMessage(lp_msg: *const Msg) -> i32;
        pub fn DispatchMessageW(lp_msg: *const Msg) -> isize;
        pub fn CreatePopupMenu() -> *mut c_void;
        pub fn AppendMenuW(
            h_menu: *mut c_void,
            u_flags: u32,
            u_id_new_item: usize,
            lp_new_item: *const u16,
        ) -> i32;
        pub fn TrackPopupMenu(
            h_menu: *mut c_void,
            u_flags: u32,
            x: i32,
            y: i32,
            n_reserved: i32,
            h_wnd: *mut c_void,
            prc_rect: *const c_void,
        ) -> i32;
        pub fn DestroyMenu(h_menu: *mut c_void) -> i32;
        pub fn SetForegroundWindow(h_wnd: *mut c_void) -> i32;
        pub fn GetCursorPos(lp_point: *mut Point) -> i32;
        pub fn CreateIconIndirect(piconinfo: *const IconInfo) -> *mut c_void;
        pub fn DestroyIcon(h_icon: *mut c_void) -> i32;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        pub fn Shell_NotifyIconW(dw_message: u32, lp_data: *mut NotifyIconDataW) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn RegOpenKeyExW(
            h_key: isize,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut isize,
        ) -> i32;
        pub fn RegCreateKeyExW(
            h_key: isize,
            lp_sub_key: *const u16,
            reserved: u32,
            lp_class: *mut u16,
            dw_options: u32,
            sam_desired: u32,
            lp_security_attributes: *const c_void,
            phk_result: *mut isize,
            lpdw_disposition: *mut u32,
        ) -> i32;
        pub fn RegQueryValueExW(
            h_key: isize,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> i32;
        pub fn RegSetValueExW(
            h_key: isize,
            lp_value_name: *const u16,
            reserved: u32,
            dw_type: u32,
            lp_data: *const u8,
            cb_data: u32,
        ) -> i32;
        pub fn RegDeleteValueW(h_key: isize, lp_value_name: *const u16) -> i32;
        pub fn RegCloseKey(h_key: isize) -> i32;
    }
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "AjazzDaemon";

#[cfg(windows)]
pub fn is_autostart_enabled() -> bool {
    let subkey = to_wide(RUN_SUBKEY);
    let val = to_wide(RUN_VALUE);
    let mut hkey: isize = 0;
    let res = unsafe {
        win32::RegOpenKeyExW(
            win32::HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            win32::KEY_READ,
            &mut hkey,
        )
    };
    if res != 0 {
        return false;
    }
    let mut size: u32 = 0;
    let q = unsafe {
        win32::RegQueryValueExW(
            hkey,
            val.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    unsafe {
        win32::RegCloseKey(hkey);
    }
    q == 0 && size > 2
}

#[cfg(windows)]
pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    let subkey = to_wide(RUN_SUBKEY);
    let val = to_wide(RUN_VALUE);
    let mut hkey: isize = 0;
    let res = unsafe {
        win32::RegCreateKeyExW(
            win32::HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            win32::KEY_SET_VALUE,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        )
    };
    if res != 0 {
        return Err(format!("Falha ao abrir Registro do Windows (código {res})"));
    }

    let op_res = if enabled {
        let exe = std::env::current_exe().map_err(|e| {
            unsafe {
                win32::RegCloseKey(hkey);
            }
            format!("Falha ao obter caminho do executável: {e}")
        })?;
        let quoted = to_wide(&format!("\"{}\"", exe.display()));
        let byte_len = (quoted.len() * 2) as u32;
        unsafe {
            win32::RegSetValueExW(
                hkey,
                val.as_ptr(),
                0,
                win32::REG_SZ,
                quoted.as_ptr().cast::<u8>(),
                byte_len,
            )
        }
    } else {
        let r = unsafe { win32::RegDeleteValueW(hkey, val.as_ptr()) };
        if r == 2 { 0 } else { r } // ERROR_FILE_NOT_FOUND == já removido
    };

    unsafe {
        win32::RegCloseKey(hkey);
    }
    if op_res != 0 {
        return Err(format!(
            "Falha ao atualizar inicialização no Windows (código {op_res})"
        ));
    }
    Ok(())
}

#[cfg(windows)]
struct TrayCtx {
    status: Arc<TrayStatus>,
    taskbar_created_msg: u32,
    icon_handle: *mut std::ffi::c_void,
}

#[cfg(windows)]
const CMD_OPEN_GUI: usize = 1001;
#[cfg(windows)]
const CMD_TOGGLE_AUTOSTART: usize = 1002;
#[cfg(windows)]
const CMD_EXIT: usize = 1003;

/// Verifica se o ponto `(x, y)` está dentro do retângulo arredondado de centro `(cx, cy)`.
fn rounded_rect_sdf(x: f32, y: f32, cx: f32, cy: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let qx = (x - cx).abs() - (half_w - radius);
    let qy = (y - cy).abs() - (half_h - radius);
    let out = qx.max(0.0).hypot(qy.max(0.0));
    let inside = qx.max(qy).min(0.0);
    out + inside - radius
}

/// Testa se o ponto `(x, y)` está dentro de um polígono convexo/simples pelo método ray-casting.
fn point_in_poly(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Rasteriza a geometria vetorial de `crates/ajazz-gui/ui/assets/logo.svg` em 32×32 ARGB pré-multiplicado.
fn render_logo_32x32_bgra() -> [u32; 32 * 32] {
    const SIZE: usize = 32;
    const SUB: usize = 4;
    const SAMPLES: f32 = (SUB * SUB) as f32;
    const LEG_LEFT: [(f32, f32); 4] = [(8.5, 23.5), (14.4, 8.5), (17.8, 8.5), (11.9, 23.5)];
    const LEG_RIGHT: [(f32, f32); 4] = [(18.3, 12.6), (23.5, 23.5), (20.0, 23.5), (16.3, 15.6)];

    let mut out = [0u32; SIZE * SIZE];
    for py in 0..SIZE {
        for px in 0..SIZE {
            let (mut r_acc, mut g_acc, mut b_acc, mut a_acc) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let x = px as f32 + (sx as f32 + 0.5) / SUB as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SUB as f32;

                    // Outer rounded rect: x=1..31, y=1..31, rx=7, stroke-width=1.5
                    let sd_outer = rounded_rect_sdf(x, y, 16.0, 16.0, 15.0, 15.0, 7.0);
                    if sd_outer > 0.75 {
                        continue;
                    }

                    // Ordem de pintura do logo.svg: fill -> stroke -> A paths -> square verde
                    let (r, g, b) = if sd_outer >= -0.75 {
                        (0x3F as f32, 0x3F as f32, 0x46 as f32) // #3F3F46 border
                    } else if rounded_rect_sdf(x, y, 16.0, 21.0, 1.7, 1.7, 0.9) <= 0.0 {
                        (0x22 as f32, 0xC5 as f32, 0x5E as f32) // #22C55E green accent
                    } else if point_in_poly(x, y, &LEG_LEFT) || point_in_poly(x, y, &LEG_RIGHT) {
                        (0xF4 as f32, 0xF4 as f32, 0xF5 as f32) // #F4F4F5 monogram
                    } else {
                        (0x18 as f32, 0x18 as f32, 0x1B as f32) // #18181B graphite bg
                    };

                    r_acc += r;
                    g_acc += g;
                    b_acc += b;
                    a_acc += 255.0;
                }
            }

            // Média já fica naturalmente pré-multiplicada pelos pixels opacos cobertos
            let a = (a_acc / SAMPLES).round().clamp(0.0, 255.0) as u32;
            let r = (r_acc / SAMPLES).round().clamp(0.0, 255.0) as u32;
            let g = (g_acc / SAMPLES).round().clamp(0.0, 255.0) as u32;
            let b = (b_acc / SAMPLES).round().clamp(0.0, 255.0) as u32;
            out[py * SIZE + px] = (a << 24) | (r << 16) | (g << 8) | b;
        }
    }
    out
}

/// Gera em memória o ícone 32×32 idêntico ao `logo.svg` da janela (fundo `#18181B`, borda `#3F3F46`, monograma `#F4F4F5` e ponto `#22C55E`).
#[cfg(windows)]
fn create_tray_icon() -> *mut std::ffi::c_void {
    use std::ffi::c_void;
    let bgra = render_logo_32x32_bgra();
    let mask = [0u32; 32]; // Bitmask 0 para usar o canal alpha da superfície 32bpp
    unsafe {
        let hbm_color = win32::CreateBitmap(32, 32, 1, 32, bgra.as_ptr().cast::<c_void>());
        let hbm_mask = win32::CreateBitmap(32, 32, 1, 1, mask.as_ptr().cast::<c_void>());
        let info = win32::IconInfo {
            f_icon: 1,
            x_hotspot: 0,
            y_hotspot: 0,
            hbm_mask,
            hbm_color,
        };
        let icon = win32::CreateIconIndirect(&info);
        if !hbm_color.is_null() {
            win32::DeleteObject(hbm_color);
        }
        if !hbm_mask.is_null() {
            win32::DeleteObject(hbm_mask);
        }
        icon
    }
}

#[cfg(windows)]
fn register_tray_icon(hwnd: *mut std::ffi::c_void, icon: *mut std::ffi::c_void) {
    use std::mem::zeroed;
    let mut nid: win32::NotifyIconDataW = unsafe { zeroed() };
    nid.cb_size = std::mem::size_of::<win32::NotifyIconDataW>() as u32;
    nid.h_wnd = hwnd;
    nid.u_id = 1;
    nid.u_flags = win32::NIF_MESSAGE | win32::NIF_ICON | win32::NIF_TIP;
    nid.u_callback_message = win32::WM_TRAY;
    nid.h_icon = icon;
    let tip = "AJAZZ Companion Daemon";
    for (i, ch) in tip.encode_utf16().take(127).enumerate() {
        nid.sz_tip[i] = ch;
    }
    unsafe {
        win32::Shell_NotifyIconW(win32::NIM_ADD, &mut nid);
    }
}

#[cfg(windows)]
fn unregister_tray_icon(hwnd: *mut std::ffi::c_void) {
    use std::mem::zeroed;
    let mut nid: win32::NotifyIconDataW = unsafe { zeroed() };
    nid.cb_size = std::mem::size_of::<win32::NotifyIconDataW>() as u32;
    nid.h_wnd = hwnd;
    nid.u_id = 1;
    unsafe {
        win32::Shell_NotifyIconW(win32::NIM_DELETE, &mut nid);
    }
}

#[cfg(windows)]
fn show_context_menu(hwnd: *mut std::ffi::c_void, status: &TrayStatus) {
    let menu = unsafe { win32::CreatePopupMenu() };
    if menu.is_null() {
        return;
    }
    let summary = to_wide(&status.summary_text());
    let open_lbl = to_wide("Abrir AJAZZ Suite");
    let auto_lbl = to_wide("Iniciar com o Windows");
    let exit_lbl = to_wide("Sair do Daemon");
    let auto_flags = if is_autostart_enabled() {
        win32::MF_STRING | win32::MF_CHECKED
    } else {
        win32::MF_STRING
    };

    unsafe {
        win32::AppendMenuW(menu, win32::MF_STRING | win32::MF_GRAYED, 0, summary.as_ptr());
        win32::AppendMenuW(menu, win32::MF_SEPARATOR, 0, std::ptr::null());
        win32::AppendMenuW(menu, win32::MF_STRING, CMD_OPEN_GUI, open_lbl.as_ptr());
        win32::AppendMenuW(menu, auto_flags, CMD_TOGGLE_AUTOSTART, auto_lbl.as_ptr());
        win32::AppendMenuW(menu, win32::MF_SEPARATOR, 0, std::ptr::null());
        win32::AppendMenuW(menu, win32::MF_STRING, CMD_EXIT, exit_lbl.as_ptr());

        let mut pt = win32::Point { x: 0, y: 0 };
        win32::GetCursorPos(&mut pt);
        win32::SetForegroundWindow(hwnd);
        win32::TrackPopupMenu(
            menu,
            win32::TPM_RIGHTALIGN | win32::TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        win32::DestroyMenu(menu);
    }
}

#[cfg(windows)]
unsafe extern "system" fn tray_wnd_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    let ctx_ptr = unsafe { win32::GetWindowLongPtrW(hwnd, win32::GWLP_USERDATA) } as *mut TrayCtx;
    if !ctx_ptr.is_null() {
        let ctx = unsafe { &*ctx_ptr };
        if ctx.taskbar_created_msg != 0 && msg == ctx.taskbar_created_msg {
            register_tray_icon(hwnd, ctx.icon_handle);
            return 0;
        }
        if msg == win32::WM_TRAY {
            let ev = (l_param as u32) & 0xFFFF;
            if ev == win32::WM_LBUTTONDBLCLK {
                if let Err(e) = launch_gui_process() {
                    warn!("{e}");
                }
            } else if ev == win32::WM_RBUTTONUP {
                show_context_menu(hwnd, &ctx.status);
            }
            return 0;
        }
        if msg == win32::WM_COMMAND {
            let cmd = w_param & 0xFFFF;
            match cmd {
                CMD_OPEN_GUI => {
                    if let Err(e) = launch_gui_process() {
                        warn!("{e}");
                    }
                }
                CMD_TOGGLE_AUTOSTART => {
                    let next = !is_autostart_enabled();
                    if let Err(e) = set_autostart_enabled(next) {
                        warn!("{e}");
                    }
                }
                CMD_EXIT => {
                    unregister_tray_icon(hwnd);
                    unsafe { win32::DestroyWindow(hwnd) };
                    return 0;
                }
                _ => {}
            }
        }
    }

    if msg == win32::WM_DESTROY {
        unregister_tray_icon(hwnd);
        unsafe { win32::PostQuitMessage(0) };
        return 0;
    }

    unsafe { win32::DefWindowProcW(hwnd, msg, w_param, l_param) }
}

#[cfg(windows)]
pub fn spawn_tray(status: Arc<TrayStatus>) {
    let _ = std::thread::Builder::new()
        .name("ajazz-tray".into())
        .spawn(move || run_tray_loop(status));
}

#[cfg(windows)]
fn run_tray_loop(status: Arc<TrayStatus>) {
    use std::mem::zeroed;
    use std::ptr::null_mut;

    let class_name = to_wide("AjazzDaemonTrayClass");
    let win_title = to_wide("AJAZZ Companion Daemon");
    let h_inst = unsafe { win32::GetModuleHandleW(std::ptr::null()) };

    let wc = win32::WndClassW {
        style: 0,
        lpfn_wnd_proc: tray_wnd_proc,
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: h_inst,
        h_icon: null_mut(),
        h_cursor: null_mut(),
        hbr_background: null_mut(),
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class_name.as_ptr(),
    };
    unsafe {
        win32::RegisterClassW(&wc);
    }

    let hwnd = unsafe {
        win32::CreateWindowExW(
            0,
            class_name.as_ptr(),
            win_title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            h_inst,
            null_mut(),
        )
    };
    if hwnd.is_null() {
        warn!("Falha ao criar janela da bandeja do sistema.");
        return;
    }

    let icon = create_tray_icon();
    let taskbar_msg_name = to_wide("TaskbarCreated");
    let taskbar_created_msg = unsafe { win32::RegisterWindowMessageW(taskbar_msg_name.as_ptr()) };

    let ctx = Box::new(TrayCtx {
        status,
        taskbar_created_msg,
        icon_handle: icon,
    });
    let ctx_ptr = Box::into_raw(ctx);
    unsafe {
        win32::SetWindowLongPtrW(hwnd, win32::GWLP_USERDATA, ctx_ptr as isize);
    }

    register_tray_icon(hwnd, icon);
    info!("Ícone do Daemon registrado na bandeja do sistema.");

    let mut msg: win32::Msg = unsafe { zeroed() };
    while unsafe { win32::GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
        unsafe {
            win32::TranslateMessage(&msg);
            win32::DispatchMessageW(&msg);
        }
    }

    unsafe {
        win32::SetWindowLongPtrW(hwnd, win32::GWLP_USERDATA, 0);
        drop(Box::from_raw(ctx_ptr));
        if !icon.is_null() {
            win32::DestroyIcon(icon);
        }
    }
    info!("Saindo do AJAZZ Daemon pela bandeja do sistema.");
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_status_formats_battery_and_offline_cleanly() {
        let st = TrayStatus::new();
        assert_eq!(st.summary_text(), "AK820: Offline  |  AJ179: Offline");
        st.set_keyboard(true);
        st.set_mouse(true, Some(70));
        assert_eq!(st.summary_text(), "AK820: OK  |  AJ179: 70%");
    }

    #[test]
    fn tray_logo_rasterizer_matches_svg_palette() {
        let px = render_logo_32x32_bgra();
        // Canto externo (0, 0) deve ser transparente
        assert_eq!(px[0], 0);
        // Centro do quadrado verde (16, 21) -> #22C55E
        assert_eq!(px[21 * 32 + 16], 0xFF22_C55E);
        // Interior da perna esquerda (13, 16) -> #F4F4F5
        assert_eq!(px[16 * 32 + 13], 0xFFF4_F4F5);
        // Interior do fundo grafite (6, 6) -> #18181B
        assert_eq!(px[6 * 32 + 6], 0xFF18_181B);
    }

    #[cfg(all(windows, target_pointer_width = "64"))]
    #[test]
    fn win32_tray_structs_match_x64_abi() {
        assert_eq!(std::mem::size_of::<win32::NotifyIconDataW>(), 976);
        assert_eq!(std::mem::size_of::<win32::WndClassW>(), 72);
    }
}
