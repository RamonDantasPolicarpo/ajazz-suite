use ajazz_protocol::UserProfile;
use std::path::PathBuf;
use tracing::{info, warn};

/// Retorna o caminho canônico de persistência do User Profile: `%APPDATA%\ajazz-companion\config.json`.
pub fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ajazz-companion").join("config.json")
}

/// Carrega o User Profile salvo em disco (ou cria padrão caso não exista/esteja corrompido).
pub fn load_profile() -> UserProfile {
    let path = config_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return UserProfile::default();
    };
    match serde_json::from_str::<UserProfile>(&raw) {
        Ok(mut profile) => {
            profile.normalize();
            info!("User Profile carregado de {}", path.display());
            profile
        }
        Err(e) => {
            warn!("Falha ao ler {}: {e}. Usando configuração padrão.", path.display());
            UserProfile::default()
        }
    }
}

/// Persiste o User Profile em disco no caminho canônico.
pub fn save_profile(profile: &UserProfile) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Não foi possível criar diretório {}: {e}", parent.display());
            return;
        }
    }
    match serde_json::to_string_pretty(profile) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("Erro ao salvar {}: {e}", path.display());
            }
        }
        Err(e) => warn!("Erro ao serializar User Profile: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ajazz_protocol::{LightingMode, RgbColor};

    #[test]
    fn test_user_profile_roundtrip_and_normalize() {
        let mut p = UserProfile::default();
        p.lighting.mode = LightingMode::Rain;
        p.lighting.multicolor = true;
        p.lighting.brightness = 99;
        p.custom_banks[1].truncate(4);
        p.custom_banks[1][0] = RgbColor::new(0x12, 0x34, 0x56);

        let json = serde_json::to_string(&p).unwrap();
        let mut decoded: UserProfile = serde_json::from_str(&json).unwrap();
        decoded.normalize();

        assert_eq!(decoded.lighting.mode, LightingMode::Rain);
        assert!(decoded.lighting.multicolor);
        assert_eq!(decoded.lighting.brightness, 4);
        assert_eq!(decoded.custom_banks[1].len(), ajazz_protocol::TOTAL_PHYSICAL_LEDS);
        assert_eq!(decoded.custom_banks[1][0], RgbColor::new(0x12, 0x34, 0x56));
    }
}
