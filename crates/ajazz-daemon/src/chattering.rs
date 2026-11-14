use ajazz_protocol::mouse::{
    ChatteringConfig, SwitchChatteringButtonStats, SwitchChatteringSnapshot, MOUSE_BUTTON_COUNT,
};
use ajazz_protocol::MouseButton;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{info, warn};

/// Máquina de estados pura (zero alocações no hot path) para detectar **Switch Chattering**
/// nos 5 switches físicos do mouse.
#[derive(Debug, Clone)]
pub struct ChatteringDetector {
    config: ChatteringConfig,
    is_down: [bool; MOUSE_BUTTON_COUNT],
    last_down_at: [Option<Instant>; MOUSE_BUTTON_COUNT],
    last_up_at: [Option<Instant>; MOUSE_BUTTON_COUNT],
    stats: [SwitchChatteringButtonStats; MOUSE_BUTTON_COUNT],
    total_chatter_events: u64,
    last_chatter_button: Option<MouseButton>,
    last_chatter_interval_ms: Option<u32>,
}

impl ChatteringDetector {
    pub fn new(mut config: ChatteringConfig) -> Self {
        config.normalize();
        let snap = SwitchChatteringSnapshot {
            enabled: config.enabled,
            threshold_ms: config.threshold_ms,
            ..Default::default()
        };
        Self {
            config,
            is_down: [false; MOUSE_BUTTON_COUNT],
            last_down_at: [None; MOUSE_BUTTON_COUNT],
            last_up_at: [None; MOUSE_BUTTON_COUNT],
            stats: snap.per_button,
            total_chatter_events: 0,
            last_chatter_button: None,
            last_chatter_interval_ms: None,
        }
    }

    pub fn config(&self) -> ChatteringConfig {
        self.config
    }

    pub fn set_config(&mut self, mut config: ChatteringConfig) {
        config.normalize();
        self.config = config;
        if !self.config.enabled {
            self.is_down = [false; MOUSE_BUTTON_COUNT];
        }
    }

    pub fn reset_stats(&mut self) {
        self.last_down_at = [None; MOUSE_BUTTON_COUNT];
        self.last_up_at = [None; MOUSE_BUTTON_COUNT];
        self.total_chatter_events = 0;
        self.last_chatter_button = None;
        self.last_chatter_interval_ms = None;
        for entry in &mut self.stats {
            entry.total_clicks = 0;
            entry.chatter_count = 0;
            entry.last_interval_ms = None;
            entry.min_interval_ms = None;
        }
    }

    pub fn snapshot(&self) -> SwitchChatteringSnapshot {
        SwitchChatteringSnapshot {
            enabled: self.config.enabled,
            threshold_ms: self.config.threshold_ms,
            total_chatter_events: self.total_chatter_events,
            last_chatter_button: self.last_chatter_button,
            last_chatter_interval_ms: self.last_chatter_interval_ms,
            per_button: self.stats,
        }
    }

    /// Registra uma transição (`down = true` ou `false`) de um botão `0..5`.
    /// Retorna `Some((MouseButton, interval_ms))` quando a transição caracteriza **Switch Chattering**.
    pub fn record_edge(
        &mut self,
        btn_idx: usize,
        down: bool,
        now: Instant,
    ) -> Option<(MouseButton, u32)> {
        if !self.config.enabled || btn_idx >= MOUSE_BUTTON_COUNT {
            return None;
        }

        if down {
            if self.is_down[btn_idx] {
                return None;
            }
            self.is_down[btn_idx] = true;
            let entry = &mut self.stats[btn_idx];
            entry.total_clicks += 1;

            // Prioriza o hiato Release -> Re-press (rebote mecânico clássico); recorre a Press -> Press se não houve UP.
            let interval_ms = self.last_up_at[btn_idx]
                .or(self.last_down_at[btn_idx])
                .map(|prev| now.saturating_duration_since(prev).as_millis() as u32);
            self.last_down_at[btn_idx] = Some(now);

            if let Some(ms) = interval_ms {
                entry.last_interval_ms = Some(ms);
                entry.min_interval_ms = Some(entry.min_interval_ms.map_or(ms, |m| m.min(ms)));
                if ms <= u32::from(self.config.threshold_ms) {
                    entry.chatter_count += 1;
                    self.total_chatter_events += 1;
                    let button = entry.button;
                    self.last_chatter_button = Some(button);
                    self.last_chatter_interval_ms = Some(ms);
                    return Some((button, ms));
                }
            }
            None
        } else {
            self.is_down[btn_idx] = false;
            self.last_up_at[btn_idx] = Some(now);
            None
        }
    }
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    pub const RIM_TYPEMOUSE: u32 = 0;
    pub const RID_INPUT: u32 = 0x1000_0003;
    pub const RIDEV_INPUTSINK: u32 = 0x0000_0100;
    pub const WM_INPUT: u32 = 0x00FF;
    pub const HWND_MESSAGE: *mut c_void = (-3isize) as *mut c_void;

