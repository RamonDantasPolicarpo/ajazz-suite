use ajazz_protocol::{AudioVisualizer, LightingConfig, RgbColor, AUDIO_FFT_SIZE, TOTAL_PHYSICAL_LEDS};

/// Janela deslizante fixa de 1024 amostras PCM mono (`-1.0..=1.0`), sem alocações dinâmicas.
pub struct SampleWindow {
    samples: [f32; AUDIO_FFT_SIZE],
}

impl SampleWindow {
    pub fn new() -> Self {
        Self {
            samples: [0.0; AUDIO_FFT_SIZE],
        }
    }

    pub fn samples(&self) -> &[f32; AUDIO_FFT_SIZE] {
        &self.samples
    }

    /// Desloca a janela e anexa novas amostras mono ao final.
    #[cfg(test)]
    pub fn push_mono(&mut self, incoming: &[f32]) {
        if incoming.is_empty() {
            return;
        }
        if incoming.len() >= AUDIO_FFT_SIZE {
            let tail = &incoming[incoming.len() - AUDIO_FFT_SIZE..];
            self.samples.copy_from_slice(tail);
        } else {
            let shift = incoming.len();
            self.samples.copy_within(shift.., 0);
            self.samples[AUDIO_FFT_SIZE - shift..].copy_from_slice(incoming);
        }
    }

    /// Preenche `count` quadros de silêncio (`0.0`) para decaimento suave sem alocar memória.
    pub fn push_silence(&mut self, count: usize) {
        if count >= AUDIO_FFT_SIZE {
            self.samples.fill(0.0);
        } else if count > 0 {
            self.samples.copy_within(count.., 0);
            self.samples[AUDIO_FFT_SIZE - count..].fill(0.0);
        }
    }

