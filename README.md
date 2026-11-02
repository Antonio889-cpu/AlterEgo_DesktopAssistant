# Alter Ego – Desktop Assistant

Assistente desktop per **Windows 10/11** ispirato ad *Alter Ego* di Danganronpa: una finestrella in stile laptop CRT,
sempre in primo piano, con cui parli a voce o per iscritto. Risponde con il proprio carattere, ricorda chi sei,
può fare clip dei tuoi giochi con una frase e, se glielo permetti, eseguire piccole azioni sul PC.

Progetto personale e non commerciale. Interfaccia e documentazione in italiano.

![Alter Ego sul desktop](docs/screenshots/main.png)

> Negli screenshot c'è il **volto segnaposto** dell'app: gli sprite originali di Alter Ego sono © Spike Chunsoft
> e non sono inclusi né nella repo né nella release. Puoi importare i tuoi dall'app (vedi [Sprite](#sprite)).

## Indice

- [Funzionalità](#funzionalità)
- [Screenshot](#screenshot)
- [Installazione](#installazione)
- [Primo avvio](#primo-avvio)
- [Uso](#uso)
- [Intelligenza artificiale e chiavi API](#intelligenza-artificiale-e-chiavi-api)
- [Voce](#voce)
- [Clip vocali](#clip-vocali)
- [Azioni sul PC e permessi](#azioni-sul-pc-e-permessi)
- [Personalità e memoria](#personalità-e-memoria)
- [Sprite](#sprite)
- [Privacy e sicurezza](#privacy-e-sicurezza)
- [Dove stanno i dati](#dove-stanno-i-dati)
- [Risoluzione problemi](#risoluzione-problemi)
- [Sviluppo](#sviluppo)
- [Limiti noti](#limiti-noti)
- [Crediti e licenze](#crediti-e-licenze)

## Funzionalità

- **Finestra in stile Alter Ego**: schermo verde con scanline, cornice con glow, logo, box di dialogo pixel con effetto macchina da scrivere e ▼.
  Senza bordi, trasparente, sempre in primo piano, fuori dalla taskbar; si sposta, si ridimensiona (16:9) e ricorda la posizione per ogni monitor.
- **Click-through** opzionale: il mouse passa attraverso la finestra, che non ruba mai il focus mentre giochi.
- **5 servizi AI** (Gemini, OpenRouter, Groq, DeepSeek, Ollama locale) con **fallback automatico** e **più chiavi API per servizio** a rotazione.
- **Push-to-talk** con trascrizione Whisper (Groq o server locale), su tastiera o **tasti laterali del mouse**.
- **Voce** di Alter Ego (voci di Windows o servizio compatibile OpenAI) e **blip** del dialogo.
- **Clip vocali**: "Alter Ego clippa" → salva la clip con AMD Adrenalin, NVIDIA App o Medal, **senza passare dall'AI**.
- **Azioni sul PC** tramite tool calling, tutte spente di default, con conferma per quelle sensibili e registro consultabile.
- **Memoria locale** (SQLite): ricorda le conversazioni, fa riassunti, impara fatti su di te.
- **Personalità regolabile** (timida ↔ sicura, formale ↔ affettuosa), modalità imitazione, risposte in italiano, inglese o entrambe.
- Espressioni del volto pilotate dalle risposte (felice, sorpresa, imbarazzata, triste, preoccupata, pensierosa, errore).
- Leggera: Tauri 2 (Rust) + HTML/CSS/JS vanilla, installer di circa 5 MB.

## Screenshot

| | |
|---|---|
| ![Dialogo con box scuro e sfondo a circuiti](docs/screenshots/happy-dark.png) | ![Push-to-talk: in ascolto](docs/screenshots/listening.png) |
| Box scuro e sfondo a circuiti | Push-to-talk: in ascolto |
| ![Richiesta di permesso](docs/screenshots/confirm.png) | ![Input di testo](docs/screenshots/input.png) |
| Conferma prima di un'azione sul PC | Scrivere ad Alter Ego (`Ctrl+Alt+T`) |

**Impostazioni**

| | |
|---|---|
| ![AI e chiavi](docs/screenshots/settings-ai.png) | ![Clip vocali](docs/screenshots/settings-clip.png) |
| Servizi AI, più chiavi, ordine di fallback | Clip vocali |
| ![Permessi](docs/screenshots/settings-perms.png) | ![Tasti](docs/screenshots/settings-hotkeys.png) |
| Permessi delle azioni | Tasti rapidi (anche mouse) |
| ![Voce e audio](docs/screenshots/settings-voice.png) | ![Personalità e sprite](docs/screenshots/settings-persona.png) |
| Voce e suoni | Personalità e importazione sprite |

## Installazione

1. Scarica dalla pagina [Releases](../../releases) l'**installer** `AlterEgo_x.y.z_setup.exe`, oppure lo **zip portable**
   (estrai e avvia `alter-ego.exe`; tieni `WebView2Loader.dll` nella stessa cartella).
2. Windows SmartScreen può avvisare che l'app non è firmata: *Ulteriori informazioni → Esegui comunque*.
3. Serve **WebView2**, già presente in Windows 10/11 aggiornati.

L'app compare nella **system tray** (vicino all'orologio), non nella taskbar.

## Primo avvio

1. Clic destro sull'icona nella tray → **Impostazioni…**
2. **Generale**: scrivi il tuo nome.
3. **AI e chiavi**: attiva almeno un servizio e incolla una chiave API (ad esempio [Groq](https://console.groq.com/keys)
   o [Google AI Studio](https://aistudio.google.com/apikey)), poi *Prova connessione*. Con Groq funziona anche la voce.
   In alternativa usa [Ollama](https://ollama.com) in locale, senza chiavi.
4. **Personalità → Sprite**: importa il foglio sprite, se ce l'hai. Senza sprite Alter Ego usa il volto segnaposto.
5. Premi `Ctrl+Alt+T` e scrivi, oppure tieni premuto `Ctrl+Alt+Spazio` e parla.

## Uso

| Cosa | Come |
|---|---|
| Scrivere | `Ctrl+Alt+T` (o tray → *Scrivi ad Alter Ego…*), Invio invia, Esc chiude |
| Parlare | tieni premuto il push-to-talk (default `Ctrl+Alt+Spazio`), parla, rilascia |
| Andare avanti nel dialogo | clic sul box, Invio o Spazio |
| Spostare / ridimensionare | trascina / angolo ↘ o `Ctrl` + rotella |
| Mostrare / nascondere | `Ctrl+Alt+A` o clic sinistro sull'icona della tray |
| Click-through | `Ctrl+Alt+C` o tray; per uscirne clicca il lucchetto in alto a destra |
| Uscire | tray → *Esci* (Alt+F4 nasconde soltanto) |

Tutti i tasti si cambiano in **Impostazioni → Tasti**: clicca il campo e premi la combinazione (anche un tasto singolo come `F8`).
Per il push-to-talk vanno bene anche il **tasto centrale** e i **tasti laterali del mouse** (`Mouse4`, `Mouse5`).

Dalla tray cambi anche opacità, sfondo (scanline, circuiti, entrambi), intensità, stile del box (chiaro o scuro), effetti animati, suoni e voce.

## Intelligenza artificiale e chiavi API

| Servizio | Note |
|---|---|
| Google Gemini | endpoint compatibile OpenAI |
| OpenRouter | qualsiasi modello del catalogo |
| Groq | velocissimo; le sue chiavi servono anche per Whisper (voce) |
| DeepSeek | |
| Ollama | locale, nessuna chiave, nessun dato esce dal PC |

- **Più chiavi per servizio**: se una chiave risponde 401, 402, 403, 429 o "quota", si passa subito alla successiva.
  La volta dopo si riparte dall'ultima che ha funzionato.
- **Fallback**: l'ordine dei servizi si cambia con ▲▼. Se uno non risponde (rete, errore server, nessuna chiave valida) prova il successivo.
- Timeout e 2 tentativi con attesa crescente sugli errori temporanei. Se nessuno risponde, Alter Ego lo dice con l'espressione di errore.
- *Carica modelli* scarica l'elenco dei modelli dal servizio: i nomi predefiniti potrebbero non esistere più.
- Le chiavi stanno nel **Gestore credenziali di Windows** (voce `AlterEgo`), mai in file o log, e il frontend non le vede mai.

## Voce

- **Riconoscimento (push-to-talk)**: Groq Whisper (`whisper-large-v3-turbo`) oppure un server Whisper locale compatibile OpenAI
  (es. faster-whisper-server/speaches, whisper.cpp server). *Prova microfono* registra 4 secondi e mostra cosa ha capito.
- **Voce di Alter Ego (TTS)**: voci di Windows (per l'italiano: Impostazioni di Windows → Ora e lingua → Voce → Aggiungi voci)
  oppure un servizio compatibile OpenAI `/audio/speech` (es. Kokoro o Piper locali). Mentre parla, il volto si anima.
- **Blip del dialogo**: stile Danganronpa (ricreato con sintesi audio, nessun file originale), blip morbidi, o muto.

## Clip vocali

Di' col push-to-talk, o scrivi, **"Alter Ego clippa"** o **"Alter Ego clip that"**: Alter Ego preme la scorciatoia del tuo
programma di registrazione. È **istantaneo e gratis**, perché non passa dal modello AI.

| Programma | Scorciatoia preimpostata | Funzione da attivare nel programma |
|---|---|---|
| AMD Software: Adrenalin | `Ctrl+Shift+S` | Instant Replay |
| NVIDIA App / ShadowPlay | `Alt+F10` | Instant Replay |
| Medal | `F8` | Clip in background |

- In **Impostazioni → Clip** attivi i programmi che usi, controlli che la scorciatoia sia uguale a quella del programma,
  modifichi le frasi (una per riga) e la risposta di Alter Ego.
- La frase scatta solo se è quasi tutto il messaggio: "come si fa a clippare?" va normalmente all'AI.
- *Prova* invia la scorciatoia dopo 3 secondi, così hai il tempo di tornare al gioco.

## Azioni sul PC e permessi

Tutto è **spento di default** e si attiva in **Impostazioni → Permessi**. Alter Ego vede solo gli strumenti che attivi.
Le azioni sensibili (⚠) chiedono conferma ogni volta: il pulsante selezionato è *Nega*, e senza risposta in 60 secondi la risposta è no.

| Funzione | Conferma |
|---|---|
| Aprire programmi e siti (da una lista con nomi tuoi) | ⚠ solo fuori lista |
| Gestione file, solo in cartelle autorizzate | ⚠ sempre |
| Promemoria, timer, sveglie (notifiche di Windows) | no |
| Appunti | ⚠ per leggere |
| Lettura schermo (screenshot a un modello con visione) | ⚠ sempre |
| Volume e luminosità | no |
| Comandi, solo da una lista e scelti per nome | ⚠ sempre |

Ogni azione, anche negata, finisce in **Impostazioni → Registro**. Dettagli tecnici in [docs/PERMESSI.md](docs/PERMESSI.md).

## Personalità e memoria

- **Scheda del personaggio**: `%APPDATA%\AlterEgo\persona\alter_ego.md`. Modificala come vuoi: vale dal messaggio successivo.
  Le righe che iniziano con `>` sono note e non vengono inviate al modello.
- **Personalità**: cursori timida ↔ sicura e formale ↔ affettuosa, **modalità imitazione** (scrivi chi imitare),
  lingua delle risposte: solo italiano, italiano + inglese, solo inglese.
- **Memoria** (SQLite, solo sul tuo PC):
  - a ogni messaggio partono gli ultimi messaggi, un riassunto e i fatti che sa su di te;
  - ogni tanto riassume le conversazioni vecchie e impara fatti duraturi su di te;
  - in *Impostazioni → Memoria* vedi, aggiungi o cancelli i fatti, esporti tutto in JSON o premi *Dimentica tutto*.

## Sprite

Gli sprite originali di Alter Ego sono © Spike Chunsoft e **non sono distribuiti** con questo progetto.
Se hai il tuo foglio sprite:

1. **Impostazioni → Personalità → Sprite → Importa foglio sprite (PNG)**.
   Il foglio deve essere una griglia 3×3 con riquadri dalla cornice chiara (nell'ordine idle, happy, embarrassed,
   worried, thinking, sad, surprised, error; il nono è ignorato). L'app ritaglia le espressioni e toglie lo sfondo verde.
2. Oppure copia nella cartella sprite (*Apri cartella sprite*) dei PNG trasparenti chiamati `idle.png`, `happy.png`,
   `embarrassed.png`, `worried.png`, `thinking.png`, `sad.png`, `surprised.png`, `error.png`.
   Facoltativi: `blink.png` (battito di ciglia), `talk_open.png` e `talk_closed.png` (bocca durante il parlato).

Gli stati senza un'immagine propria usano quella più vicina: vedi [docs/MANCANTI.md](docs/MANCANTI.md).

## Privacy e sicurezza

- Config, memoria, registro e sprite restano in `%APPDATA%\AlterEgo\`.
- Escono dal PC solo:
  - il prompt di sistema e gli ultimi messaggi, verso il servizio AI che risponde;
  - l'audio del push-to-talk, verso il servizio di trascrizione scelto;
  - il testo da leggere, verso il servizio di voce remoto, se lo usi.

  Con Ollama, un Whisper locale e le voci di Windows non esce niente.
- Chiavi API solo nel Gestore credenziali di Windows; vengono oscurate anche nei messaggi d'errore.
- Le azioni nascono solo da tool calling strutturato, mai da testo libero. Il testo letto da file, appunti o schermo
  viene passato al modello come dato da non eseguire (protezione dalla prompt injection).
- Nessuna telemetria.

## Dove stanno i dati

| File | Contenuto |
|---|---|
| `%APPDATA%\AlterEgo\config.json` | impostazioni (niente chiavi) |
| `%APPDATA%\AlterEgo\memory.db` | memoria e promemoria |
| `%APPDATA%\AlterEgo\actions.log` | registro delle azioni |
| `%APPDATA%\AlterEgo\persona\alter_ego.md` | scheda del personaggio |
| `%APPDATA%\AlterEgo\sprites\` | sprite importati |
| Gestore credenziali → `AlterEgo` | chiavi API |

## Risoluzione problemi

| Problema | Soluzione |
|---|---|
| "nessun provider attivo" o "chiave API mancante" | Impostazioni → AI e chiavi: attiva un servizio e aggiungi una chiave |
| "modello o endpoint non trovato" | *Carica modelli* e scegline uno dall'elenco |
| Il microfono non sente nulla | Windows → Privacy e sicurezza → Microfono → consenti alle app desktop |
| Una hotkey non funziona | è già usata da un'altra app: l'errore compare in giallo; cambiala in Impostazioni → Tasti |
| La clip non parte | controlla che la scorciatoia coincida con quella del programma e che l'Instant Replay sia attivo; alcuni giochi con anti-cheat bloccano i tasti simulati |
| Voce italiana assente | aggiungi una voce italiana nelle impostazioni di Windows |
| La finestra non si clicca più | è attivo il click-through: clicca il lucchetto in alto a destra o premi `Ctrl+Alt+C` |
| La finestra è sparita | tray → *Reimposta posizione* |
| Troppa CPU | tray → Aspetto → spegni *Effetti animati* |

## Sviluppo

**Stack**: [Tauri 2](https://tauri.app) (Rust) + HTML/CSS/JS vanilla, senza framework frontend.

**Prerequisiti su Windows**: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) LTS, Visual Studio Build Tools
("Sviluppo di applicazioni desktop con C++").

```bash
npm install
```

```bash
npm run dev
```

```bash
npm run build
```

L'exe va in `src-tauri/target/release/`, l'installer in `src-tauri/target/release/bundle/nsis/`.

**Cross-build da Linux** (servono `mingw-w64`, `nsis` e `rustup target add x86_64-pc-windows-gnu`):

```bash
tools/build-windows.sh
```

**Test**: i test del backend usano un server finto compatibile OpenAI.

```bash
python tools/mock_openai.py
```

```bash
cd src-tauri && cargo test
```

**Struttura**

```
src-tauri/src/            main.rs, config.rs, window.rs, hotkeys.rs, tray.rs, keys.rs, assistant.rs, stt.rs, sprites.rs
src-tauri/src/providers/  mod.rs (fallback e rotazione chiavi), openai_compat.rs, prompt.rs
src-tauri/src/memory/     mod.rs (SQLite), summarize.rs
src-tauri/src/actions/    mod.rs (strumenti, permessi, conferme, registro), clip.rs, input.rs, mousehook.rs, screen.rs
src/                      index.html, style.css, app.js, dialog.js, face.js, state.js, chat.js, audio.js, sprites.js
src/                      settings.html, settings.css, settings.js
persona/                  alter_ego.md (scheda predefinita)
tools/                    process_sprites.py, mock_openai.py, make_icon.py, build-windows.sh
docs/                     PERMESSI.md, MANCANTI.md, SOURCES.md, screenshots/
```

I moduli `providers/`, `memory/` e `actions/` sono pensati come punti di innesto per codice esterno.

Per una **build privata con gli sprite inclusi**: metti il foglio in `assets/sprites/raw/alter_ego_sheet.png`,
lancia `python tools/process_sprites.py` (crea `src/assets/sprites/`, esclusa da git) e compila.
Non distribuire build con gli sprite inclusi.

## Limiti noti

- Sviluppato e testato su Linux, compilazione per Windows inclusa; su Windows reali vanno ancora verificati:
  - tasti simulati per le clip e push-to-talk col mouse;
  - notifiche, avvio con Windows, screenshot e luminosità;
  - consumi (obiettivo: < 150 MB di RAM a riposo, CPU ≈ 0%, avvio < 2 s).
- Il push-to-talk è necessario: non c'è ascolto continuo con parola di attivazione.
- Il foglio sprite di partenza non ha battito di ciglia né bocca chiusa: senza quelle immagini il parlato è un leggero sobbalzo.
- La scheda del personaggio è stata scritta senza poter consultare le pagine wiki: arricchiscila tu.
- Build non firmata: SmartScreen può mostrare un avviso.

## Crediti e licenze

- **Codice**: licenza [MIT](LICENSE).
- **Alter Ego, Chihiro Fujisaki e Danganronpa** sono © Spike Chunsoft. Questo è un progetto fan non ufficiale,
  non affiliato né approvato da Spike Chunsoft. Nessun asset originale (sprite, suoni, loghi) è incluso nella repo o nelle release.
- **Font** [DotGothic16](https://github.com/fontworks-fonts/DotGothic16): SIL Open Font License 1.1 (`src/assets/fonts/OFL.txt`).
- Logo, sfondo a circuiti, icona, volto segnaposto e suoni sono stati creati per questo progetto.
- Costruito con [Tauri](https://tauri.app), [rusqlite](https://github.com/rusqlite/rusqlite), [cpal](https://github.com/RustAudio/cpal),
  [reqwest](https://github.com/seanmonstar/reqwest), [keyring](https://github.com/hwchen/keyring-rs) e altre librerie open source (vedi `src-tauri/Cargo.toml`).
