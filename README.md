# Alter Ego Assistant

Assistente desktop personale per Windows 10/11, ispirato ad Alter Ego (Danganronpa).
Stack: Tauri 2 (Rust) + HTML/CSS/JS vanilla. Uso personale, non commerciale.

## Stato

| Fase | Contenuto | Stato |
|---|---|---|
| 1 | Scheletro: finestra, drag, resize, tray, hotkey, posizione | ✅ da testare |
| 2 | Look CRT, sfondi, box dialogo | – |
| 3 | Sprite e stati | – |
| 4 | Backend AI multi-provider | – |
| 5 | Personalità e memoria | – |
| 6 | Input testo e push-to-talk | – |
| 7 | Azioni PC e permessi | – |
| 8 | Audio e TTS | – |
| 9 | Rifinitura e build finale | – |

## Uso (fase 1)

- **Spostare**: trascina la finestra.
- **Ridimensionare**: trascina l'angolo ↘ oppure Ctrl+rotella. Il rapporto resta sempre 16:9.
- **Tray**: clic sinistro mostra/nasconde. Clic destro apre il menu (click-through, opacità, reimposta posizione, esci).
- **Alt+F4** nasconde la finestra. Per uscire usa *Esci* dalla tray.
- **Click-through**: il mouse passa attraverso la finestra. Per disattivarlo passa sul lucchetto in alto a destra e cliccalo, oppure usa la tray o la hotkey.

### Hotkey predefinite

| Azione | Combinazione |
|---|---|
| Mostra/nascondi | `Ctrl+Alt+A` |
| Parla (push-to-talk) | `Ctrl+Alt+Space` |
| Input testo | `Ctrl+Alt+T` |
| Click-through | `Ctrl+Alt+C` |

Per cambiarle modifica `hotkeys` in `%APPDATA%\AlterEgo\config.json` e riavvia l'app.
Una stringa vuota disattiva la hotkey. La schermata impostazioni arriva in fase 9.

### Dati salvati

`%APPDATA%\AlterEgo\config.json`: opacità, hotkey, posizione e dimensione per ogni monitor.
Al riavvio la finestra torna sull'ultimo monitor usato. Se quel monitor non c'è più, va sul monitor principale.

## Build

### Su Windows

Prerequisiti: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) LTS, Visual Studio Build Tools con "Sviluppo di applicazioni desktop con C++". WebView2 è già presente in Windows 10/11.

```bash
npm install
```

```bash
npm run dev
```

```bash
npm run build
```

L'eseguibile finisce in `src-tauri/target/release/`. L'installer finisce in `src-tauri/target/release/bundle/nsis/`.

### Cross-build da Linux

Richiede `mingw-w64`, `nsis` e `rustup target add x86_64-pc-windows-gnu`.

```bash
tools/build-windows.sh
```

Con il target GNU, `WebView2Loader.dll` deve stare accanto a `alter-ego.exe`. L'installer la include già.

## Struttura

```
src-tauri/src/   main.rs, config.rs, window.rs, hotkeys.rs, tray.rs
src/             index.html, style.css, app.js
tools/           make_icon.py (icona provvisoria), build-windows.sh
```