    pub const BUTTON_MASK_PAIRS: [(u16, u16); 5] = [
        (0x0001, 0x0002), // Left
        (0x0004, 0x0008), // Right
        (0x0010, 0x0020), // Middle
        (0x0040, 0x0080), // Back (XBUTTON1)
        (0x0100, 0x0200), // Forward (XBUTTON2)
    ];

    #[repr(C)]
    pub struct RawInputDevice {
        pub us_usage_page: u16,
        pub us_usage: u16,
        pub dw_flags: u32,
        pub hwnd_target: *mut c_void,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RawInputHeader {
        pub dw_type: u32,
        pub dw_size: u32,
        pub h_device: isize,
        pub w_param: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RawMouse {
        pub us_flags: u16,
        pub _pad0: u16,
        pub us_button_flags: u16,
        pub us_button_data: u16,
        pub ul_raw_buttons: u32,
        pub l_last_x: i32,
        pub l_last_y: i32,
        pub ul_extra_information: u32,
    }

    #[repr(C)]
    pub struct RawInputMouse {
        pub header: RawInputHeader,
        pub mouse: RawMouse,
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

    #[link(name = "user32")]
    unsafe extern "system" {
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
        pub fn DestroyWindow(h_wnd: *mut c_void) -> i32;
        pub fn RegisterRawInputDevices(
            p_raw_input_devices: *const RawInputDevice,
            ui_num_devices: u32,
            cb_size: u32,
        ) -> i32;
        pub fn GetRawInputData(
            h_raw_input: isize,
            ui_command: u32,
            p_data: *mut c_void,
            pcb_size: *mut u32,
            cb_size_header: u32,
        ) -> u32;
        pub fn GetMessageW(
            lp_msg: *mut Msg,
            h_wnd: *mut c_void,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
        ) -> i32;
        pub fn TranslateMessage(lp_msg: *const Msg) -> i32;
        pub fn DispatchMessageW(lp_msg: *const Msg) -> isize;
    }
}

/// Inicia a thread bloqueante de monitoramento via Windows Raw Input (`0%` CPU em idle, zero alocações no loop).
pub fn spawn_chattering_monitor(detector: Arc<Mutex<ChatteringDetector>>) {
    #[cfg(windows)]
    {
        let _ = std::thread::Builder::new()
            .name("ajazz-chattering-mon".into())
            .spawn(move || run_windows_raw_input_loop(detector));
    }
    #[cfg(not(windows))]
    {
        let _ = detector;
    }
}

#[cfg(windows)]
fn run_windows_raw_input_loop(detector: Arc<Mutex<ChatteringDetector>>) {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;

    let class_name: [u16; 7] = [b'S' as u16, b'T' as u16, b'A' as u16, b'T' as u16, b'I' as u16, b'C' as u16, 0];
    let title: [u16; 2] = [b'A' as u16, 0];

    let hwnd = unsafe {
        win32::CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            win32::HWND_MESSAGE,
            null_mut(),
            null_mut(),
            null_mut(),
        )
    };
    if hwnd.is_null() {
        warn!("Falha ao criar janela oculta de Raw Input para detector de Switch Chattering.");
        return;
    }

    let rid = win32::RawInputDevice {
        us_usage_page: 0x01,
        us_usage: 0x02,
        dw_flags: win32::RIDEV_INPUTSINK,
        hwnd_target: hwnd,
    };
    let ok = unsafe {
        win32::RegisterRawInputDevices(&rid, 1, size_of::<win32::RawInputDevice>() as u32)
    };
    if ok == 0 {
        warn!("Falha em RegisterRawInputDevices para detector de Switch Chattering.");
        unsafe {
            win32::DestroyWindow(hwnd);
        }
        return;
    }

    info!("Monitor passivo de Switch Chattering ativo via Windows Raw Input.");

    let mut msg: win32::Msg = unsafe { zeroed() };
    let mut raw: win32::RawInputMouse = unsafe { zeroed() };
    let mut active_down = [false; MOUSE_BUTTON_COUNT];

    loop {
        let res = unsafe { win32::GetMessageW(&mut msg, hwnd, 0, 0) };
        if res <= 0 {
            break;
        }
        if msg.message == win32::WM_INPUT {
            let mut size = size_of::<win32::RawInputMouse>() as u32;
            let copied = unsafe {
                win32::GetRawInputData(
                    msg.l_param,
                    win32::RID_INPUT,
                    (&mut raw as *mut win32::RawInputMouse).cast::<c_void>(),
                    &mut size,
                    size_of::<win32::RawInputHeader>() as u32,
                )
            };
            // Ignora cliques injetados por software (`h_device == 0`) e eventos só de cursor (`us_button_flags == 0`).
            if copied != u32::MAX
                && raw.header.dw_type == win32::RIM_TYPEMOUSE
                && raw.header.h_device != 0
                && raw.mouse.us_button_flags != 0
            {
                let flags = raw.mouse.us_button_flags;
                let now = Instant::now();
                if let Ok(mut det) = detector.lock() {
                    if det.config().enabled {
                        for (idx, &(down_mask, up_mask)) in win32::BUTTON_MASK_PAIRS.iter().enumerate() {
                            let has_down = (flags & down_mask) != 0;
                            let has_up = (flags & up_mask) != 0;
                            if !has_down && !has_up {
                                continue;
                            }
                            let events: [Option<bool>; 2] = if has_down && has_up {
                                if active_down[idx] {
                                    [Some(false), Some(true)]
                                } else {
                                    [Some(true), Some(false)]
                                }
                            } else if has_down {
                                [Some(true), None]
                            } else {
                                [Some(false), None]
                            };
                            for edge in events.into_iter().flatten() {
                                active_down[idx] = edge;
                                if let Some((btn, ms)) = det.record_edge(idx, edge, now) {
                                    warn!(
                                        "Switch Chattering detectado em {:?}: intervalo {} ms (limite {} ms)",
                                        btn,
                                        ms,
                                        det.config().threshold_ms
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else {
            unsafe {
                win32::TranslateMessage(&msg);
                win32::DispatchMessageW(&msg);
            }
        }
    }

    unsafe {
        win32::DestroyWindow(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn detects_chattering_below_threshold_and_tracks_min_interval() {
        let mut det = ChatteringDetector::new(ChatteringConfig {
            enabled: true,
            threshold_ms: 45,
        });
        let t0 = Instant::now();

        // Clique normal 1: 0ms down, +80ms up
        assert_eq!(det.record_edge(0, true, t0), None);
        assert_eq!(det.record_edge(0, false, t0 + Duration::from_millis(80)), None);

        // Clique normal 2: +150ms após UP (> 45ms) -> sem chatter
        assert_eq!(det.record_edge(0, true, t0 + Duration::from_millis(230)), None);
        assert_eq!(det.record_edge(0, false, t0 + Duration::from_millis(300)), None);

        // Rebote mecânico: DOWN apenas 12ms após UP (<= 45ms) -> Switch Chattering!
        let alert = det.record_edge(0, true, t0 + Duration::from_millis(312));
        assert_eq!(alert, Some((MouseButton::Left, 12)));

        let snap = det.snapshot();
        assert_eq!(snap.total_chatter_events, 1);
        assert_eq!(snap.last_chatter_button, Some(MouseButton::Left));
        assert_eq!(snap.last_chatter_interval_ms, Some(12));
        assert_eq!(snap.per_button[0].total_clicks, 3);
        assert_eq!(snap.per_button[0].chatter_count, 1);
        assert_eq!(snap.per_button[0].min_interval_ms, Some(12));

        det.reset_stats();
        assert_eq!(det.snapshot().total_chatter_events, 0);
        assert_eq!(det.snapshot().per_button[0].total_clicks, 0);
    }

    #[test]
    fn disabled_detector_ignores_edges() {
        let mut det = ChatteringDetector::new(ChatteringConfig {
            enabled: false,
            threshold_ms: 45,
        });
        let t0 = Instant::now();
        assert_eq!(det.record_edge(1, true, t0), None);
        assert_eq!(det.record_edge(1, false, t0 + Duration::from_millis(5)), None);
        assert_eq!(det.record_edge(1, true, t0 + Duration::from_millis(10)), None);
        assert_eq!(det.snapshot().total_chatter_events, 0);
        assert_eq!(det.snapshot().per_button[1].total_clicks, 0);
    }

    #[cfg(all(windows, target_pointer_width = "64"))]
    #[test]
    fn raw_input_win32_structs_match_x64_abi() {
        assert_eq!(std::mem::size_of::<win32::RawInputHeader>(), 24);
        assert_eq!(std::mem::size_of::<win32::RawMouse>(), 24);
        assert_eq!(std::mem::size_of::<win32::RawInputMouse>(), 48);
    }
}
