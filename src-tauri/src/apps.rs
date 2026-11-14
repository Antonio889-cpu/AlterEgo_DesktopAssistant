//! Applicazioni principali aperte (solo i nomi, mai i titoli delle finestre): Alter Ego le sa senza
//! doverle chiedere. Il riepilogo va in fondo all'ultima domanda e non nel prompt di sistema, così il
//! prefisso (prompt + cronologia) resta identico e continua a essere letto dalla cache del provider.
//! La lista è ordinata e senza doppioni: cambia solo quando cambiano le app.

use std::collections::BTreeSet;

/// Processi che non sono "app" per l'utente (shell di sistema, servizi con finestre invisibili…).
const SKIP: &[&str] = &[
    "explorer", "applicationframehost", "textinputhost", "shellexperiencehost", "searchhost", "startmenuexperiencehost",
    "systemsettings", "lockapp", "dwm", "taskmgr", "ctfmon", "sihost", "widgets", "gamebar", "nvidia overlay", "rtkuwp",
    "alter-ego", "alter ego", "xdg-desktop-portal", "gnome-shell", "plasmashell", "xfdesktop", "nautilus-desktop", "kwin_x11",
    "kwin_wayland", "xfce4-panel", "lxpanel", "conky", "xterm-helper", "msedgewebview2",
];

/// Nomi leggibili per i programmi più comuni (il resto: nome dell'eseguibile).
fn friendly(exe: &str) -> String {
    let e = exe.to_lowercase();
    let known: &[(&str, &str)] = &[
        ("chrome", "Chrome"), ("msedge", "Edge"), ("firefox", "Firefox"), ("brave", "Brave"), ("opera", "Opera"),
        ("code", "Visual Studio Code"), ("devenv", "Visual Studio"), ("idea64", "IntelliJ IDEA"), ("pycharm64", "PyCharm"),
        ("discord", "Discord"), ("slack", "Slack"), ("teams", "Teams"), ("ms-teams", "Teams"), ("telegram", "Telegram"), ("whatsapp", "WhatsApp"),
        ("spotify", "Spotify"), ("steam", "Steam"), ("steamwebhelper", "Steam"), ("epicgameslauncher", "Epic Games"), ("obs64", "OBS Studio"), ("obs", "OBS Studio"),
        ("winword", "Word"), ("excel", "Excel"), ("powerpnt", "PowerPoint"), ("outlook", "Outlook"), ("olk", "Outlook"), ("onenote", "OneNote"),
        ("notepad", "Blocco note"), ("notepad++", "Notepad++"), ("windowsterminal", "Terminale"), ("wt", "Terminale"), ("cmd", "Prompt dei comandi"),
        ("powershell", "PowerShell"), ("ssms", "SQL Server Management Studio"), ("gimp-2.10", "GIMP"), ("gimp", "GIMP"), ("blender", "Blender"),
        ("photoshop", "Photoshop"), ("vlc", "VLC"), ("thunderbird", "Thunderbird"), ("libreoffice", "LibreOffice"), ("soffice.bin", "LibreOffice"),
        ("gnome-terminal-server", "Terminale"), ("konsole", "Konsole"), ("nautilus", "File"), ("dolphin", "Dolphin"), ("ollama app", "Ollama"),
    ];
    known.iter().find(|(k, _)| *k == e).map(|(_, v)| v.to_string()).unwrap_or_else(|| {
        let mut c = exe.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    })
}

