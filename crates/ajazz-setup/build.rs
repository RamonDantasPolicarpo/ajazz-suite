use std::path::{Path, PathBuf};

#[path = "../../tools/embed_win_icon.rs"]
mod embed_win_icon;

fn main() {
    embed_win_icon::embed_ico(Path::new("../ajazz-gui/ui/assets/logo.ico"));

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let release_dir = manifest_dir.join("../../target/release");
    let daemon_exe = release_dir.join("ajazz-daemon.exe");
    let gui_exe = release_dir.join("ajazz-gui.exe");

    println!("cargo:rerun-if-changed={}", daemon_exe.display());
    println!("cargo:rerun-if-changed={}", gui_exe.display());

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let has_bins = daemon_exe.is_file() && gui_exe.is_file();
    if profile == "release" && !has_bins {
        panic!(
            "Compile os binários release antes do instalador: cargo build --release --bins (esperado em {})",
            release_dir.display()
        );
    }

    let code = if profile == "release" {
        format!(
            "pub const DAEMON_BIN: &[u8] = include_bytes!({:?});\npub const GUI_BIN: &[u8] = include_bytes!({:?});\n",
            daemon_exe.to_string_lossy(),
            gui_exe.to_string_lossy()
        )
    } else {
        "pub const DAEMON_BIN: &[u8] = &[];\npub const GUI_BIN: &[u8] = &[];\n".to_string()
    };

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("payloads.rs"), code).expect("Falha ao gerar payloads.rs");
}
