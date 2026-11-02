# Alter Ego Assistant

Assistente desktop personale per Windows 10/11, ispirato ad Alter Ego (Danganronpa).
Stack: Tauri 2 (Rust) + HTML/CSS/JS vanilla. Uso personale, non commerciale.

## Stato

| Fase | Contenuto | Stato |
|---|---|---|
| 1 | Scheletro: finestra, drag, resize, tray, hotkey, posizione | ✅ |
| 2 | Look CRT, sfondi, box dialogo | ✅ |
| 3 | Sprite e stati | ✅ da testare |
| 4 | Backend AI multi-provider | ✅ da testare |
| 5 | Personalità e memoria | – |
| 6 | Input testo e push-to-talk | – |
| 7 | Azioni PC e permessi | – |
| 8 | Audio e TTS | – |
| 9 | Rifinitura e build finale | – |

## Uso

- **Spostare**: trascina la finestra.
- **Ridimensionare**: trascina l'angolo ↘ oppure Ctrl+rotella. Il rapporto resta sempre 16:9.
- **Tray**: clic sinistro mostra/nasconde. Clic destro apre il menu: messaggio di prova, click-through, opacità, aspetto, reimposta posizione, esci.
- **Aspetto** (tray): sfondo (scanline / circuiti / entrambi), intensità sfondo, box dialogo (chiaro come lo screenshot / scuro), effetti animati.
- **Box dialogo**: clic sul box o Invio/Spazio completa la pagina, poi passa alla successiva. Il testo lungo va in pagine da 2 righe.
- **Scrivere ad Alter Ego**: `Ctrl+Alt+T` (o tray → *Scrivi ad Alter Ego…*), scrivi, Invio. Esc chiude.
- **Impostazioni** (tray → *Impostazioni…*): nome, tag, provider AI, chiavi, modelli, ordine di fallback, temperatura.
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

`%APPDATA%\AlterEgo\config.json`: opacità, hotkey, aspetto, posizione e dimensione per ogni monitor.

Campi modificabili a mano finché non c'è la schermata impostazioni:

| Campo | Significato | Default |
|---|---|---|
| `user_name` | Tuo nome, usato da Alter Ego | vuoto |
| `name_tag` | Tag nome nel box dialogo | `Alter Ego` |
| `effects` | Flicker e rumore animati (`false` = CPU minima) | `true` |

Al riavvio la finestra torna sull'ultimo monitor usato. Se quel monitor non c'è più, va sul monitor principale.

## Intelligenza artificiale

Provider: Gemini, OpenRouter, Groq, DeepSeek, Ollama. Tutti usano l'endpoint compatibile OpenAI, con streaming.

1. Apri *Impostazioni…* dalla tray.
2. Attiva i provider che vuoi e incolla le chiavi API. Le chiavi vanno nel Credential Manager di Windows (voce `AlterEgo`), mai in file o log.
3. Scegli il modello. *Carica modelli* scarica l'elenco dal provider. Per Ollama, campo vuoto = primo modello installato.
4. Ordina con ▲▼: il primo attivo risponde. Se fallisce (rete, chiave errata, rate limit, errore server) si passa al successivo.
5. *Prova connessione* verifica un singolo provider.

I modelli predefiniti (`gemini-2.5-flash`, `openrouter/auto`, `llama-3.3-70b-versatile`, `deepseek-chat`) potrebbero non esistere più: in quel caso usa *Carica modelli*.

Ogni tentativo ha timeout e 2 retry con attesa crescente per 429 ed errori 5xx.
Se nessun provider risponde, Alter Ego lo dice in personaggio con lo sprite `error`.

Le chiamate partono dal backend Rust: il frontend non vede mai le chiavi.

## Sprite

Origine e licenze: `docs/SOURCES.md`. Espressioni mancanti: `docs/MANCANTI.md`.

Per rigenerare gli sprite dal foglio originale (`pip install pillow numpy`):

```bash
python tools/process_sprites.py
```

## Test

I test del backend AI usano un server finto compatibile OpenAI su `127.0.0.1:18080`
(vedi `tools/mock_openai.py`).

```bash
python tools/mock_openai.py
```

```bash
cd src-tauri && cargo test
```

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
src-tauri/src/   main.rs, config.rs, window.rs, hotkeys.rs, tray.rs, keys.rs
src-tauri/src/providers/   mod.rs (fallback), openai_compat.rs (adapter), prompt.rs
src/             index.html, style.css, app.js, dialog.js, face.js, state.js, chat.js, sprites.js
src/             settings.html, settings.css, settings.js
src/assets/      font, immagini e sprite usati a runtime (sprite esclusi da git)
assets/          sorgenti originali (sprite raw, ecc.), esclusi da git
tools/           process_sprites.py, mock_openai.py, make_icon.py, build-windows.sh
docs/            SOURCES.md, MANCANTI.md
```

## Licenze di terze parti

- Font **DotGothic16**: SIL Open Font License 1.1 (`src/assets/fonts/OFL.txt`).
- Personaggio e immagini di Alter Ego: © Spike Chunsoft. Solo uso personale. Gli sprite non vanno in repo pubblici.
