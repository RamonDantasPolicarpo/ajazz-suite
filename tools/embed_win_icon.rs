// Utilitário compartilhado entre build.rs para embutir `logo.ico` no recurso PE de executáveis Windows.
use std::path::Path;

fn pad4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn push_res_entry(buf: &mut Vec<u8>, res_type: u16, res_id: u16, data: &[u8]) {
    pad4(buf);
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    buf.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize (dois ordinais)
    buf.extend_from_slice(&[0xFF, 0xFF]);
    buf.extend_from_slice(&res_type.to_le_bytes());
    buf.extend_from_slice(&[0xFF, 0xFF]);
    buf.extend_from_slice(&res_id.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    buf.extend_from_slice(&0x1030u16.to_le_bytes()); // MemoryFlags: MOVEABLE | PURE | DISCARDABLE
    buf.extend_from_slice(&0x0409u16.to_le_bytes()); // LanguageId (en-US padrão PE)
    buf.extend_from_slice(&0u32.to_le_bytes()); // Version
    buf.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    buf.extend_from_slice(data);
}

/// Converte um arquivo `.ico` em um `.res` Win32 (`RT_ICON` + `RT_GROUP_ICON` id=1) em `OUT_DIR` e instrui o linker.
pub fn embed_ico(ico_path: &Path) {
    println!("cargo:rerun-if-changed={}", ico_path.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let ico = std::fs::read(ico_path).expect("Falha ao ler arquivo .ico do ícone");
    assert!(ico.len() >= 6, "Arquivo .ico inválido (muito curto)");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(
        count > 0 && ico.len() >= 6 + count * 16,
        "Cabeçalho .ico inconsistente"
    );

    let mut res = Vec::with_capacity(ico.len() + 256);
    // Cabeçalho nulo obrigatório de arquivo .res (32 bytes)
    res.extend_from_slice(&[
        0, 0, 0, 0, 32, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ]);

    let mut grp = Vec::with_capacity(6 + count * 14);
    grp.extend_from_slice(&ico[..6]);

    for i in 0..count {
        let off = 6 + i * 16;
        let bytes_in_res = u32::from_le_bytes([
            ico[off + 8],
            ico[off + 9],
            ico[off + 10],
            ico[off + 11],
        ]) as usize;
        let img_off = u32::from_le_bytes([
            ico[off + 12],
            ico[off + 13],
            ico[off + 14],
            ico[off + 15],
        ]) as usize;
        let payload = &ico[img_off..img_off + bytes_in_res];

        // RT_ICON = 3
        push_res_entry(&mut res, 3, (i + 1) as u16, payload);

        // GRPICONDIRENTRY: primeiros 12 bytes + nID (u16)
        grp.extend_from_slice(&ico[off..off + 12]);
        grp.extend_from_slice(&((i + 1) as u16).to_le_bytes());
    }

    // RT_GROUP_ICON = 14, ID = 1
    push_res_entry(&mut res, 14, 1, &grp);
    pad4(&mut res);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR ausente");
    let res_path = Path::new(&out_dir).join("ajazz_icon.res");
    std::fs::write(&res_path, res).expect("Falha ao gravar arquivo .res do ícone");
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}
