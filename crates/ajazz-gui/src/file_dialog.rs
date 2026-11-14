#[cfg(windows)]
mod win32_dialog {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    const COINIT_APARTMENTTHREADED: u32 = 2;
    const RPC_E_CHANGED_MODE: i32 = -2147417850;
    const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
    const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
    const OFN_NOCHANGEDIR: u32 = 0x0000_0008;

    #[repr(C)]
    struct OpenFileNameW {
        l_struct_size: u32,
        hwnd_owner: *mut c_void,
        h_instance: *mut c_void,
        lpstr_filter: *const u16,
        lpstr_custom_filter: *mut u16,
        n_max_cust_filter: u32,
        n_filter_index: u32,
        lpstr_file: *mut u16,
        n_max_file: u32,
        lpstr_file_title: *mut u16,
        n_max_file_title: u32,
        lpstr_initial_dir: *const u16,
        lpstr_title: *const u16,
        flags: u32,
        n_file_offset: u16,
        n_file_extension: u16,
        lpstr_def_ext: *const u16,
        l_cust_data: isize,
        lpfn_hook: *const c_void,
        lp_template_name: *const u16,
        pv_reserved: *mut c_void,
        dw_reserved: u32,
        flags_ex: u32,
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pv_reserved: *mut c_void, dw_co_init: u32) -> i32;
        fn CoUninitialize();
    }

    #[link(name = "comdlg32")]
    extern "system" {
        fn GetOpenFileNameW(lpofn: *mut OpenFileNameW) -> i32;
    }

    struct ComStaGuard(bool);

    impl ComStaGuard {
        fn enter() -> Self {
            let hr = unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED) };
            Self(hr >= 0 && hr != RPC_E_CHANGED_MODE)
        }
    }

    impl Drop for ComStaGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    pub fn open_gif_dialog() -> Option<String> {
        let _com = ComStaGuard::enter();
        let filter = super::utf16_filter_gif();
        let title: Vec<u16> = "Selecionar animação GIF\0".encode_utf16().collect();
        let mut path_buf = vec![0u16; 1024];

        let mut ofn = OpenFileNameW {
            l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
            hwnd_owner: null_mut(),
            h_instance: null_mut(),
            lpstr_filter: filter.as_ptr(),
            lpstr_custom_filter: null_mut(),
            n_max_cust_filter: 0,
            n_filter_index: 1,
            lpstr_file: path_buf.as_mut_ptr(),
            n_max_file: path_buf.len() as u32,
            lpstr_file_title: null_mut(),
            n_max_file_title: 0,
            lpstr_initial_dir: null(),
            lpstr_title: title.as_ptr(),
            flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
            n_file_offset: 0,
            n_file_extension: 0,
            lpstr_def_ext: null(),
            l_cust_data: 0,
            lpfn_hook: null(),
            lp_template_name: null(),
            pv_reserved: null_mut(),
            dw_reserved: 0,
            flags_ex: 0,
        };

        let ok = unsafe { GetOpenFileNameW(&mut ofn) };
        if ok == 0 {
            return None;
        }
        let end = path_buf.iter().position(|&c| c == 0).unwrap_or(path_buf.len());
        if end == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&path_buf[..end]))
    }
}

fn utf16_filter_gif() -> Vec<u16> {
    "Animacao GIF (*.gif)\0*.gif\0Todos (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect()
}

/// Abre o diálogo nativo do sistema para seleção de um arquivo `.gif`.
pub fn pick_gif_file() -> Option<String> {
    #[cfg(windows)]
    {
        win32_dialog::open_gif_dialog()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::utf16_filter_gif;

    #[test]
    fn gif_filter_ends_with_double_nul() {
        let f = utf16_filter_gif();
        assert!(f.len() >= 2);
        assert_eq!(f[f.len() - 1], 0);
        assert_eq!(f[f.len() - 2], 0);
    }
}
