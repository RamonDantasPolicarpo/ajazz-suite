use ajazz_protocol::mouse::{
    ChatteringConfig, SwitchChatteringButtonStats, SwitchChatteringSnapshot, MOUSE_BUTTON_COUNT,
};
use ajazz_protocol::{
    KeyboardChatteringConfig, KeyboardChatteringKeyStats, KeyboardChatteringSnapshot, MouseButton,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{info, warn};

const STUCK_HOLD_THRESHOLD_MS: u32 = 10_000;
const KEYBOARD_TRACKED_SLOTS: usize = 128;

/// Máquina de estados pura (zero alocações no hot path) para detectar **Switch Chattering**
/// e **Stuck Input** nos 5 switches físicos do mouse.
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
    current_battery: Option<u8>,
    stuck_count: u64,
    last_stuck_button: Option<MouseButton>,
    last_stuck_battery: Option<u8>,
    last_stuck_hold_ms: Option<u32>,
    last_stuck_missed_up: bool,
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
            current_battery: None,
            stuck_count: 0,
            last_stuck_button: None,
            last_stuck_battery: None,
            last_stuck_hold_ms: None,
            last_stuck_missed_up: false,
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

    pub fn set_mouse_battery(&mut self, battery: Option<u8>) {
        self.current_battery = battery;
    }

    pub fn reset_stats(&mut self) {
        self.last_down_at = [None; MOUSE_BUTTON_COUNT];
        self.last_up_at = [None; MOUSE_BUTTON_COUNT];
        self.total_chatter_events = 0;
        self.last_chatter_button = None;
        self.last_chatter_interval_ms = None;
        self.stuck_count = 0;
        self.last_stuck_button = None;
        self.last_stuck_battery = None;
        self.last_stuck_hold_ms = None;
        self.last_stuck_missed_up = false;
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
            stuck_count: self.stuck_count,
            last_stuck_button: self.last_stuck_button,
            last_stuck_battery: self.last_stuck_battery,
            last_stuck_hold_ms: self.last_stuck_hold_ms,
            last_stuck_missed_up: self.last_stuck_missed_up,
        }
    }

    fn record_stuck(&mut self, btn_idx: usize, hold_ms: Option<u32>, missed_up: bool) {
        self.stuck_count += 1;
        self.last_stuck_button = Some(self.stats[btn_idx].button);
        self.last_stuck_battery = self.current_battery;
        self.last_stuck_hold_ms = hold_ms;
        self.last_stuck_missed_up = missed_up;
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
                let hold_ms = self.last_down_at[btn_idx]
                    .map(|prev| now.saturating_duration_since(prev).as_millis() as u32);
                self.record_stuck(btn_idx, hold_ms, true);
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
            if self.is_down[btn_idx] {
                if let Some(prev_down) = self.last_down_at[btn_idx] {
                    let hold_ms = now.saturating_duration_since(prev_down).as_millis() as u32;
                    if hold_ms >= STUCK_HOLD_THRESHOLD_MS {
                        self.record_stuck(btn_idx, Some(hold_ms), false);
                    }
                }
            }
            self.is_down[btn_idx] = false;
            self.last_up_at[btn_idx] = Some(now);
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct KeySlot {
    used: bool,
    key_id: u32,
    vkey: u16,
    make_code: u16,
    is_e0: bool,
    is_down: bool,
    last_up_at: Option<Instant>,
    total_presses: u64,
    chatter_count: u64,
    last_interval_ms: Option<u32>,
    min_interval_ms: Option<u32>,
}

impl Default for KeySlot {
    fn default() -> Self {
        Self {
            used: false,
            key_id: 0,
            vkey: 0,
            make_code: 0,
            is_e0: false,
            is_down: false,
            last_up_at: None,
            total_presses: 0,
            chatter_count: 0,
            last_interval_ms: None,
            min_interval_ms: None,
        }
    }
}

/// Detector passivo de **Switch Chattering** para teclas físicas do teclado (zero alocações no hot path).
pub struct KeyboardChatteringDetector {
    config: KeyboardChatteringConfig,
    enabled_flag: Arc<AtomicBool>,
    slots: [KeySlot; KEYBOARD_TRACKED_SLOTS],
    total_chatter_events: u64,
    last_chatter_key: Option<String>,
    last_chatter_interval_ms: Option<u32>,
}

impl KeyboardChatteringDetector {
    pub fn new(mut config: KeyboardChatteringConfig) -> Self {
        config.normalize();
        let enabled_flag = Arc::new(AtomicBool::new(config.enabled));
        Self {
            config,
            enabled_flag,
            slots: [KeySlot::default(); KEYBOARD_TRACKED_SLOTS],
            total_chatter_events: 0,
            last_chatter_key: None,
            last_chatter_interval_ms: None,
        }
    }

    pub fn enabled_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.enabled_flag)
    }

    pub fn config(&self) -> KeyboardChatteringConfig {
        self.config
    }

    pub fn set_config(&mut self, mut config: KeyboardChatteringConfig) {
        config.normalize();
        self.config = config;
        self.enabled_flag.store(config.enabled, Ordering::Relaxed);
        if !config.enabled {
            for slot in &mut self.slots {
                slot.is_down = false;
            }
        }
    }

    pub fn reset_stats(&mut self) {
        self.total_chatter_events = 0;
        self.last_chatter_key = None;
        self.last_chatter_interval_ms = None;
        self.slots = [KeySlot::default(); KEYBOARD_TRACKED_SLOTS];
    }

    pub fn snapshot(&self) -> KeyboardChatteringSnapshot {
        let mut keys: Vec<KeyboardChatteringKeyStats> = self
            .slots
            .iter()
            .filter(|s| s.used && s.total_presses > 0)
            .map(|s| KeyboardChatteringKeyStats {
                label: key_label_from_raw(s.vkey, s.make_code, s.is_e0),
                vkey: s.vkey,
                make_code: s.make_code,
                total_presses: s.total_presses,
                chatter_count: s.chatter_count,
                last_interval_ms: s.last_interval_ms,
                min_interval_ms: s.min_interval_ms,
            })
            .collect();

        keys.sort_by(|a, b| {
            b.chatter_count
                .cmp(&a.chatter_count)
                .then_with(|| b.total_presses.cmp(&a.total_presses))
                .then_with(|| a.label.cmp(&b.label))
        });

        KeyboardChatteringSnapshot {
            enabled: self.config.enabled,
            threshold_ms: self.config.threshold_ms,
            total_chatter_events: self.total_chatter_events,
            last_chatter_key: self.last_chatter_key.clone(),
            last_chatter_interval_ms: self.last_chatter_interval_ms,
            keys,
        }
    }

    fn find_or_alloc_slot(&mut self, vkey: u16, make_code: u16, is_e0: bool) -> Option<usize> {
        let key_id = (u32::from(is_e0) << 24) | (u32::from(make_code) << 16) | u32::from(vkey);
        let mut free_idx = None;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.used {
                if slot.key_id == key_id {
                    return Some(i);
                }
            } else if free_idx.is_none() {
                free_idx = Some(i);
            }
        }
        let idx = free_idx?;
        self.slots[idx] = KeySlot {
            used: true,
            key_id,
            vkey,
            make_code,
            is_e0,
            ..Default::default()
        };
        Some(idx)
    }

    /// Registra uma transição de tecla (`down = true/false`), ignorando `auto-repeat` do Windows.
    pub fn record_key_edge(
        &mut self,
        vkey: u16,
        make_code: u16,
        is_e0: bool,
        down: bool,
        now: Instant,
    ) -> Option<(String, u32)> {
        if !self.config.enabled || vkey == 0 || vkey == 0xFF {
            return None;
        }
        let idx = self.find_or_alloc_slot(vkey, make_code, is_e0)?;
        let slot = &mut self.slots[idx];

        if down {
            // Repetição automática de tecla mantida pressionada no Windows não é rebote mecânico.
            if slot.is_down {
                return None;
            }
            slot.is_down = true;
            slot.total_presses += 1;

            if let Some(last_up) = slot.last_up_at {
                let ms = now.saturating_duration_since(last_up).as_millis() as u32;
                slot.last_interval_ms = Some(ms);
                slot.min_interval_ms = Some(slot.min_interval_ms.map_or(ms, |m| m.min(ms)));
                if ms <= u32::from(self.config.threshold_ms) {
                    slot.chatter_count += 1;
                    self.total_chatter_events += 1;
                    let label = key_label_from_raw(vkey, make_code, is_e0);
                    self.last_chatter_key = Some(label.clone());
                    self.last_chatter_interval_ms = Some(ms);
                    return Some((label, ms));
                }
            }
            None
        } else {
            slot.is_down = false;
            slot.last_up_at = Some(now);
            None
        }
    }
}

