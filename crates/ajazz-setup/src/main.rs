#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/payloads.rs"));

const INSTALL_FOLDER_NAME: &str = "AjazzSuite";
const SHORTCUT_NAME: &str = "AJAZZ Suite.lnk";
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "AjazzDaemon";

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    pub const WM_COMMAND: u32 = 0x0111;
    pub const CMD_TRAY_EXIT: usize = 1003;
    pub const HKEY_CURRENT_USER: isize = -0x7FFF_FFFFisize;
    pub const KEY_SET_VALUE: u32 = 0x0002;
    pub const REG_SZ: u32 = 1;

    pub const MB_OK: u32 = 0x0000_0000;
    pub const MB_YESNO: u32 = 0x0000_0004;
    pub const MB_ICONERROR: u32 = 0x0000_0010;
    pub const MB_ICONINFORMATION: u32 = 0x0000_0040;
    pub const IDYES: i32 = 6;

    pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    pub const DETACHED_PROCESS: u32 = 0x0000_0008;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn FindWindowW(lp_class_name: *const u16, lp_window_name: *const u16) -> *mut c_void;
        pub fn PostMessageW(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> i32;
        pub fn MessageBoxW(
            h_wnd: *mut c_void,
            lp_text: *const u16,
            lp_caption: *const u16,
            u_type: u32,
        ) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
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

/// Escapa aspas simples para uso seguro dentro de strings PowerShell com `'...'`.
fn ps_escape(s: &str) -> String {
    s.replace('\'', "''")
}

/// Gera o script PowerShell que cria os atalhos no Menu Iniciar e na Área de Trabalho.
fn build_shortcuts_script(gui_exe: &Path, install_dir: &Path) -> String {
    let target = ps_escape(&gui_exe.to_string_lossy());
    let workdir = ps_escape(&install_dir.to_string_lossy());
    let link_name = ps_escape(SHORTCUT_NAME);
    format!(
        "$ws = New-Object -ComObject WScript.Shell\n\
         foreach ($folder in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {{\n\
           if ($folder) {{\n\
             $lnk = $ws.CreateShortcut((Join-Path $folder '{link_name}'))\n\
             $lnk.TargetPath = '{target}'\n\
             $lnk.WorkingDirectory = '{workdir}'\n\
             $lnk.IconLocation = '{target},0'\n\
             $lnk.Description = 'AJAZZ AK820 & AJ179 NL P Companion Suite'\n\
             $lnk.Save()\n\
           }}\n\
         }}"
    )
}

#[cfg(windows)]
fn stop_running_instances() {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let class = to_wide("AjazzDaemonTrayClass");
    let hwnd = unsafe { win32::FindWindowW(class.as_ptr(), std::ptr::null()) };
    if !hwnd.is_null() {
        unsafe {
            win32::PostMessageW(hwnd, win32::WM_COMMAND, win32::CMD_TRAY_EXIT, 0);
        }
        std::thread::sleep(std::time::Duration::from_millis(450));
    }

    for image in ["ajazz-gui.exe", "ajazz-daemon.exe"] {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", image])
            .creation_flags(win32::CREATE_NO_WINDOW)
            .output();
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
}

#[cfg(windows)]
fn set_autostart_registry(daemon_exe: Option<&Path>) -> Result<(), String> {
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
        return Err(format!("Falha ao abrir Registro do Windows (código {res})."));
    }

    let op = match daemon_exe {
        Some(exe) => {
            let quoted = to_wide(&format!("\"{}\"", exe.display()));
            let len = (quoted.len() * 2) as u32;
            unsafe {
                win32::RegSetValueExW(
                    hkey,
                    val.as_ptr(),
                    0,
                    win32::REG_SZ,
                    quoted.as_ptr().cast::<u8>(),
                    len,
                )
            }
        }
        None => {
            let r = unsafe { win32::RegDeleteValueW(hkey, val.as_ptr()) };
            if r == 2 { 0 } else { r }
        }
    };
    unsafe { win32::RegCloseKey(hkey) };
    if op != 0 {
        return Err(format!("Falha ao gravar inicialização automática (código {op})."));
    }
    Ok(())
}

#[cfg(windows)]
fn run_hidden_powershell(script: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script])
        .creation_flags(win32::CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Falha ao executar PowerShell para atalhos: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Erro ao criar atalhos do Windows: {}", err.trim()));
    }
    Ok(())
}

#[cfg(windows)]
fn resolve_install_dir() -> Result<PathBuf, String> {
    let local_app = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "Variável LOCALAPPDATA não encontrada no ambiente Windows.".to_string())?;
    Ok(PathBuf::from(local_app).join(INSTALL_FOLDER_NAME))
}

