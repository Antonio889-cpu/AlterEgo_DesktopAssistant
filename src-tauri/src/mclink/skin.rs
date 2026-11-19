//! Skin di Alter Ego in Minecraft.
//!
//! Con un account offline il server non sa che skin mostrare. Due strade che funzionano davvero:
//! - CustomSkinLoader (mod lato client, Fabric/Forge): legge `.minecraft/CustomSkinLoader/LocalSkin/skins/<nome>.png`;
//!   l'app copia lì la skin, la vede chi ha la mod (di solito tu che ospiti il mondo);
//! - SkinsRestorer (plugin del server): il bot scrive `/skin set <nome>` o `/skin url "<url>"` appena entra.
//!   L'URL deve essere pubblico (lo elabora MineSkin): si ottiene caricando il PNG su skinsrestorer.net/upload.

use std::path::{Path, PathBuf};

/// Skin inclusa: "Alter Ego (Chihiro)" (tools/make_mc_skin.py).
pub const PRESET: &[u8] = include_bytes!("../../../src/assets/minecraft/alterego_skin.png");

/// Controlla che sia un PNG da skin: 64x64 (formato 1.8+) o 64x32 (vecchio).
pub fn validate(bytes: &[u8]) -> Result<(u32, u32), String> {
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let reader = dec.read_info().map_err(|_| crate::i18n::tr("non è un PNG valido", "not a valid PNG").to_string())?;
    let info = reader.info();
    let (w, h) = (info.width, info.height);
    if (w, h) == (64, 64) || (w, h) == (64, 32) {
        Ok((w, h))
    } else {
        Err(crate::tr!("la skin deve essere 64x64 o 64x32 pixel (questa è {}x{})", "the skin must be 64x64 or 64x32 pixels (this one is {}x{})", w, h))
    }
}

/// Skin scelta dall'utente (copiata nei dati dell'app).
pub fn custom_path(data_dir: &Path) -> PathBuf {
    data_dir.join("minecraft").join("skin.png")
}

/// PNG della skin in uso.
pub fn current(data_dir: &Path, choice: &str) -> Vec<u8> {
    if choice == "file" {
        if let Ok(b) = std::fs::read(custom_path(data_dir)) {
            if validate(&b).is_ok() {
                return b;
            }
        }
    }
    PRESET.to_vec()
}

/// Cartella `.minecraft` del launcher ufficiale.
pub fn dot_minecraft() -> Option<PathBuf> {
    let p = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/minecraft"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".minecraft"))
    }?;
    p.is_dir().then_some(p)
}

/// CustomSkinLoader è installato (la mod crea la sua cartella al primo avvio).
pub fn csl_dir(mc: &Path) -> Option<PathBuf> {
    let d = mc.join("CustomSkinLoader");
    d.is_dir().then_some(d)
}

/// Copia la skin per CustomSkinLoader (`LocalSkin/skins/<nome>.png`). Restituisce il file scritto.
pub fn csl_install(csl: &Path, name: &str, png: &[u8]) -> Result<PathBuf, String> {
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(16).collect();
    if name.is_empty() {
        return Err("nome".into());
    }
    let d = csl.join("LocalSkin").join("skins");
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let p = d.join(format!("{name}.png"));
    std::fs::write(&p, png).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(p)
}

/// Comando di SkinsRestorer da quello che ha scritto l'utente: un comando intero (`/skin url "…" slim`),
/// un URL (anche `skinsrestorer-axolotl://…` dalla pagina di caricamento) o il nome di un account premium.
pub fn skin_cmd(input: &str, slim: bool) -> Option<String> {
    let t = input.trim().replace(['\r', '\n'], " ");
    if t.is_empty() {
        return None;
    }
    let low = t.to_lowercase();
    if low.starts_with("/skin ") {
        return Some(t.chars().take(250).collect());
    }
    if low.starts_with("http://") || low.starts_with("https://") || low.starts_with("skinsrestorer-axolotl://") {
        if t.contains('"') || t.contains(' ') {
            return None;
        }
        return Some(format!("/skin url \"{t}\" {}", if slim { "slim" } else { "classic" }));
    }
    let ok = (3..=16).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| format!("/skin set {t}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut wr = e.write_header().unwrap();
            wr.write_image_data(&vec![0u8; (w * h * 4) as usize]).unwrap();
        }
        out
    }

    #[test]
    fn validates_skins() {
        assert_eq!(validate(PRESET), Ok((64, 64)), "la skin inclusa è giusta");
        assert_eq!(validate(&png(64, 32)), Ok((64, 32)));
        assert!(validate(&png(128, 128)).unwrap_err().contains("128x128"));
        assert!(validate(b"GIF89a....").is_err());
        assert!(validate(&[]).is_err());
    }

    #[test]
    fn skinsrestorer_commands() {
        assert_eq!(skin_cmd("Notch", false).as_deref(), Some("/skin set Notch"));
        assert_eq!(skin_cmd(" https://textures.minecraft.net/texture/abc ", true).as_deref(), Some("/skin url \"https://textures.minecraft.net/texture/abc\" slim"));
        assert_eq!(skin_cmd("skinsrestorer-axolotl://xyz", false).as_deref(), Some("/skin url \"skinsrestorer-axolotl://xyz\" classic"));
        assert_eq!(skin_cmd("/skin url \"https://x\" slim", false).as_deref(), Some("/skin url \"https://x\" slim"));
        assert_eq!(skin_cmd("", false), None);
        assert_eq!(skin_cmd("no spaces allowed", false), None);
        assert_eq!(skin_cmd("/op me", false), None, "solo /skin");
        assert_eq!(skin_cmd("https://a b", false), None);
    }

    #[test]
    fn copies_for_customskinloader() {
        let dir = std::env::temp_dir().join(format!("ae-mc-skin-{}", std::process::id()));
        let csl = dir.join("CustomSkinLoader");
        std::fs::create_dir_all(&csl).unwrap();
        assert_eq!(csl_dir(&dir), Some(csl.clone()));
        let p = csl_install(&csl, "Alter/Ego", PRESET).unwrap();
        assert!(p.ends_with("LocalSkin/skins/AlterEgo.png"));
        assert_eq!(std::fs::read(&p).unwrap(), PRESET);
        assert!(csl_install(&csl, "///", PRESET).is_err());
        // senza scelta "file" (o file rotto): la skin inclusa
        assert_eq!(current(&dir, "file"), PRESET);
        std::fs::create_dir_all(dir.join("minecraft")).unwrap();
        let other = png(64, 32);
        std::fs::write(custom_path(&dir), &other).unwrap();
        assert_eq!(current(&dir, "file"), other);
        assert_eq!(current(&dir, "alterego"), PRESET);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
