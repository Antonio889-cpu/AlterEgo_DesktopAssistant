//! Avatar senza login: VRChat scrive un file di configurazione OSC per ogni avatar usato,
//! `%USERPROFILE%\AppData\LocalLow\VRChat\VRChat\OSC\usr_*\Avatars\avtr_*.json`, con id, nome e
//! parametri (indirizzo e tipo; "input" se si può scrivere via OSC, "output" se VRChat lo manda).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    /// "Bool", "Int" o "Float".
    pub ty: String,
    /// Si può scrivere da OSC (parametro dell'avatar, non incorporato).
    pub input: bool,
    /// VRChat lo manda quando cambia.
    pub output: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Avatar {
    pub id: String,
    pub name: String,
    /// Cartella `usr_…` (l'account che l'ha usato).
    pub user: String,
    pub params: Vec<Param>,
    /// Ultima modifica (secondi Unix): l'avatar usato per ultimo sta in cima.
    pub mtime: u64,
}

#[derive(Deserialize)]
struct RawEnd {
    #[serde(default, rename = "type")]
    ty: String,
}

#[derive(Deserialize)]
struct RawParam {
    #[serde(default)]
    name: String,
    input: Option<RawEnd>,
    output: Option<RawEnd>,
}

#[derive(Deserialize)]
struct RawAvatar {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    parameters: Vec<RawParam>,
}

/// Legge un file di configurazione OSC (VRChat lo scrive con il BOM UTF-8).
pub fn parse(raw: &str) -> Option<Avatar> {
    let r: RawAvatar = serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()?;
    if !r.id.starts_with("avtr_") {
        return None;
    }
    let params = r
        .parameters
        .into_iter()
        .filter(|p| !p.name.trim().is_empty())
        .map(|p| {
            let ty = p.input.as_ref().or(p.output.as_ref()).map(|e| e.ty.clone()).unwrap_or_default();
            Param { name: p.name, ty, input: p.input.is_some(), output: p.output.is_some() }
        })
        .collect();
    Some(Avatar { name: if r.name.trim().is_empty() { r.id.clone() } else { r.name }, id: r.id, user: String::new(), params, mtime: 0 })
}

/// Cartelle `OSC` possibili: impostazione, Windows, Proton (Steam su Linux).
pub fn osc_dirs(custom: &str) -> Vec<PathBuf> {
    let c = custom.trim();
    if !c.is_empty() {
        return vec![PathBuf::from(c)];
    }
    let mut v = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from) {
        v.push(home.join("AppData").join("LocalLow").join("VRChat").join("VRChat").join("OSC"));
        for steam in [".steam/steam", ".local/share/Steam"] {
            v.push(home.join(steam).join("steamapps/compatdata/438100/pfx/drive_c/users/steamuser/AppData/LocalLow/VRChat/VRChat/OSC"));
        }
    }
    v
}

/// Tutti gli avatar trovati (il più recente per primo; un id compare una volta sola).
pub fn list(dirs: &[PathBuf]) -> Vec<Avatar> {
    let mut out: Vec<Avatar> = Vec::new();
    for d in dirs {
        let Ok(users) = std::fs::read_dir(d) else { continue };
        for u in users.flatten() {
            let uname = u.file_name().to_string_lossy().to_string();
            if !uname.starts_with("usr_") {
                continue;
            }
            let Ok(files) = std::fs::read_dir(u.path().join("Avatars")) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Some(mut a) = read(&p) {
                    a.user = uname.clone();
                    out.push(a);
                }
            }
        }
    }
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime).then(a.name.cmp(&b.name)));
    let mut seen = std::collections::HashSet::new();
    out.retain(|a| seen.insert(a.id.clone()));
    out
}

fn read(p: &Path) -> Option<Avatar> {
    // file enormi: non sono configurazioni OSC
    if std::fs::metadata(p).ok()?.len() > 2_000_000 {
        return None;
    }
    let mut a = parse(&std::fs::read_to_string(p).ok()?)?;
    a.mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    Some(a)
}

/// Avatar con questo id (dopo /avatar/change).
pub fn find(dirs: &[PathBuf], id: &str) -> Option<Avatar> {
    list(dirs).into_iter().find(|a| a.id == id)
}