/// (app aperte ordinate, app in primo piano). Nomi degli eseguibili senza estensione.
pub fn running() -> (Vec<String>, Option<String>) {
    let stem = |p: String| std::path::Path::new(&p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let (exes, fg) = imp::windows();
    let (exes, fg): (Vec<String>, _) = (exes.into_iter().map(stem).collect(), fg.map(stem));
    let keep = |e: &String| !e.is_empty() && !SKIP.contains(&e.to_lowercase().as_str());
    let set: BTreeSet<String> = exes.into_iter().filter(keep).map(|e| friendly(&e)).collect();
    let fg = fg.filter(keep).map(|e| friendly(&e));
    (set.into_iter().take(20).collect(), fg)
}

/// Percorso completo del primo processo con finestra il cui eseguibile si chiama come uno di `names`.
pub fn exe_path(names: &[&str]) -> Option<std::path::PathBuf> {
    imp::windows().0.into_iter().map(std::path::PathBuf::from).find(|p| {
        p.file_stem().is_some_and(|s| names.iter().any(|n| s.to_string_lossy().eq_ignore_ascii_case(n)))
    })
}

/// Finestre principali visibili: (percorso dell'eseguibile, titolo). Solo per riconoscere
/// alcuni giochi in locale: i titoli non vengono mai mandati al modello AI.
pub fn titled() -> Vec<(String, String)> {
    imp::titled()
}

/// Riga di contesto da aggiungere alla domanda (vuota se non si trova niente).
pub fn note(apps: &[String], fg: Option<&str>) -> String {
    if apps.is_empty() {
        return String::new();
    }
    let mut s = format!("\n\n[Contesto automatico, non scritto dall'utente — app aperte sul PC: {}", apps.join(", "));
    if let Some(f) = fg {
        s.push_str(&format!("; in primo piano: {f}"));
    }
    s.push(']');
    s
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, BOOL, HWND, LPARAM},
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
        UI::WindowsAndMessaging::*,
    };

    fn exe_of(hwnd: HWND) -> String {
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return String::new();
            }
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..len as usize])
        }
    }

    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<String>);
        // finestre principali visibili, con titolo, non "strumento" e non possedute da un'altra finestra
        if IsWindowVisible(hwnd) == 0 || GetWindowTextLengthW(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW != 0 {
            return 1;
        }
        out.push(exe_of(hwnd));
        1
    }

    unsafe extern "system" fn each_titled(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<(String, String)>);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }
        let n = GetWindowTextLengthW(hwnd);
        if n == 0 {
            return 1;
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        out.push((exe_of(hwnd), String::from_utf16_lossy(&buf[..got.max(0) as usize])));
        1
    }

    pub fn titled() -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = Vec::new();
        unsafe { EnumWindows(Some(each_titled), &mut v as *mut _ as LPARAM) };
        v
    }

    pub fn windows() -> (Vec<String>, Option<String>) {
        let mut v: Vec<String> = Vec::new();
        unsafe {
            EnumWindows(Some(each), &mut v as *mut _ as LPARAM);
            let fg = GetForegroundWindow();
            let f = if fg.is_null() { None } else { Some(exe_of(fg)) };
            (v, f)
        }
    }
}

/// Linux (X11): finestre visibili con xdotool, nome del processo da /proc.
#[cfg(not(windows))]
mod imp {
    use std::process::Command;

    fn comm(wid: &str) -> Option<String> {
        let out = Command::new("xdotool").args(["getwindowpid", wid]).output().ok()?;
        let pid = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if pid.is_empty() {
            return None;
        }
        std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
    }

    pub fn titled() -> Vec<(String, String)> {
        let Ok(out) = Command::new("xdotool").args(["search", "--onlyvisible", "--name", "."]).output() else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .take(80)
            .filter_map(|wid| {
                let name = Command::new("xdotool").args(["getwindowname", wid]).output().ok()?;
                let title = String::from_utf8_lossy(&name.stdout).trim().to_string();
                (!title.is_empty()).then(|| (comm(wid).unwrap_or_default(), title))
            })
            .collect()
    }

    pub fn windows() -> (Vec<String>, Option<String>) {
        let Ok(out) = Command::new("xdotool").args(["search", "--onlyvisible", "--name", "."]).output() else {
            return (Vec::new(), None);
        };
        let ids = String::from_utf8_lossy(&out.stdout).to_string();
        let mut v: Vec<String> = ids.lines().take(80).filter_map(comm).collect();
        v.dedup();
        let fg = Command::new("xdotool").arg("getactivewindow").output().ok().and_then(|o| comm(String::from_utf8_lossy(&o.stdout).trim()));
        (v, fg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_and_note() {
        assert_eq!(friendly("chrome"), "Chrome");
        assert_eq!(friendly("Code"), "Visual Studio Code");
        assert_eq!(friendly("keepassxc"), "Keepassxc");
        assert_eq!(note(&[], None), "");
        let n = note(&["Chrome".into(), "Discord".into()], Some("Chrome"));
        assert!(n.contains("app aperte sul PC: Chrome, Discord; in primo piano: Chrome]"), "{n}");
    }
}
