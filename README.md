# Alter Ego Assistant

Assistente desktop personale per Windows 10/11, ispirato ad Alter Ego (Danganronpa).
Stack: Tauri 2 (Rust) + HTML/CSS/JS vanilla. Uso personale, non commerciale.

## Stato

| Fase | Contenuto | Stato |
|---|---|---|
| 1 | Scheletro: finestra, drag, resize, tray, hotkey, posizione | ✅ |
| 2 | Look CRT, sfondi, box dialogo | ✅ |
| 3 | Sprite e stati | ✅ |
| 4 | Backend AI multi-provider, più chiavi per servizio | ✅ |
| 5 | Personalità e memoria | ✅ |
| 6 | Input testo e push-to-talk (tastiera o mouse) | ✅ |
| 7 | Azioni PC, permessi, conferme, registro; clip vocali | ✅ |
| 8 | Blip del dialogo e voce (TTS) | ✅ |
| 9 | Impostazioni complete, avvio con Windows, build | ✅ |

Verificato su Linux (build, test automatici, prove nell'app); su Windows va provato da te (vedi *Da verificare su Windows*).

## Uso

- **Spostare**: trascina la finestra. **Ridimensionare**: angolo ↘ o Ctrl+rotella (sempre 16:9).
- **Scrivere**: `Ctrl+Alt+T` (o tray → *Scrivi ad Alter Ego…*), Invio invia, Esc chiude.
- **Parlare**: tieni premuto il tasto push-to-talk (default `Ctrl+Alt+Spazio`), parla, rilascia.
- **Box dialogo**: clic o Invio/Spazio per andare avanti.
- **Tray**: clic sinistro mostra/nasconde; clic destro: impostazioni, click-through, opacità, aspetto, suoni, voce, esci.
- **Alt+F4** nasconde. Per uscire: tray → *Esci*.
- **Click-through**: il mouse passa attraverso Alter Ego; per disattivarlo clicca il lucchetto in alto a destra, o tray/hotkey.

## Tasti

Si cambiano in *Impostazioni → Tasti*: clicca il campo e premi la combinazione, anche un tasto singolo (es. `F8`).
Per il **push-to-talk** puoi usare anche il **tasto centrale** o i **tasti laterali del mouse** (`Mouse4`, `Mouse5`).

| Azione | Default |
|---|---|
| Mostra/nascondi | `Ctrl+Alt+A` |
| Push-to-talk | `Ctrl+Alt+Space` |
| Scrivi | `Ctrl+Alt+T` |
| Click-through | `Ctrl+Alt+C` |

## Intelligenza artificiale

Provider: Gemini, OpenRouter, Groq, DeepSeek, Ollama (endpoint compatibili OpenAI, streaming).

- **Più chiavi per servizio**: in *Impostazioni → AI e chiavi* aggiungi quante chiavi vuoi.
  Se una risponde 401/402/403/429 o "quota", si passa subito alla successiva; la prossima volta si riparte dall'ultima che ha funzionato.
- **Fallback tra servizi**: ordine con ▲▼. Se un servizio non risponde (rete, errore server, nessuna chiave valida) passa al successivo.
- Timeout e 2 retry con attesa crescente sugli errori temporanei. Se nessuno risponde, Alter Ego si scusa con lo sprite `error`.
- Chiavi nel **Credential Manager di Windows** (`AlterEgo / api-key:<servizio>:<id>`), mai in file o log; il frontend non le vede.
- I modelli predefiniti potrebbero non esistere più: usa *Carica modelli*.
- Le chiavi singole delle versioni precedenti vengono spostate automaticamente come "Chiave 1".

## Personalità e memoria

- Scheda personaggio: `%APPDATA%\AlterEgo\persona\alter_ego.md` (modificabile, vale subito). Righe `>` = note non inviate al modello.
- Cursori timida ↔ sicura e formale ↔ affettuosa, modalità imitazione, lingua (IT / IT+EN / EN).
- Memoria SQLite in `%APPDATA%\AlterEgo\memory.db`: cronologia, riassunti automatici, fatti su di te, promemoria.
  Da *Impostazioni → Memoria*: vedi/aggiungi/cancella fatti, esporta JSON, dimentica tutto.
- Esce dal PC solo ciò che va al servizio AI scelto: prompt di sistema e ultimi messaggi (e l'audio, al servizio di trascrizione).

## Voce

- **Riconoscimento (STT)**: Groq Whisper (usa le chiavi Groq, con rotazione) o un server Whisper locale compatibile OpenAI.
  Se non sente nulla: Windows → Privacy e sicurezza → Microfono → "Consenti alle app desktop di accedere al microfono".
- **Voce di Alter Ego (TTS)**: voci di Windows, oppure un servizio compatibile OpenAI `/audio/speech` (es. Kokoro o Piper locali).
  Per una voce italiana di Windows: Impostazioni → Ora e lingua → Voce → Aggiungi voci.
  Mentre parla il volto si anima (con lo sprite `talk_closed` sarebbe bocca aperta/chiusa: vedi `docs/MANCANTI.md`).
- **Blip del dialogo**: stile Danganronpa (ricreato con sintesi, nessun file originale), blip morbidi, o muto.

## Clip vocali

Di' col push-to-talk (o scrivi) una frase come **"Alter Ego clippa"** o **"Alter Ego clip that"**:
Alter Ego preme la scorciatoia del tuo programma di registrazione **senza passare dal modello AI** (istantaneo, gratis).

- Frasi e programmi in *Impostazioni → Clip*. Preimpostati (da attivare e verificare nel tuo programma):
  AMD Adrenalin `Ctrl+Shift+S`, NVIDIA App `Alt+F10`, Medal `F8`.
- La registrazione in background del programma (Instant Replay ecc.) deve essere attiva.
- La frase scatta solo se è quasi tutto il messaggio (così "come si fa a clippare…?" va al modello).
- *Prova* invia la scorciatoia dopo 3 secondi, per controllare dal gioco.

## Azioni sul PC

Tutte spente di default; dettagli in [`docs/PERMESSI.md`](docs/PERMESSI.md): aprire programmi/siti, file in cartelle autorizzate,
promemoria, appunti, screenshot, volume/luminosità, comandi in lista. Conferma per quelle sensibili, tutto nel Registro.

## Avvio con Windows

Spento di default; si attiva in *Impostazioni → Generale*.

## Da verificare su Windows

Questo progetto è stato sviluppato e testato su Linux (compilazione per Windows inclusa). Da provare su Windows 10/11:

- tasti simulati per le clip (AMD/NVIDIA/Medal) e push-to-talk col mouse;
- microfono, voci di Windows, notifiche dei promemoria, avvio con Windows;
- screenshot, volume, luminosità;
- criteri di prestazioni: < 150 MB di RAM a riposo, CPU ≈ 0%, avvio < 2 s
  (per CPU minima spegni *Effetti animati*).

## Sprite

Origine e licenze: `docs/SOURCES.md`. Espressioni mancanti: `docs/MANCANTI.md`.
Gli sprite **non sono nella repo**: metti il foglio in `assets/sprites/raw/alter_ego_sheet.png` e lancia

```bash
python tools/process_sprites.py
```

## Test

```bash
python tools/mock_openai.py
```

```bash
cd src-tauri && cargo test
```

## Build

### Su Windows

Prerequisiti: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) LTS, Visual Studio Build Tools ("Sviluppo di applicazioni desktop con C++").

```bash
npm install
```

```bash
npm run build
```

Exe in `src-tauri/target/release/`, installer in `src-tauri/target/release/bundle/nsis/`.

### Cross-build da Linux

Richiede `mingw-w64`, `nsis` e `rustup target add x86_64-pc-windows-gnu`.

```bash
tools/build-windows.sh
```

Con il target GNU, `WebView2Loader.dll` deve stare accanto a `alter-ego.exe` (l'installer la include).

## Struttura

```
src-tauri/src/            main.rs, config.rs, window.rs, hotkeys.rs, tray.rs, keys.rs, assistant.rs, stt.rs
src-tauri/src/providers/  mod.rs (fallback + rotazione chiavi), openai_compat.rs, prompt.rs
src-tauri/src/memory/     mod.rs (SQLite), summarize.rs
src-tauri/src/actions/    mod.rs (strumenti, permessi, conferme, registro), clip.rs, input.rs, mousehook.rs, screen.rs
src/                      index.html, style.css, app.js, dialog.js, face.js, state.js, chat.js, audio.js, sprites.js
src/                      settings.html, settings.css, settings.js
persona/                  alter_ego.md
tools/                    process_sprites.py, mock_openai.py, make_icon.py, build-windows.sh
docs/                     SOURCES.md, MANCANTI.md, PERMESSI.md
```

Punti di innesto per codice esterno (es. CyberAI): `providers/`, `memory/`, `actions/`.

## Licenze di terze parti

- Font **DotGothic16**: SIL Open Font License 1.1 (`src/assets/fonts/OFL.txt`).
- Personaggio e immagini di Alter Ego: © Spike Chunsoft. Solo uso personale. Gli sprite non vanno in repo pubblici e le build che li contengono non vanno distribuite.