#[cfg(windows)]
fn install_suite() -> Result<(PathBuf, PathBuf), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    if DAEMON_BIN.is_empty() || GUI_BIN.is_empty() {
        return Err("Este executável foi compilado sem os binários release embutidos.".into());
    }

    stop_running_instances();

    let install_dir = resolve_install_dir()?;
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Falha ao criar pasta {}: {e}", install_dir.display()))?;

    let daemon_path = install_dir.join("ajazz-daemon.exe");
    let gui_path = install_dir.join("ajazz-gui.exe");

    std::fs::write(&daemon_path, DAEMON_BIN)
        .map_err(|e| format!("Falha ao instalar ajazz-daemon.exe: {e}"))?;
    std::fs::write(&gui_path, GUI_BIN)
        .map_err(|e| format!("Falha ao instalar ajazz-gui.exe: {e}"))?;

    set_autostart_registry(Some(&daemon_path))?;

    let ps = build_shortcuts_script(&gui_path, &install_dir);
    run_hidden_powershell(&ps)?;

    Command::new(&daemon_path)
        .current_dir(&install_dir)
        .creation_flags(win32::DETACHED_PROCESS | win32::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Instalado, mas falhou ao iniciar o daemon: {e}"))?;

    Ok((install_dir, gui_path))
}

#[cfg(windows)]
fn uninstall_suite() -> Result<(), String> {
    stop_running_instances();
    set_autostart_registry(None)?;

    let link_name = ps_escape(SHORTCUT_NAME);
    let ps = format!(
        "foreach ($folder in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {{\n\
           if ($folder) {{ Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $folder '{link_name}') }}\n\
         }}"
    );
    let _ = run_hidden_powershell(&ps);

    let install_dir = resolve_install_dir()?;
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .map_err(|e| format!("Falha ao remover {}: {e}", install_dir.display()))?;
    }
    Ok(())
}

#[cfg(windows)]
fn show_message(title: &str, msg: &str, flags: u32) -> i32 {
    let t = to_wide(title);
    let m = to_wide(msg);
    unsafe { win32::MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), flags) }
}

fn main() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let uninstall = std::env::args().any(|a| a == "--uninstall");
        if uninstall {
            match uninstall_suite() {
                Ok(()) => {
                    show_message(
                        "AJAZZ Companion Suite",
                        "Suíte removida com sucesso (atalhos, inicialização automática e executáveis).",
                        win32::MB_OK | win32::MB_ICONINFORMATION,
                    );
                }
                Err(e) => {
                    show_message("Erro na Desinstalação", &e, win32::MB_OK | win32::MB_ICONERROR);
                    std::process::exit(1);
                }
            }
            return;
        }

        match install_suite() {
            Ok((dir, gui_path)) => {
                let msg = format!(
                    "AJAZZ Companion Suite instalada com sucesso!\n\n\
                     • Pasta: {}\n\
                     • Atalhos: Menu Iniciar e Área de Trabalho ('AJAZZ Suite')\n\
                     • Inicialização automática com o Windows: ATIVADA\n\
                     • Serviço já iniciado na bandeja do sistema.\n\n\
                     Deseja abrir a interface agora?",
                    dir.display()
                );
                let ans = show_message(
                    "Instalação Concluída — AJAZZ Suite",
                    &msg,
                    win32::MB_YESNO | win32::MB_ICONINFORMATION,
                );
                if ans == win32::IDYES {
                    let _ = std::process::Command::new(&gui_path)
                        .current_dir(&dir)
                        .creation_flags(win32::DETACHED_PROCESS)
                        .spawn();
                }
            }
            Err(e) => {
                show_message(
                    "Falha na Instalação — AJAZZ Suite",
                    &e,
                    win32::MB_OK | win32::MB_ICONERROR,
                );
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("Este instalador destina-se apenas ao Windows.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_script_quotes_spaces_and_resolves_windows_folders() {
        let dir = Path::new(r"C:\Users\Ramon Test\AppData\Local\AjazzSuite");
        let gui = dir.join("ajazz-gui.exe");
        let script = build_shortcuts_script(&gui, dir);
        assert!(script.contains("GetFolderPath('Programs')"));
        assert!(script.contains("GetFolderPath('Desktop')"));
        assert!(script.contains("IconLocation = 'C:\\Users\\Ramon Test\\AppData\\Local\\AjazzSuite\\ajazz-gui.exe,0'"));
    }

    #[test]
    fn ps_escape_doubles_single_quotes() {
        assert_eq!(ps_escape("O'Brien"), "O''Brien");
    }
}
