//! Rinomina il lato "registrazione" del cavo virtuale (VB-CABLE: "CABLE Output") in
//! "AlterEgo Link Audio", come farebbe Pannello di controllo → Audio → Proprietà → nome.
//!
//! Scrivere le proprietà di un endpoint audio richiede i diritti di amministratore: l'app
//! si riavvia elevata con `--rename-link-device` (vedi main.rs), fa solo questo ed esce.

use super::LINK_NAME;
use windows::{
    core::PCWSTR,
    Win32::{
        Devices::FunctionDiscovery::{PKEY_Device_DeviceDesc, PKEY_Device_FriendlyName},
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        Media::Audio::{eCapture, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE, EDataFlow},
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ, STGM_READWRITE},
            Com::StructuredStorage::PROPVARIANT,
            Threading::{GetExitCodeProcess, WaitForSingleObject},
        },
        UI::{
            Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
            WindowsAndMessaging::SW_HIDE,
        },
    },
};

pub const ARG: &str = "--rename-link-device";
/// Nome per il lato "riproduzione" del cavo: chiaro che non va usato come altoparlante.
pub const RENDER_NAME: &str = "AlterEgo Link (uso interno)";

fn rename_flow(en: &IMMDeviceEnumerator, flow: EDataFlow, matches: &str, new_name: &str) -> windows::core::Result<u32> {
    let mut n = 0;
    unsafe {
        let col = en.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        for i in 0..col.GetCount()? {
            let dev = col.Item(i)?;
            let store = dev.OpenPropertyStore(STGM_READ)?;
            let desc = store.GetValue(&PKEY_Device_DeviceDesc)?.to_string();
            let friendly = store.GetValue(&PKEY_Device_FriendlyName)?.to_string();
            let is_cable = friendly.contains("VB-Audio Virtual Cable") || friendly.contains("VB-Audio Cable");
            if is_cable && (desc.eq_ignore_ascii_case(matches) || desc == new_name) {
                if desc != new_name {
                    let w = dev.OpenPropertyStore(STGM_READWRITE)?;
                    w.SetValue(&PKEY_Device_DeviceDesc, &PROPVARIANT::from(new_name))?;
                    w.Commit()?;
                }
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Eseguita nel processo elevato. Codice d'uscita: 0 = ok, 2 = cavo non trovato, 1 = errore.
pub fn run_elevated() -> i32 {
    let r = (|| -> windows::core::Result<u32> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let n = rename_flow(&en, eCapture, "CABLE Output", LINK_NAME)?;
            let _ = rename_flow(&en, eRender, "CABLE Input", RENDER_NAME);
            Ok(n)
        }
    })();
    match r {
        Ok(0) => 2,
        Ok(_) => 0,
        Err(e) => {
            eprintln!("rinomina non riuscita: {e}");
            1
        }
    }
}

/// Chiede l'elevazione (finestra UAC) e attende il risultato.
pub fn request() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (verb, file, params) = (wide("runas"), wide(&exe.to_string_lossy()), wide(ARG));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|_| "permesso di amministratore negato".to_string())?;
        if info.hProcess.is_invalid() {
            return Err("processo di rinomina non avviato".into());
        }
        let wait = WaitForSingleObject(info.hProcess, 30_000);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if wait != WAIT_OBJECT_0 {
            return Err("la rinomina non ha risposto in tempo".into());
        }
        match code {
            0 => Ok(()),
            2 => Err("cavo VB-CABLE non trovato: installalo e riprova".into()),
            _ => Err("rinomina non riuscita".into()),
        }
    }
}
