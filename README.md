# Alter Ego – Desktop Assistant

> ⚠ **Repo privata.** Da 1.0.1 il branch `main` contiene gli sprite originali di Alter Ego (© Spike Chunsoft),
> inclusi nella repo e nell'app. Non rendere pubblica la repo e non distribuire le build: sono solo per uso personale.

Assistente desktop per **Windows 10/11** ispirato ad *Alter Ego* di Danganronpa: una finestrella in stile laptop CRT,
sempre in primo piano, con cui parli a voce o per iscritto. Risponde con il proprio carattere, ricorda chi sei,
può fare clip dei tuoi giochi con una frase e, se glielo permetti, eseguire piccole azioni sul PC.

Progetto personale e non commerciale. Interfaccia e documentazione in italiano.

![Alter Ego sul desktop](docs/screenshots/main.png)

![Le 8 espressioni di Alter Ego](docs/screenshots/expressions.png)

## Indice

- [Funzionalità](#funzionalità)
- [Screenshot](#screenshot)
- [Installazione](#installazione)
- [Primo avvio](#primo-avvio)
- [Uso](#uso)
- [Intelligenza artificiale e chiavi API](#intelligenza-artificiale-e-chiavi-api)
- [Voce](#voce)
- [Mic-Link](#mic-link)
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
- **Animazione da vecchia TV** quando la finestra si apre (una linea luminosa che si allarga) e si chiude (lo schermo collassa in una linea e poi in un punto).
- **Dialogo automatico**: se non clicchi, il testo va avanti da solo dopo qualche secondo (e aspetta che la voce finisca).
- **5 servizi AI** (Gemini, OpenRouter, Groq, DeepSeek, Ollama locale) con **fallback automatico** e **più chiavi API per servizio** a rotazione.
  Prompt e cronologia sono costruiti per sfruttare la **cache di DeepSeek** (input in cache costa molto meno).
- **Push-to-talk** con trascrizione Whisper (Groq o server locale), su tastiera o **tasti laterali del mouse**.
- **Voce** di Alter Ego: **Kokoro-82M locale** (offline, installazione con un clic), voci di Windows o servizio compatibile OpenAI; **blip** del dialogo.
- **Dispositivi audio a scelta**: altoparlanti/cuffie per la voce, microfono per il push-to-talk.
- **Mic-Link**: microfono virtuale **AlterEgo Link Audio** con la tua voce + quella di Alter Ego, da usare in Discord, OBS, giochi.
- **Clip vocali**: "Alter Ego clippa" → salva la clip con AMD Adrenalin, NVIDIA App o Medal, **senza passare dall'AI**.
- **Azioni sul PC** tramite tool calling, tutte spente di default, con conferma per quelle sensibili (disattivabile) e registro consultabile.
- **Memoria locale** (SQLite): ricorda le conversazioni, fa riassunti, impara fatti su di te.
- **Personalità regolabile** (timida ↔ sicura, formale ↔ affettuosa), modalità imitazione, risposte in italiano, inglese o entrambe.
- Espressioni del volto pilotate dalle risposte (felice, sorpresa, imbarazzata, triste, preoccupata, pensierosa, errore).
- Leggera: Tauri 2 (Rust) + HTML/CSS/JS vanilla, installer di circa 6 MB.

## Screenshot

| | |
|---|---|
| ![Sorpresa](docs/screenshots/surprised.png) | ![Mentre pensa](docs/screenshots/thinking.png) |
| Espressione dalla risposta dell'AI (`[surprised]`) | Mentre pensa, solo scanline |
| ![Dialogo con box scuro e sfondo a circuiti](docs/screenshots/happy-dark.png) | ![Push-to-talk: in ascolto](docs/screenshots/listening.png) |
| Box scuro e sfondo a circuiti | Push-to-talk: in ascolto |
| ![Richiesta di permesso](docs/screenshots/confirm.png) | ![Input di testo](docs/screenshots/input.png) |
| Conferma prima di un'azione sul PC | Scrivere ad Alter Ego (`Ctrl+Alt+T`) |

**Spegnimento e accensione "vecchia TV"**

![Animazione: lo schermo collassa in una linea e un punto, poi si riaccende da una linea luminosa](docs/screenshots/tv-animation.png)

**Impostazioni**

| | |
|---|---|
| ![AI e chiavi](docs/screenshots/settings-ai.png) | ![Clip vocali](docs/screenshots/settings-clip.png) |
| Servizi AI, più chiavi, ordine di fallback | Clip vocali |
| ![Permessi](docs/screenshots/settings-perms.png) | ![Tasti](docs/screenshots/settings-hotkeys.png) |
| Permessi delle azioni | Tasti rapidi (anche mouse) |
| ![Voce e audio](docs/screenshots/settings-voice.png) | ![Generale](docs/screenshots/settings-general.png) |
| Dispositivi, Kokoro, Mic-Link | Aspetto, animazione TV, dialogo automatico |
| ![Personalità](docs/screenshots/settings-persona.png) | ![Registro](docs/screenshots/settings-log.png) |
| Personalità e lingua | Registro delle azioni |

## Installazione

1. Scarica dalla pagina [Releases](../../releases) l'ultima release (**`v1.0.4`**):
   l'**installer** `AlterEgo_1.0.4_setup.exe` oppure lo **zip portable** (estrai e avvia `alter-ego.exe`).
   Aggiornando da una versione precedente impostazioni, chiavi e memoria restano.
2. Windows SmartScreen può avvisare che l'app non è firmata: *Ulteriori informazioni → Esegui comunque*.
3. Serve **WebView2**, già presente in Windows 10/11 aggiornati.

L'app compare nella **system tray** (vicino all'orologio), non nella taskbar.

## Primo avvio

1. Clic destro sull'icona nella tray → **Impostazioni…**
2. **Generale**: scrivi il tuo nome.
3. **AI e chiavi**: attiva almeno un servizio e incolla una chiave API (ad esempio [Groq](https://console.groq.com/keys)
   o [Google AI Studio](https://aistudio.google.com/apikey)), poi *Prova connessione*. Con Groq funziona anche la voce.
   In alternativa usa [Ollama](https://ollama.com) in locale, senza chiavi.
4. Premi `Ctrl+Alt+T` e scrivi, oppure tieni premuto `Ctrl+Alt+Spazio` e parla.

## Uso

| Cosa | Come |
|---|---|
| Scrivere | `Ctrl+Alt+T` (o tray → *Scrivi ad Alter Ego…*), Invio invia, Esc chiude |
| Parlare | tieni premuto il push-to-talk (default `Ctrl+Alt+Spazio`), parla, rilascia |
| Andare avanti nel dialogo | clic sul box, Invio o Spazio (o aspetta: va avanti da solo) |
| Spostare / ridimensionare | trascina / angolo ↘ o `Ctrl` + rotella |
| Mostrare / nascondere | `Ctrl+Alt+A` o clic sinistro sull'icona della tray |
| Click-through | `Ctrl+Alt+C` o tray; per uscirne clicca il lucchetto in alto a destra |
| Uscire | tray → *Esci* (Alt+F4 nasconde soltanto) |

Avanzamento automatico e animazione TV si attivano/disattivano in **Impostazioni → Generale**: al tempo d'attesa scelto
si aggiunge il tempo di lettura della pagina.

Tutti i tasti si cambiano in **Impostazioni → Tasti**: clicca il campo e premi la combinazione (anche un tasto singolo come `F8`).
Per il push-to-talk vanno bene anche il **tasto centrale** e i **tasti laterali del mouse** (`Mouse4`, `Mouse5`).

Dalla tray cambi anche opacità, sfondo (scanline, circuiti, entrambi), intensità, stile del box (chiaro o scuro), effetti animati, suoni e voce.

## Intelligenza artificiale e chiavi API

| Servizio | Note |
|---|---|
| Google Gemini | endpoint compatibile OpenAI |
| OpenRouter | qualsiasi modello del catalogo |
| Groq | velocissimo; le sue chiavi servono anche per Whisper (voce) |
| DeepSeek | ottimizzato per la sua cache (vedi sotto) |
| Ollama | locale, nessuna chiave, nessun dato esce dal PC |

- **Più chiavi per servizio**: se una chiave risponde 401, 402, 403, 429 o "quota", si passa subito alla successiva.
  La volta dopo si riparte dall'ultima che ha funzionato.
- **Fallback**: l'ordine dei servizi si cambia con ▲▼. Se uno non risponde (rete, errore server, nessuna chiave valida) prova il successivo.
- Timeout e 2 tentativi con attesa crescente sugli errori temporanei. Se nessuno risponde, Alter Ego lo dice con l'espressione di errore.
- *Carica modelli* scarica l'elenco dei modelli dal servizio: i nomi predefiniti potrebbero non esistere più.
- Le chiavi stanno nel **Gestore credenziali di Windows** (voce `AlterEgo`), mai in file o log, e il frontend non le vede mai.

**Cache di DeepSeek** (e degli altri servizi con cache automatica del prefisso). DeepSeek fa pagare molto meno i token di input
che iniziano esattamente come una richiesta precedente. Per questo:

- il prompt di sistema mette prima tutto ciò che non cambia mai (personaggio, formato, regole) e in fondo le parti che cambiano
  (nome, fatti su di te, riassunto), così il blocco iniziale resta identico anche quando la memoria si aggiorna;
- la cronologia non "scorre" di un messaggio a ogni domanda: la finestra si sposta **a scatti** di metà della sua dimensione,
  quindi per diversi turni di fila tutta la conversazione precedente è già in cache (si mandano tra 1× e 1,5× i *Messaggi di contesto*);
- in *Impostazioni → AI e chiavi* vedi quanti token di input sono stati letti dalla cache da quando l'app è aperta.

## Voce

- **Riconoscimento (push-to-talk)**: Groq Whisper (`whisper-large-v3-turbo`) oppure un server Whisper locale compatibile OpenAI
  (es. faster-whisper-server/speaches, whisper.cpp server). *Prova microfono* registra 4 secondi e mostra cosa ha capito.
- **Voce di Alter Ego (TTS)**, in *Impostazioni → Voce e audio*:
  - **Kokoro-82M (locale)**: modello da 82M parametri ([Hugging Face](https://huggingface.co/hexgrad/Kokoro-82M),
    [libreria](https://github.com/hexgrad/kokoro), Apache-2.0), voci italiane *Sara* (`if_sara`) e *Nicola* (`im_nicola`) più
    quelle inglesi per le risposte in inglese. **Installa Kokoro** apre una finestra di PowerShell che crea un ambiente Python
    dedicato in `%APPDATA%\AlterEgo\kokoro` (installa Python 3.12 con winget se manca, PyTorch CPU o CUDA, la libreria `kokoro`)
    e scarica il modello. Poi Alter Ego avvia il server da solo (solo su `127.0.0.1`) e lo chiude quando esci.
    Hai già [Kokoro-FastAPI](https://github.com/remsky/Kokoro-FastAPI)? Metti il suo URL e spegni l'avvio automatico.
  - **Voci di Windows**, offline (per l'italiano: Impostazioni di Windows → Ora e lingua → Voce → Aggiungi voci, es. Elsa o Cosimo);
  - un **servizio compatibile OpenAI** `/audio/speech` (es. Piper locale o un servizio cloud).

  Le frasi vengono sintetizzate una alla volta: la prima si sente subito, le altre si preparano mentre parla. Il volto si anima finché la voce suona.
- **Dispositivi**: scegli l'uscita per voce e suoni e il microfono del push-to-talk (o lascia quelli predefiniti di Windows).
- **Blip del dialogo**: stile Danganronpa (ricreato con sintesi audio, nessun file originale), blip morbidi, o muto.

## Mic-Link

Mic-Link crea un microfono virtuale, **AlterEgo Link Audio**, che contiene **il tuo microfono + la voce di Alter Ego** (e, se vuoi, i blip).
Scegliendolo come microfono in Discord, OBS, Teams o in un gioco, gli altri sentono anche lei.

Windows non permette a un'app di creare da sola un dispositivo audio: serve un driver. Alter Ego usa il cavo virtuale gratuito
**[VB-CABLE](https://vb-audio.com/Cable/)** di VB-Audio (donationware, non incluso: si scarica dal loro sito).

1. *Impostazioni → Voce e audio → Mic-Link → Apri la pagina di VB-CABLE*: scarica il pacchetto, esegui `VBCABLE_Setup_x64.exe`
   come amministratore → *Install Driver*, poi riavvia il PC.
2. Premi **Rinomina**: con il permesso di amministratore il lato microfono del cavo ("CABLE Output") diventa
   **AlterEgo Link Audio** e il lato uscita ("CABLE Input") diventa "AlterEgo Link (uso interno)".
3. Attiva **Mic-Link** e scegli il tuo microfono vero. Regola i volumi di microfono e voce.
4. Nelle altre app scegli **AlterEgo Link Audio** come microfono.

Come funziona: Alter Ego legge il tuo microfono e scrive nel lato uscita del cavo il microfono mixato con la sua voce
(latenza massima del microfono ~150 ms). La voce si sente comunque anche dai tuoi altoparlanti.
Se dopo l'installazione di VB-CABLE non senti più niente, Windows l'ha messo come uscita predefinita: rimetti i tuoi altoparlanti
in *Impostazioni di Windows → Sistema → Audio* (le impostazioni di Alter Ego te lo segnalano).

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

**Non chiedere conferma** (in cima alla scheda, spento di default) esegue subito anche le azioni ⚠. Restano valide le
autorizzazioni per tipo di azione, le cartelle consentite e la lista dei comandi, e tutto finisce nel Registro
(con "approvata in automatico"). Rischio: un testo letto da file, schermo o web potrebbe spingere il modello a usare
un'azione senza che tu la veda prima, quindi attivalo solo con azioni e cartelle che ti stanno bene.

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

Gli sprite sono **inclusi** nell'app (`src/assets/sprites/`) e il foglio originale è in `assets/sprites/raw/alter_ego_sheet.png`.
Le espressioni seguono le emozioni delle risposte:

| Stato | Sprite |
|---|---|
| `idle`, `listening`, `talk_open` | idle |
| `happy`, `embarrassed`, `worried`, `thinking`, `sad`, `surprised`, `error` | sprite omonimo |
| `sleep` (dopo 10 minuti di inattività) | sad, attenuato e con un respiro lento |

Mancano `blink` (battito di ciglia) e `talk_closed` (bocca chiusa): senza quelle immagini il parlato è un leggero sobbalzo.
Dettagli in [docs/MANCANTI.md](docs/MANCANTI.md).

**Sprite personalizzati**: in *Impostazioni → Personalità → Sprite* puoi importare un altro foglio sprite (PNG): l'app
riconosce i riquadri, toglie lo sfondo verde e salva le immagini in `%APPDATA%\AlterEgo\sprites`, che hanno la precedenza
su quelli inclusi. *Rimuovi* torna agli sprite inclusi. Puoi anche mettere a mano in quella cartella `blink.png`,
`talk_open.png` o `talk_closed.png` (PNG trasparenti, stesso ancoraggio).

Per rigenerare gli sprite inclusi dal foglio (es. dopo averlo modificato), con `pip install pillow numpy`:

```bash
python tools/process_sprites.py
```

## Privacy e sicurezza

- Config, memoria e registro restano in `%APPDATA%\AlterEgo\`.
- Escono dal PC solo:
  - il prompt di sistema e gli ultimi messaggi, verso il servizio AI che risponde;
  - l'audio del push-to-talk, verso il servizio di trascrizione scelto;
  - il testo da leggere, verso il servizio di voce remoto, se lo usi;
  - solo durante l'installazione di Kokoro: i download di Python, PyTorch, `kokoro` e del modello da Hugging Face.

  Con Ollama, un Whisper locale e Kokoro o le voci di Windows non esce niente.
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
| `%APPDATA%\AlterEgo\sprites\` | sprite importati (facoltativi) |
| `%APPDATA%\AlterEgo\kokoro\` | Kokoro: ambiente Python, server, `server.log` (il modello è nella cache di Hugging Face, `%USERPROFILE%\.cache\huggingface`) |
| Gestore credenziali → `AlterEgo` | chiavi API |

## Risoluzione problemi

| Problema | Soluzione |
|---|---|
| "nessun provider attivo" o "chiave API mancante" | Impostazioni → AI e chiavi: attiva un servizio e aggiungi una chiave |
| "modello o endpoint non trovato" | *Carica modelli* e scegline uno dall'elenco |
| Il microfono non sente nulla | Windows → Privacy e sicurezza → Microfono → consenti alle app desktop |
| Una hotkey non funziona | è già usata da un'altra app: l'errore compare in giallo; cambiala in Impostazioni → Tasti |
| La clip non parte | controlla che la scorciatoia coincida con quella del programma e che l'Instant Replay sia attivo; alcuni giochi con anti-cheat bloccano i tasti simulati |
| Voce italiana assente | aggiungi una voce italiana nelle impostazioni di Windows, o usa Kokoro |
| Kokoro non si installa | leggi l'errore nella finestra di PowerShell; serve internet e ~3 GB liberi. *Reinstalla / aggiorna* riprende da dove si era fermato |
| "Kokoro non pronto" | il primo avvio carica il modello (fino a ~30 s); i dettagli sono in `%APPDATA%\AlterEgo\kokoro\server.log`. La porta 8880 potrebbe essere occupata: cambia l'URL |
| Non sento la voce | controlla *Uscita* in Voce e audio; se Windows ha messo VB-CABLE come uscita predefinita, rimetti i tuoi altoparlanti |
| Gli altri non mi sentono con Mic-Link | nell'app scegli **AlterEgo Link Audio** come microfono; in Alter Ego controlla che lo stato di Mic-Link sia "Attivo" e che il microfono da mixare sia quello vero |
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
src-tauri/src/            main.rs, config.rs, window.rs, hotkeys.rs, tray.rs, keys.rs, assistant.rs, stt.rs
src-tauri/src/providers/  mod.rs (fallback e rotazione chiavi), openai_compat.rs, prompt.rs
src-tauri/src/memory/     mod.rs (SQLite), summarize.rs
src-tauri/src/actions/    mod.rs (strumenti, permessi, conferme, registro), clip.rs, input.rs, mousehook.rs, screen.rs
src-tauri/src/audio/      mod.rs (uscite, Mic-Link), synth.rs (blip), tts.rs (Windows/Kokoro/OpenAI), kokoro.rs, rename.rs, commands.rs
src/                      index.html, style.css, app.js, dialog.js, face.js, state.js, chat.js, audio.js, sprites.js
src/                      settings.html, settings.css, settings.js
persona/                  alter_ego.md (scheda predefinita)
tools/                    process_sprites.py, mock_openai.py, make_icon.py, build-windows.sh
tools/kokoro/             server.py (server Kokoro compatibile OpenAI), setup.ps1 (installazione)
src/assets/sprites/       gli 8 sprite elaborati
assets/sprites/raw/       foglio sprite originale
docs/                     PERMESSI.md, MANCANTI.md, SOURCES.md, screenshots/
```

I moduli `providers/`, `memory/` e `actions/` sono pensati come punti di innesto per codice esterno.

Gli sprite sono inclusi: `npm run build` produce direttamente la versione con il volto di Alter Ego. Non distribuire le build.

La release si crea da GitHub: Actions → *Release* → *Run workflow* (branch `main`), oppure con un push sul branch `release`.
La versione è quella di `src-tauri/tauri.conf.json`. Il workflow si rifiuta di pubblicare se la repo è pubblica.

## Limiti noti

- Sviluppato e testato su Linux, compilazione per Windows inclusa; su Windows reali vanno ancora verificati:
  - voci di Windows (WinRT), Mic-Link con VB-CABLE e la rinomina del dispositivo, installazione di Kokoro;
  - tasti simulati per le clip e push-to-talk col mouse;
  - notifiche, avvio con Windows, screenshot e luminosità;
  - consumi (obiettivo: < 150 MB di RAM a riposo, CPU ≈ 0%, avvio < 2 s).
- Il push-to-talk è necessario: non c'è ascolto continuo con parola di attivazione.
- Il foglio sprite di partenza non ha battito di ciglia né bocca chiusa: senza quelle immagini il parlato è un leggero sobbalzo.
- Mic-Link richiede VB-CABLE: Windows non permette di creare un microfono virtuale senza un driver.
- Kokoro su CPU impiega circa mezzo secondo per frase su un PC recente; la prima frase dopo l'avvio aspetta il caricamento del modello.
- La scheda del personaggio è stata scritta senza poter consultare le pagine wiki: arricchiscila tu.
- Build non firmata: SmartScreen può mostrare un avviso.

## Crediti e licenze

- **Codice**: licenza [MIT](LICENSE).
- **Alter Ego, Chihiro Fujisaki e Danganronpa** sono © Spike Chunsoft. Questo è un progetto fan non ufficiale,
  non affiliato né approvato da Spike Chunsoft. **Gli sprite originali sono inclusi solo per uso personale** (repo privata):
  sono esclusi dalla licenza MIT e non vanno ridistribuiti.
- **Kokoro-82M** e la libreria `kokoro` di hexgrad: Apache-2.0. Non sono inclusi nell'app: li scarica l'installazione di Kokoro.
- **VB-CABLE** di VB-Audio Software: donationware, non incluso; si scarica dal sito ufficiale.
- **Font** [DotGothic16](https://github.com/fontworks-fonts/DotGothic16): SIL Open Font License 1.1 (`src/assets/fonts/OFL.txt`).
- Logo, sfondo a circuiti, icona e suoni sono stati creati per questo progetto.
- Costruito con [Tauri](https://tauri.app), [rusqlite](https://github.com/rusqlite/rusqlite), [cpal](https://github.com/RustAudio/cpal),
  [reqwest](https://github.com/seanmonstar/reqwest), [keyring](https://github.com/hwchen/keyring-rs) e altre librerie open source (vedi `src-tauri/Cargo.toml`).