pub fn key_label_from_raw(vkey: u16, make_code: u16, is_e0: bool) -> String {
    match vkey {
        0x08 => "BACKSPACE".into(),
        0x09 => "TAB".into(),
        0x0D => if is_e0 { "NUM ENTER" } else { "ENTER" }.into(),
        0x10 => if make_code == 0x36 { "RSHIFT" } else { "LSHIFT" }.into(),
        0x11 => if is_e0 { "RCTRL" } else { "LCTRL" }.into(),
        0x12 => if is_e0 { "ALTGR" } else { "LALT" }.into(),
        0x14 => "CAPS".into(),
        0x1B => "ESC".into(),
        0x20 => "SPACE".into(),
        0x21 => "PGUP".into(),
        0x22 => "PGDN".into(),
        0x23 => "END".into(),
        0x24 => "HOME".into(),
        0x25 => "LEFT".into(),
        0x26 => "UP".into(),
        0x27 => "RIGHT".into(),
        0x28 => "DOWN".into(),
        0x2C => "PRTSC".into(),
        0x2D => "INS".into(),
        0x2E => "DEL".into(),
        0x30..=0x39 => format!("{}", (vkey as u8) as char),
        0x41..=0x5A => format!("{}", (vkey as u8) as char),
        0x5B => "LWIN".into(),
        0x5C => "RWIN".into(),
        0x5D => "MENU".into(),
        0x70..=0x7B => format!("F{}", vkey - 0x6F),
        0xBA => ";/:".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => ";/".into(),
        0xC0 => "'".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "~".into(),
        _ => format!("VK 0x{vkey:02X}"),
    }
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    pub const RIM_TYPEMOUSE: u32 = 0;
    pub const RIM_TYPEKEYBOARD: u32 = 1;
    pub const RID_INPUT: u32 = 0x1000_0003;
    pub const RIDEV_INPUTSINK: u32 = 0x0000_0100;
    pub const WM_INPUT: u32 = 0x00FF;
    pub const RI_KEY_BREAK: u16 = 0x0001;
    pub const RI_KEY_E0: u16 = 0x0002;
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
    #[derive(Clone, Copy)]
    pub struct RawKeyboard {
        pub make_code: u16,
        pub flags: u16,
        pub reserved: u16,
        pub v_key: u16,
        pub message: u32,
        pub extra_information: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union RawInputData {
        pub mouse: RawMouse,
        pub keyboard: RawKeyboard,
    }

    #[repr(C)]
    pub struct RawInputPacket {
        pub header: RawInputHeader,
        pub data: RawInputData,
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
pub fn spawn_chattering_monitor(
    mouse_detector: Arc<Mutex<ChatteringDetector>>,
    kb_detector: Arc<Mutex<KeyboardChatteringDetector>>,
    kb_enabled_flag: Arc<AtomicBool>,
) {
    #[cfg(windows)]
    {
        let _ = std::thread::Builder::new()
            .name("ajazz-chattering-mon".into())
            .spawn(move || {
                run_windows_raw_input_loop(mouse_detector, kb_detector, kb_enabled_flag)
            });
    }
    #[cfg(not(windows))]
    {
        let _ = (mouse_detector, kb_detector, kb_enabled_flag);
    }
}

#[cfg(windows)]
fn run_windows_raw_input_loop(
    mouse_detector: Arc<Mutex<ChatteringDetector>>,
    kb_detector: Arc<Mutex<KeyboardChatteringDetector>>,
    kb_enabled_flag: Arc<AtomicBool>,
) {
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

    let rids = [
        win32::RawInputDevice {
            us_usage_page: 0x01,
            us_usage: 0x02,
            dw_flags: win32::RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
        win32::RawInputDevice {
            us_usage_page: 0x01,
            us_usage: 0x06,
            dw_flags: win32::RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
    ];
    let ok = unsafe {
        win32::RegisterRawInputDevices(
            rids.as_ptr(),
            rids.len() as u32,
            size_of::<win32::RawInputDevice>() as u32,
        )
    };
    if ok == 0 {
        warn!("Falha em RegisterRawInputDevices para detector de Switch Chattering.");
        unsafe {
            win32::DestroyWindow(hwnd);
        }
        return;
    }

    info!("Monitor passivo de Switch Chattering & Stuck Input ativo via Windows Raw Input.");

    let mut msg: win32::Msg = unsafe { zeroed() };
    let mut raw: win32::RawInputPacket = unsafe { zeroed() };
    let mut active_down = [false; MOUSE_BUTTON_COUNT];

    loop {
        let res = unsafe { win32::GetMessageW(&mut msg, hwnd, 0, 0) };
        if res <= 0 {
            break;
        }
        if msg.message == win32::WM_INPUT {
            let mut size = size_of::<win32::RawInputPacket>() as u32;
            let copied = unsafe {
                win32::GetRawInputData(
                    msg.l_param,
                    win32::RID_INPUT,
                    (&mut raw as *mut win32::RawInputPacket).cast::<c_void>(),
                    &mut size,
                    size_of::<win32::RawInputHeader>() as u32,
                )
            };
            if copied == u32::MAX || raw.header.h_device == 0 {
                continue;
            }

            if raw.header.dw_type == win32::RIM_TYPEMOUSE {
                let mouse = unsafe { raw.data.mouse };
                if mouse.us_button_flags == 0 {
                    continue;
                }
                let flags = mouse.us_button_flags;
                let now = Instant::now();
                if let Ok(mut det) = mouse_detector.lock() {
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
            } else if raw.header.dw_type == win32::RIM_TYPEKEYBOARD
                && kb_enabled_flag.load(Ordering::Relaxed)
            {
                let kb = unsafe { raw.data.keyboard };
                let down = (kb.flags & win32::RI_KEY_BREAK) == 0;
                let is_e0 = (kb.flags & win32::RI_KEY_E0) != 0;
                let now = Instant::now();
                if let Ok(mut det) = kb_detector.lock() {
                    if let Some((key_label, ms)) =
                        det.record_key_edge(kb.v_key, kb.make_code, is_e0, down, now)
                    {
                        warn!(
                            "Switch Chattering no teclado em [{}]: {} ms (limite {} ms)",
                            key_label,
                            ms,
                            det.config().threshold_ms
                        );
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

    #[test]
    fn detects_stuck_input_missed_up_with_battery_correlation() {
        let mut det = ChatteringDetector::new(ChatteringConfig {
            enabled: true,
            threshold_ms: 45,
        });
        det.set_mouse_battery(Some(20));
        let t0 = Instant::now();

        // Pressiona esquerdo e perde evento UP no rádio 2.4G -> usuário clica de novo depois de 2s para "soltar"
        assert_eq!(det.record_edge(0, true, t0), None);
        assert_eq!(det.record_edge(0, true, t0 + Duration::from_millis(2000)), None);

        let snap = det.snapshot();
        assert_eq!(snap.stuck_count, 1);
        assert_eq!(snap.last_stuck_button, Some(MouseButton::Left));
        assert_eq!(snap.last_stuck_battery, Some(20));
        assert_eq!(snap.last_stuck_hold_ms, Some(2000));
        assert!(snap.last_stuck_missed_up);
    }

    #[test]
    fn keyboard_detector_ignores_auto_repeat_and_flags_switch_chattering() {
        let mut kb = KeyboardChatteringDetector::new(KeyboardChatteringConfig {
            enabled: true,
            threshold_ms: 45,
        });
        let t0 = Instant::now();

        // Pressão normal + vários auto-repeats segurando a tecla 'E' (0x45)
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, true, t0), None);
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, true, t0 + Duration::from_millis(20)), None);
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, true, t0 + Duration::from_millis(40)), None);
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, false, t0 + Duration::from_millis(120)), None);

        // Digitação normal: +150ms depois
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, true, t0 + Duration::from_millis(270)), None);
        assert_eq!(kb.record_key_edge(0x45, 0x12, false, false, t0 + Duration::from_millis(320)), None);

        // Rebote mecânico: DOWN 14ms depois do UP -> Switch Chattering!
        let alert = kb.record_key_edge(0x45, 0x12, false, true, t0 + Duration::from_millis(334));
        assert_eq!(alert, Some(("E".to_string(), 14)));

        let snap = kb.snapshot();
        assert_eq!(snap.total_chatter_events, 1);
        assert_eq!(snap.last_chatter_key.as_deref(), Some("E"));
        assert_eq!(snap.keys.len(), 1);
        assert_eq!(snap.keys[0].total_presses, 3);
        assert_eq!(snap.keys[0].chatter_count, 1);
        assert_eq!(snap.keys[0].min_interval_ms, Some(14));
    }

    #[test]
    fn keyboard_detector_starts_disabled_and_ignores_when_paused() {
        let mut kb = KeyboardChatteringDetector::new(KeyboardChatteringConfig::default());
        assert!(!kb.config().enabled);
        let t0 = Instant::now();
        assert_eq!(kb.record_key_edge(0x41, 0x1E, false, true, t0), None);
        assert_eq!(kb.record_key_edge(0x41, 0x1E, false, false, t0 + Duration::from_millis(5)), None);
        assert_eq!(kb.record_key_edge(0x41, 0x1E, false, true, t0 + Duration::from_millis(12)), None);
        assert_eq!(kb.snapshot().total_chatter_events, 0);
        assert!(kb.snapshot().keys.is_empty());
    }

    #[cfg(all(windows, target_pointer_width = "64"))]
    #[test]
    fn raw_input_win32_structs_match_x64_abi() {
        assert_eq!(std::mem::size_of::<win32::RawInputHeader>(), 24);
        assert_eq!(std::mem::size_of::<win32::RawMouse>(), 24);
        assert_eq!(std::mem::size_of::<win32::RawKeyboard>(), 16);
        assert_eq!(std::mem::size_of::<win32::RawInputPacket>(), 48);
    }
}