    /// Decodifica um pacote PCM/IEEE-Float intercalado para mono diretamente na janela fixa.
    pub fn push_interleaved(&mut self, bytes: &[u8], frames: usize, spec: AudioSampleSpec) {
        if frames == 0 || spec.channels == 0 {
            return;
        }
        let ch = spec.channels as usize;
        let stride = ch * spec.bytes_per_sample();
        let avail_frames = frames.min(bytes.len() / stride);
        if avail_frames == 0 {
            return;
        }

        // Mantém no máximo os últimos AUDIO_FFT_SIZE quadros para zero overhead em rajadas.
        let skip = avail_frames.saturating_sub(AUDIO_FFT_SIZE);
        let process_frames = avail_frames - skip;
        if process_frames >= AUDIO_FFT_SIZE {
            self.samples.fill(0.0);
        } else {
            self.samples.copy_within(process_frames.., 0);
        }

        let dst_start = AUDIO_FFT_SIZE - process_frames;
        let inv_ch = 1.0f32 / (ch as f32);

        for i in 0..process_frames {
            let frame_off = (skip + i) * stride;
            let mut sum = 0.0f32;
            for c in 0..ch {
                let s_off = frame_off + c * spec.bytes_per_sample();
                sum += decode_sample(&bytes[s_off..s_off + spec.bytes_per_sample()], spec.kind);
            }
            self.samples[dst_start + i] = (sum * inv_ch).clamp(-1.0, 1.0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    F32,
    I16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSampleSpec {
    pub channels: u16,
    pub kind: SampleKind,
}

impl AudioSampleSpec {
    pub const fn bytes_per_sample(self) -> usize {
        match self.kind {
            SampleKind::F32 => 4,
            SampleKind::I16 => 2,
        }
    }
}

#[inline]
fn decode_sample(chunk: &[u8], kind: SampleKind) -> f32 {
    match kind {
        SampleKind::F32 => f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        SampleKind::I16 => (i16::from_le_bytes([chunk[0], chunk[1]]) as f32) / 32768.0,
    }
}

#[cfg(windows)]
mod wasapi {
    use super::{AudioSampleSpec, SampleKind, SampleWindow};
    use std::ffi::c_void;
    use std::ptr::null_mut;

    type Hresult = i32;

    const COINIT_MULTITHREADED: u32 = 0;
    const RPC_E_CHANGED_MODE: Hresult = -2147417850; // 0x80010106
    const CLSCTX_ALL: u32 = 0x17;
    const E_RENDER: i32 = 0;
    const E_CONSOLE: i32 = 0;
    const AUDCLNT_SHAREMODE_SHARED: i32 = 0;
    const AUDCLNT_STREAMFLAGS_LOOPBACK: u32 = 0x0002_0000;
    const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 2;
    const AUDCLNT_S_BUFFER_EMPTY: Hresult = 0x0889_0001;

    const WAVE_FORMAT_PCM: u16 = 1;
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const CLSID_MM_DEVICE_ENUMERATOR: Guid = Guid {
        data1: 0xBCDE_0395,
        data2: 0xE52F,
        data3: 0x467C,
        data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
    };
    const IID_IMM_DEVICE_ENUMERATOR: Guid = Guid {
        data1: 0xA956_64D2,
        data2: 0x9614,
        data3: 0x4F35,
        data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
    };
    const IID_IAUDIO_CLIENT: Guid = Guid {
        data1: 0x1CB9_AD4C,
        data2: 0xDBFA,
        data3: 0x4C32,
        data4: [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2],
    };
    const IID_IAUDIO_CAPTURE_CLIENT: Guid = Guid {
        data1: 0xC8AD_BD64,
        data2: 0xE71E,
        data3: 0x48A0,
        data4: [0xA4, 0xDE, 0x18, 0x5C, 0x39, 0x5C, 0xD3, 0x17],
    };
    const KSDATAFORMAT_SUBTYPE_PCM: Guid = Guid {
        data1: 0x0000_0001,
        data2: 0x0000,
        data3: 0x0010,
        data4: [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    };
    const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: Guid = Guid {
        data1: 0x0000_0003,
        data2: 0x0000,
        data3: 0x0010,
        data4: [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    };

    #[repr(C, packed(1))]
    #[derive(Clone, Copy)]
    struct WaveFormatEx {
        w_format_tag: u16,
        n_channels: u16,
        n_samples_per_sec: u32,
        n_avg_bytes_per_sec: u32,
        n_block_align: u16,
        w_bits_per_sample: u16,
        cb_size: u16,
    }

    #[repr(C, packed(1))]
    #[derive(Clone, Copy)]
    struct WaveFormatExtensible {
        format: WaveFormatEx,
        w_valid_bits_per_sample: u16,
        dw_channel_mask: u32,
        sub_format: Guid,
    }

    #[repr(C)]
    struct IUnknownVtbl {
        query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> Hresult,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    struct IMMDeviceEnumeratorVtbl {
        base: IUnknownVtbl,
        enum_audio_endpoints: unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> Hresult,
        get_default_audio_endpoint: unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> Hresult,
    }

    #[repr(C)]
    struct IMMDeviceVtbl {
        base: IUnknownVtbl,
        activate: unsafe extern "system" fn(*mut c_void, *const Guid, u32, *const c_void, *mut *mut c_void) -> Hresult,
    }

    #[repr(C)]
    struct IAudioClientVtbl {
        base: IUnknownVtbl,
        initialize: unsafe extern "system" fn(*mut c_void, i32, u32, i64, i64, *const WaveFormatEx, *const Guid) -> Hresult,
        get_buffer_size: unsafe extern "system" fn(*mut c_void, *mut u32) -> Hresult,
        get_stream_latency: unsafe extern "system" fn(*mut c_void, *mut i64) -> Hresult,
        get_current_padding: unsafe extern "system" fn(*mut c_void, *mut u32) -> Hresult,
        is_format_supported: unsafe extern "system" fn(*mut c_void, i32, *const WaveFormatEx, *mut *mut WaveFormatEx) -> Hresult,
        get_mix_format: unsafe extern "system" fn(*mut c_void, *mut *mut WaveFormatEx) -> Hresult,
        get_device_period: unsafe extern "system" fn(*mut c_void, *mut i64, *mut i64) -> Hresult,
        start: unsafe extern "system" fn(*mut c_void) -> Hresult,
        stop: unsafe extern "system" fn(*mut c_void) -> Hresult,
        reset: unsafe extern "system" fn(*mut c_void) -> Hresult,
        set_event_handle: unsafe extern "system" fn(*mut c_void, *mut c_void) -> Hresult,
        get_service: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> Hresult,
    }

    #[repr(C)]
    struct IAudioCaptureClientVtbl {
        base: IUnknownVtbl,
        get_buffer: unsafe extern "system" fn(*mut c_void, *mut *mut u8, *mut u32, *mut u32, *mut u64, *mut u64) -> Hresult,
        release_buffer: unsafe extern "system" fn(*mut c_void, u32) -> Hresult,
        get_next_packet_size: unsafe extern "system" fn(*mut c_void, *mut u32) -> Hresult,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pv_reserved: *mut c_void, dw_co_init: u32) -> Hresult;
        fn CoUninitialize();
        fn CoCreateInstance(
            rclsid: *const Guid,
            p_unk_outer: *mut c_void,
            dw_cls_context: u32,
            riid: *const Guid,
            ppv: *mut *mut c_void,
        ) -> Hresult;
        fn CoTaskMemFree(pv: *mut c_void);
    }

    struct ComInitGuard {
        should_uninit: bool,
    }

    impl ComInitGuard {
        fn init() -> Result<Self, Hresult> {
            let hr = unsafe { CoInitializeEx(null_mut(), COINIT_MULTITHREADED) };
            if hr >= 0 {
                Ok(Self { should_uninit: true })
            } else if hr == RPC_E_CHANGED_MODE {
                Ok(Self {
                    should_uninit: false,
                })
            } else {
                Err(hr)
            }
        }
    }

    impl Drop for ComInitGuard {
        fn drop(&mut self) {
            if self.should_uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    struct ComPtr(*mut c_void);

    impl ComPtr {
        fn raw(&self) -> *mut c_void {
            self.0
        }

        unsafe fn vtbl<T>(&self) -> &T {
            &*(*(self.0 as *const *const T))
        }
    }

    impl Drop for ComPtr {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let vtbl = self.vtbl::<IUnknownVtbl>();
                    (vtbl.release)(self.0);
                }
                self.0 = null_mut();
            }
        }
    }

    struct TaskMemFormat(*mut WaveFormatEx);

    impl Drop for TaskMemFormat {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CoTaskMemFree(self.0 as *mut c_void) };
                self.0 = null_mut();
            }
        }
    }

    unsafe fn parse_sample_spec(fmt: *const WaveFormatEx) -> Option<AudioSampleSpec> {
        let base = fmt.read_unaligned();
        if base.n_channels == 0 {
            return None;
        }
        match base.w_format_tag {
            WAVE_FORMAT_IEEE_FLOAT if base.w_bits_per_sample == 32 => Some(AudioSampleSpec {
                channels: base.n_channels,
                kind: SampleKind::F32,
            }),
            WAVE_FORMAT_PCM if base.w_bits_per_sample == 16 => Some(AudioSampleSpec {
                channels: base.n_channels,
                kind: SampleKind::I16,
            }),
            WAVE_FORMAT_EXTENSIBLE => {
                let ext = (fmt as *const WaveFormatExtensible).read_unaligned();
                let sub = ext.sub_format;
                if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT && base.w_bits_per_sample == 32 {
                    Some(AudioSampleSpec {
                        channels: base.n_channels,
                        kind: SampleKind::F32,
                    })
                } else if sub == KSDATAFORMAT_SUBTYPE_PCM && base.w_bits_per_sample == 16 {
                    Some(AudioSampleSpec {
                        channels: base.n_channels,
                        kind: SampleKind::I16,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub struct LoopbackSession {
        _com: ComInitGuard,
        audio_client: ComPtr,
        capture_client: ComPtr,
        spec: AudioSampleSpec,
        started: bool,
    }

    impl LoopbackSession {
        pub fn open() -> Result<Self, String> {
            let com = ComInitGuard::init()
                .map_err(|hr| format!("CoInitializeEx falhou (0x{hr:08X})"))?;

            unsafe {
                let mut enum_raw: *mut c_void = null_mut();
                let hr = CoCreateInstance(
                    &CLSID_MM_DEVICE_ENUMERATOR,
                    null_mut(),
                    CLSCTX_ALL,
                    &IID_IMM_DEVICE_ENUMERATOR,
                    &mut enum_raw,
                );
                if hr < 0 || enum_raw.is_null() {
                    return Err(format!("CoCreateInstance MMDeviceEnumerator falhou (0x{hr:08X})"));
                }
                let enumerator = ComPtr(enum_raw);

                let mut dev_raw: *mut c_void = null_mut();
                let hr = (enumerator.vtbl::<IMMDeviceEnumeratorVtbl>().get_default_audio_endpoint)(
                    enumerator.raw(),
                    E_RENDER,
                    E_CONSOLE,
                    &mut dev_raw,
                );
                if hr < 0 || dev_raw.is_null() {
                    return Err(format!("GetDefaultAudioEndpoint falhou (0x{hr:08X})"));
                }
                let device = ComPtr(dev_raw);

                let mut client_raw: *mut c_void = null_mut();
                let hr = (device.vtbl::<IMMDeviceVtbl>().activate)(
                    device.raw(),
                    &IID_IAUDIO_CLIENT,
                    CLSCTX_ALL,
                    null_mut(),
                    &mut client_raw,
                );
                if hr < 0 || client_raw.is_null() {
                    return Err(format!("IMMDevice::Activate falhou (0x{hr:08X})"));
                }
                let audio_client = ComPtr(client_raw);

                let mut fmt_raw: *mut WaveFormatEx = null_mut();
                let hr = (audio_client.vtbl::<IAudioClientVtbl>().get_mix_format)(
                    audio_client.raw(),
                    &mut fmt_raw,
                );
                if hr < 0 || fmt_raw.is_null() {
                    return Err(format!("IAudioClient::GetMixFormat falhou (0x{hr:08X})"));
                }
                let _fmt_guard = TaskMemFormat(fmt_raw);

                let spec = parse_sample_spec(fmt_raw)
                    .ok_or_else(|| "Formato de áudio WASAPI não suportado".to_string())?;

                // 50 ms em unidades de 100 ns no modo compartilhado de loopback.
                let hr = (audio_client.vtbl::<IAudioClientVtbl>().initialize)(
                    audio_client.raw(),
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    500_000,
                    0,
                    fmt_raw,
                    null_mut(),
                );
                if hr < 0 {
                    return Err(format!("IAudioClient::Initialize falhou (0x{hr:08X})"));
                }

                let mut cap_raw: *mut c_void = null_mut();
                let hr = (audio_client.vtbl::<IAudioClientVtbl>().get_service)(
                    audio_client.raw(),
                    &IID_IAUDIO_CAPTURE_CLIENT,
                    &mut cap_raw,
                );
                if hr < 0 || cap_raw.is_null() {
                    return Err(format!("IAudioClient::GetService falhou (0x{hr:08X})"));
                }
                let capture_client = ComPtr(cap_raw);

                let hr = (audio_client.vtbl::<IAudioClientVtbl>().start)(audio_client.raw());
                if hr < 0 {
                    return Err(format!("IAudioClient::Start falhou (0x{hr:08X})"));
                }

                Ok(Self {
                    _com: com,
                    audio_client,
                    capture_client,
                    spec,
                    started: true,
                })
            }
        }

        /// Drena todos os pacotes pendentes na janela deslizante; retorna `true` se houve áudio novo.
        pub fn drain_into(&mut self, window: &mut SampleWindow) -> Result<bool, i32> {
            unsafe {
                let mut got_audio = false;
                loop {
                    let mut packet_len: u32 = 0;
                    let hr = (self
                        .capture_client
                        .vtbl::<IAudioCaptureClientVtbl>()
                        .get_next_packet_size)(
                        self.capture_client.raw(), &mut packet_len
                    );
                    if hr < 0 {
                        return Err(hr);
                    }
                    if packet_len == 0 {
                        break;
                    }

                    let mut data_ptr: *mut u8 = null_mut();
                    let mut num_frames: u32 = 0;
                    let mut flags: u32 = 0;
                    let hr = (self.capture_client.vtbl::<IAudioCaptureClientVtbl>().get_buffer)(
                        self.capture_client.raw(),
                        &mut data_ptr,
                        &mut num_frames,
                        &mut flags,
                        null_mut(),
                        null_mut(),
                    );
                    if hr == AUDCLNT_S_BUFFER_EMPTY {
                        break;
                    }
                    if hr < 0 {
                        return Err(hr);
                    }

                    if num_frames > 0 {
                        if (flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0 || data_ptr.is_null() {
                            window.push_silence(num_frames as usize);
                        } else {
                            let byte_len = (num_frames as usize)
                                * (self.spec.channels as usize)
                                * self.spec.bytes_per_sample();
                            let slice = std::slice::from_raw_parts(data_ptr, byte_len);
                            window.push_interleaved(slice, num_frames as usize, self.spec);
                            got_audio = true;
                        }
                    }

                    let rel_hr = (self
                        .capture_client
                        .vtbl::<IAudioCaptureClientVtbl>()
                        .release_buffer)(
                        self.capture_client.raw(), num_frames
                    );
                    if rel_hr < 0 {
                        return Err(rel_hr);
                    }
                }
                Ok(got_audio)
            }
        }
    }

    impl Drop for LoopbackSession {
        fn drop(&mut self) {
            if self.started {
                unsafe {
                    let _ = (self.audio_client.vtbl::<IAudioClientVtbl>().stop)(
                        self.audio_client.raw(),
                    );
                }
            }
        }
    }
}

/// Capturador em lote de loopback WASAPI + renderizador zero-alloc em matriz RGB.
pub struct AudioSyncEngine {
    window: SampleWindow,
    visualizer: AudioVisualizer,
    #[cfg(windows)]
    session: Option<wasapi::LoopbackSession>,
}

impl AudioSyncEngine {
    pub fn new() -> Self {
        Self {
            window: SampleWindow::new(),
            visualizer: AudioVisualizer::new(),
            #[cfg(windows)]
            session: None,
        }
    }

    /// Captura os pacotes pendentes de áudio do sistema e gera um frame RGB para o teclado.
    pub fn poll_frame(&mut self, cfg: &LightingConfig) -> [RgbColor; TOTAL_PHYSICAL_LEDS] {
        #[cfg(windows)]
        {
            if self.session.is_none() {
                if let Ok(sess) = wasapi::LoopbackSession::open() {
                    self.session = Some(sess);
                }
            }
            if let Some(ref mut sess) = self.session {
                match sess.drain_into(&mut self.window) {
                    Ok(true) => {}
                    Ok(false) => {
                        // Alguns drivers WASAPI loopback não emitem pacotes durante silêncio total;
                        // desliza silêncio equivalente a 512 amostras (~10 ms) para decair suavemente.
                        self.window.push_silence(512);
                    }
                    Err(_) => {
                        // Dispositivo de saída mudou ou desconectou; recria na próxima iteração.
                        self.session = None;
                        self.window.push_silence(AUDIO_FFT_SIZE);
                    }
                }
            } else {
                self.window.push_silence(AUDIO_FFT_SIZE);
            }
        }
        self.visualizer.process_pcm_frame(self.window.samples(), cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_window_handles_partial_and_oversized_pushes() {
        let mut win = SampleWindow::new();
        win.push_mono(&[0.5, 0.25]);
        assert_eq!(win.samples()[AUDIO_FFT_SIZE - 2], 0.5);
        assert_eq!(win.samples()[AUDIO_FFT_SIZE - 1], 0.25);

        let large = [0.75f32; AUDIO_FFT_SIZE + 64];
        win.push_mono(&large);
        assert!(win.samples().iter().all(|&x| (x - 0.75).abs() < 1e-6));

        win.push_silence(AUDIO_FFT_SIZE);
        assert!(win.samples().iter().all(|&x| x == 0.0));
    }

    #[test]
    fn interleaved_stereo_f32_and_i16_downmix_to_mono() {
        let mut win = SampleWindow::new();
        // 2 quadros estéreo f32: (+0.5, -0.5) => 0.0 e (+1.0, +0.5) => +0.75
        let mut f32_bytes = Vec::new();
        for s in [0.5f32, -0.5f32, 1.0f32, 0.5f32] {
            f32_bytes.extend_from_slice(&s.to_le_bytes());
        }
        win.push_interleaved(
            &f32_bytes,
            2,
            AudioSampleSpec {
                channels: 2,
                kind: SampleKind::F32,
            },
        );
        assert!((win.samples()[AUDIO_FFT_SIZE - 2] - 0.0).abs() < 1e-6);
        assert!((win.samples()[AUDIO_FFT_SIZE - 1] - 0.75).abs() < 1e-6);

        // 1 quadro estéreo i16: (16384, 16384) => +0.5
        let mut i16_bytes = Vec::new();
        for s in [16384i16, 16384i16] {
            i16_bytes.extend_from_slice(&s.to_le_bytes());
        }
        win.push_interleaved(
            &i16_bytes,
            1,
            AudioSampleSpec {
                channels: 2,
                kind: SampleKind::I16,
            },
        );
        assert!((win.samples()[AUDIO_FFT_SIZE - 1] - 0.5).abs() < 1e-4);
    }
}