/// Collegamenti automatici parametro → senso del cervello di moscerino, dai nomi più comuni dei
/// contatti e dei PhysBones (carezze, "boop", vicinanza, schiaffi, coda tirata).
pub fn auto_senses(params: &[Param]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for p in params {
        let n = p.name.to_lowercase();
        let side = if n.ends_with("_l") || n.contains("left") { "loom_l" } else if n.ends_with("_r") || n.contains("right") { "loom_r" } else { "loom_l" };
        let sense = if ["hit", "slap", "punch", "kick", "hurt", "pain", "smack"].iter().any(|k| n.contains(k)) || n.ends_with("_stretch") {
            "pain"
        } else if ["pat", "boop", "touch", "poke", "hug", "pet", "isgrabbed", "tickle", "nose"].iter().any(|k| n.contains(k)) {
            "touch"
        } else if ["proxim", "near", "close"].iter().any(|k| n.contains(k)) {
            side
        } else if ["food", "cookie", "candy", "sweet", "cake"].iter().any(|k| n.contains(k)) {
            "sweet"
        } else {
            continue;
        };
        // solo ciò che VRChat ci manda (i contatti e i PhysBones lo fanno sempre)
        if p.output && p.ty != "Int" {
            out.push((p.name.clone(), sense.to_string()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("fixtures/avtr_test.json");

    #[test]
    fn parses_osc_config() {
        assert!(SAMPLE.starts_with('\u{feff}'), "il fixture ha il BOM come i file veri");
        let a = parse(SAMPLE).unwrap();
        assert_eq!(a.id, "avtr_c38a1615-5bf5-42b4-84eb-a8b6c37cbd11");
        assert_eq!(a.name, "Alter Ego (Chihiro)");
        assert_eq!(a.params.len(), 13);
        let v = a.params.iter().find(|p| p.name == "VelocityZ").unwrap();
        assert_eq!((v.ty.as_str(), v.input, v.output), ("Float", false, true));
        let e = a.params.iter().find(|p| p.name == "VRCEmote").unwrap();
        assert_eq!((e.ty.as_str(), e.input, e.output), ("Int", true, true));
        assert!(parse("{\"id\":\"usr_x\",\"name\":\"n\"}").is_none(), "non è un avatar");
        assert!(parse("non json").is_none());
        assert_eq!(parse("{\"id\":\"avtr_1\"}").unwrap().name, "avtr_1");
        // sensi automatici
        let s = auto_senses(&a.params);
        let get = |k: &str| s.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str());
        assert_eq!(get("HeadPat"), Some("touch"));
        assert_eq!(get("Contact_Boop"), Some("touch"));
        assert_eq!(get("Proximity_L"), Some("loom_l"));
        assert_eq!(get("Proximity_R"), Some("loom_r"));
        assert_eq!(get("Hit_Slap"), Some("pain"));
        assert_eq!(get("Tail_IsGrabbed"), Some("touch"));
        assert_eq!(get("Tail_Stretch"), Some("pain"));
        assert_eq!(get("VelocityZ"), None);
        assert_eq!(get("Expression"), None);
    }

    #[test]
    fn lists_avatars_from_osc_folder() {
        let dir = std::env::temp_dir().join(format!("ae-vrc-osc-{}", std::process::id()));
        let av = dir.join("usr_1234").join("Avatars");
        std::fs::create_dir_all(&av).unwrap();
        std::fs::write(av.join("avtr_c38a1615-5bf5-42b4-84eb-a8b6c37cbd11.json"), SAMPLE).unwrap();
        std::fs::write(av.join("avtr_2.json"), "{\"id\":\"avtr_2\",\"name\":\"Second\",\"parameters\":[]}").unwrap();
        std::fs::write(av.join("broken.json"), "{").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let l = list(&[dir.clone(), dir.join("missing")]);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|a| a.user == "usr_1234" && a.mtime > 0));
        assert_eq!(find(&[dir.clone()], "avtr_2").unwrap().name, "Second");
        assert!(find(&[dir.clone()], "avtr_404").is_none());
        assert_eq!(osc_dirs("C:/x"), vec![PathBuf::from("C:/x")]);
        assert!(osc_dirs("").iter().any(|p| p.ends_with("VRChat/VRChat/OSC") || p.to_string_lossy().contains("OSC")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
