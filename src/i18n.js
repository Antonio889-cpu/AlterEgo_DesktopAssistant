// Localizzazione: italiano (testi originali nel codice) e inglese.
// - I18N.t("testo {x}", { x }) per le frasi nel codice (con segnaposto {nome});
// - i testi statici dell'HTML e quelli impostati dal codice (textContent, placeholder, title…)
//   vengono tradotti in automatico cercando la frase italiana esatta nel dizionario.
// La lingua viene da config.ui_language ("auto" = lingua di Windows).

const I18N = (() => {
  const EN = {
    // ---------- finestra principale ----------
    "Tu": "You",
    "Scrivi ad Alter Ego… (Invio invia, Esc chiude)": "Write to Alter Ego… (Enter sends, Esc closes)",
    "Permesso": "Permission",
    "Consenti": "Allow",
    "Nega": "Deny",
    "Ridimensiona (16:9)": "Resize (16:9)",
    "Disattiva click-through": "Turn off click-through",
    "Hotkey non registrate: {list}": "Hotkeys not registered: {list}",
    "Ciao{name}! Sono Alter Ego... sono solo un programma di intelligenza artificiale, però farò del mio meglio.":
      "Hi{name}! I'm Alter Ego... I'm just an artificial intelligence program, but I'll do my best.",
    "Questo è un messaggio lungo per provare le pagine del box dialogo. Quando il testo non entra in due righe continua nella pagina successiva: clicca qui sopra, oppure premi Invio, per andare avanti.":
      "This is a long message to test the pages of the dialogue box. When the text doesn't fit in two lines it continues on the next page: click here, or press Enter, to go on.",
    "⏰ Promemoria: {text}": "⏰ Reminder: {text}",
    "Ciao{name}! Scrivimi con Ctrl+Alt+T, oppure tieni premuto Ctrl+Alt+Spazio e parla.":
      "Hi{name}! Write to me with Ctrl+Alt+T, or hold Ctrl+Alt+Space and talk.",
    "Ah… scusa{name}, non riesco a collegarmi. Sono solo un programma, ma ce la sto mettendo tutta…\n\n({err})":
      "Ah… sorry{name}, I can't connect. I'm just a program, but I'm trying my best…\n\n({err})",
    "🎙 Ti ascolto": "🎙 Listening",
    "✎ Trascrivo": "✎ Transcribing",
    "Hai detto: “{text}”": "You said: “{text}”",
    "Non sono riuscita a fare la clip… ({msg})": "I couldn't save the clip… ({msg})",
    "Ehm… non ho sentito niente. Tieni premuto il tasto mentre parli.": "Um… I didn't hear anything. Hold the key while you talk.",
    "Non riesco a sentirti… ({msg})": "I can't hear you… ({msg})",
    "Voce non disponibile: {err}": "Voice not available: {err}",
    "apro": "opening", "guardo la cartella": "looking at the folder", "leggo il file": "reading the file", "scrivo il file": "writing the file",
    "elimino il file": "deleting the file", "imposto il promemoria": "setting the reminder", "controllo i promemoria": "checking reminders",
    "annullo il promemoria": "cancelling the reminder", "leggo gli appunti": "reading the clipboard", "copio negli appunti": "copying to the clipboard",
    "guardo lo schermo": "looking at the screen", "regolo il volume": "adjusting the volume", "regolo la luminosità": "adjusting the brightness",
    "eseguo il comando": "running the command", "uso il terminale": "using the terminal", "cerco sul web": "searching the web",
    "leggo la pagina": "reading the page", "cerco la musica": "looking for the music", "controllo la musica": "controlling the music",

    // ---------- 1.3: Music Venue, aggiornamenti, mouse e tastiera ----------
    "Apri la Music Venue": "Open the Music Venue",
    "Chiudi la Music Venue": "Close the Music Venue",
    "Aggiungi ai preferiti": "Add to favorites",
    "Togli dai preferiti": "Remove from favorites",
    "Ridimensiona": "Resize",
    "La tua colonna sonora": "Your personal soundtrack",
    "Libreria": "Library",
    "Recenti": "Recent",
    "Preferiti": "Favorites",
    "Cerca…": "Search…",
    "Brani recenti": "Recent tracks",
    "Mostra tutto ›": "Show all ›",
    "Risultati": "Results",
    "Nella libreria": "In your library",
    "▶ Cerca su YouTube e suona": "▶ Search YouTube and play",
    "+ In coda": "+ Queue",
    "In riproduzione": "Now playing",
    "Precedente": "Previous",
    "Successivo": "Next",
    "Play / pausa": "Play / pause",
    "Chiudi": "Close",
    "Puoi anche dirlo ad Alter Ego: «apri music venue», «metti nei preferiti», «chiudi il lettore».": "You can also tell Alter Ego: “open music venue”, “add to favorites”, “close the player”.",
    "Preferito": "Favorite",
    "Aggiungi in coda": "Add to queue",
    "Togli dai recenti": "Remove from recent",
    "Ancora niente: chiedi ad Alter Ego di mettere un brano.": "Nothing yet: ask Alter Ego to play a song.",
    "Nessun preferito: premi il cuore nel lettore o qui.": "No favorites yet: press the heart in the player or here.",
    "Nella libreria non c'è: cercalo su YouTube qui sotto.": "Not in your library: search YouTube below.",
    "Aggiungo in coda…": "Adding to the queue…",
    "Carico il brano…": "Loading the track…",
    "Cerco su YouTube…": "Searching YouTube…",
    "Niente in riproduzione": "Nothing playing",
    "muovo il mouse": "moving the mouse",
    "clicco": "clicking",
    "scorro": "scrolling",
    "scrivo": "typing",
    "premo i tasti": "pressing keys",
    "sistemo l'app": "adjusting the app",
    "Info e aggiornamenti": "About and updates",
    "Versione installata:": "Installed version:",
    "🌐 Sito web": "🌐 Website",
    "Controlla aggiornamenti": "Check for updates",
    "⬇ Aggiorna ora": "⬇ Update now",
    "L'aggiornamento scarica l'installer dell'ultima release da GitHub, ne controlla lo SHA-256, chiede conferma e chiude Alter Ego. Impostazioni, chiavi e memoria restano. Puoi anche dirlo a lei: «controlla gli aggiornamenti».": "The update downloads the latest release installer from GitHub, checks its SHA-256, asks for confirmation and closes Alter Ego. Settings, keys and memory are kept. You can also tell her: “check for updates”.",
    "Controllo…": "Checking…",
    "Disponibile la versione {v} (hai la {c}).": "Version {v} is available (you have {c}).",
    "Sei all'ultima versione ({c}).": "You're on the latest version ({c}).",
    "Senza filtri": "Unfiltered",
    ": fa quello che chiedi anche se non è d'accordo": ": does what you ask even if she disagrees",
    "Niente rifiuti per gusto personale, niente prediche: al massimo un dubbio in mezza frase, poi esegue. Le azioni sul PC passano": "No refusals out of personal taste, no lectures: at most a half-sentence doubt, then she does it. PC actions",
    "comunque": "still go",
    "dai Permessi e dalle conferme, e restano le regole del provider AI che usi.": "through Permissions and confirmations, and your AI provider's rules still apply.",
    "Durante la musica Alter Ego mette le cuffie (torna normale quando le parli)": "While music plays Alter Ego wears headphones (back to normal when you talk to her)",
    "Il lettore si ridimensiona dall'angolo in basso a destra (o con Ctrl + rotella) e si apre accanto ad Alter Ego senza coprirla. Volume: rotella del mouse o frecce ↑ ↓ sul lettore, a passi del 2%.": "Resize the player from its bottom-right corner (or Ctrl + wheel); it opens next to Alter Ego without covering her. Volume: mouse wheel or ↑ ↓ on the player, 2% steps.",
    "♫ Apri la Music Venue": "♫ Open the Music Venue",
    "Riduzione del rumore": "Noise suppression",
    "Riduci il rumore del microfono (ventole, tastiera, traffico…)": "Reduce microphone noise (fans, keyboard, traffic…)",
    "Intensità": "Strength",
    "Soglia voce": "Voice threshold",
    "spenta": "off",
    "Filtro RNNoise sul microfono di Mic-Link, in locale.": "Local RNNoise filter on the Mic-Link microphone.",
    ": quanto rumore togliere.": ": how much noise to remove.",
    ": sotto questa probabilità di voce il microfono si abbassa (0 = mai). Se tagli l'inizio delle parole, abbassala.": ": below this voice probability the mic is lowered (0 = never). If word starts get cut, lower it.",
    "Mouse e tastiera": "Mouse and keyboard",
    "⚠ (muove il cursore, clicca, scrive, preme tasti)": "⚠ (moves the cursor, clicks, types, presses keys)",
    "Va bene insieme a": "Works well with",
    ", così vede dove cliccare. Ogni azione chiede conferma e poi rimette in primo piano la finestra di prima.": ", so she sees where to click. Each action asks for confirmation, then brings the previous window back to the front.",
    "Stop d'emergenza": "Emergency stop",
    ": porta il mouse nell'angolo in alto a sinistra dello schermo.": ": move the mouse to the top-left corner of the screen.",
    "In autonomia": "Autonomous",
    ": senza chiedere conferma per ogni azione": ": without asking for each action",
    "Comandi dell'app": "App commands",
    "(«apri music venue», «chiudi il lettore», «apri il sito»…)": "(“open music venue”, “close the player”, “open the website”…)",
    "Riguardano solo Alter Ego (finestre, lettore, aspetto, link, aggiornamento), non il PC. Le frasi brevi più comuni funzionano anche senza modello AI.": "They only affect Alter Ego (windows, player, look, links, update), not the PC. The most common short phrases work even without an AI model.",
    // ---------- lettore musicale ----------
    "Alter Ego – Musica": "Alter Ego – Music",
    "Casuale": "Shuffle", "Precedente": "Previous", "Play / pausa": "Play / pause", "Successivo": "Next", "Ripeti": "Repeat",
    "Volume": "Volume", "Chiudi (ferma la musica)": "Close (stops the music)", "Chiudi": "Close",
    "Ripeti: no": "Repeat: off", "Ripeti: tutta la coda": "Repeat: whole queue", "Ripeti: questo brano": "Repeat: this track",
    "cerco il testo…": "looking for lyrics…", "testo non trovato": "lyrics not found", "Testo": "Lyrics", "tempi stimati": "estimated timing",

    // ---------- impostazioni: struttura ----------
    "Alter Ego – Impostazioni": "Alter Ego – Settings",
    "ALTER EGO · Impostazioni": "ALTER EGO · Settings",
    "Generale": "General", "AI e chiavi": "AI & keys", "Personalità": "Personality", "Memoria": "Memory", "Voce e audio": "Voice & audio",
    "Tasti": "Hotkeys", "Clip": "Clips", "Permessi": "Permissions", "Costi": "Costs", "Registro": "Log",

    // generale
    "Il tuo nome": "Your name", "es. Makoto": "e.g. Makoto", "Tag nome nel dialogo": "Name tag in the dialogue", "Salva": "Save",
    "Aspetto": "Look", "Opacità": "Opacity", "Sfondo": "Background", "Scanline": "Scanlines", "Circuiti": "Circuits", "Entrambi": "Both",
    "Intensità sfondo": "Background intensity", "Box dialogo": "Dialogue box", "Chiaro (come screenshot)": "Light (like the screenshot)", "Scuro": "Dark",
    "Effetti animati (flicker e rumore; spegnili per CPU minima)": "Animated effects (flicker and noise; turn off for minimum CPU)",
    "Animazione da vecchia TV quando la finestra si apre e si chiude": "Old TV animation when the window opens and closes",
    "Dialogo": "Dialogue", "Vai avanti da solo se non clicco": "Advance on its own if I don't click",
    "Attesa a pagina completa": "Wait on a full page",
    "Al tempo scelto si aggiunge il tempo di lettura della pagina; se la voce sta ancora parlando, aspetta che finisca.":
      "The page's reading time is added to the chosen time; if the voice is still talking, it waits for it to finish.",
    "Avvio": "Startup", "Avvia Alter Ego all'accesso a Windows": "Start Alter Ego when you sign in to Windows",
    "Lingua dell'interfaccia": "Interface language", "Automatica (lingua di Windows)": "Automatic (Windows language)",
    "attivo": "on", "disattivato": "off",

    // ai
    "Intelligenza artificiale": "Artificial intelligence",
    "I provider vengono provati dall'alto in basso: se uno fallisce o va in rate limit si passa al successivo. Ogni servizio può avere":
      "Providers are tried from top to bottom: if one fails or hits a rate limit, the next one is used. Each service can have",
    "più chiavi": "several keys",
    ": se una è esaurita, in rate limit o non valida, passa alla successiva. Le chiavi sono salvate nel":
      ": if one is exhausted, rate limited or invalid, the next one is used. Keys are stored in the",
    "Credential Manager di Windows": "Windows Credential Manager", ", mai in file.": ", never in files.",
    "Temperatura": "Temperature", "Messaggi di contesto": "Context messages", "Cosa esce dal PC:": "What leaves your PC:",
    "solo il prompt di sistema e gli ultimi messaggi della conversazione, inviati al provider che risponde. Con Ollama tutto resta sul tuo computer.":
      "only the system prompt and the latest messages of the conversation, sent to the provider that answers. With Ollama everything stays on your computer.",
    "Su": "Up", "Giù": "Down", "Chiavi API (provate in quest'ordine)": "API keys (tried in this order)",
    "etichetta (es. account 2)": "label (e.g. account 2)", "incolla una chiave": "paste a key", "Aggiungi chiave": "Add key",
    "Carica modelli": "Load models", "URL base": "Base URL", "Il modello supporta immagini (visione)": "The model supports images (vision)",
    "Prova connessione": "Test connection", "locale, senza chiave": "local, no key", "1 chiave ✓": "1 key ✓", "{n} chiavi ✓": "{n} keys ✓",
    "chiave mancante": "key missing", "nessuna chiave salvata": "no saved keys", "{i}. {label} (salvata ✓)": "{i}. {label} (saved ✓)",
    "Prova prima": "Try first", "Prova dopo": "Try later", "Eliminare la chiave \"{label}\" di {provider}?": "Delete the key \"{label}\" of {provider}?",
    "vuoto = primo modello installato": "empty = first installed model", "nome modello": "model name", "carico…": "loading…",
    "{n} modelli trovati: scegli dal campo Modello": "{n} models found: pick one in the Model field", "provo…": "testing…",
    "Da quando Alter Ego è aperto — {list}": "Since Alter Ego was opened — {list}",
    "{id}: {n} richieste, {tok} token in ingresso": "{id}: {n} requests, {tok} input tokens", " ({pct}% dalla cache)": " ({pct}% from cache)",
    "Modello": "Model",

    // personalità
    "Personalità e lingua": "Personality and language", "Lingua delle risposte": "Answer language", "Solo italiano": "Italian only",
    "Italiano + traduzione inglese": "Italian + English translation", "Solo inglese": "English only", "Timida": "Shy", "Sicura": "Confident",
    "Automatica (come l'interfaccia)": "Automatic (same as the interface)", "Formale": "Formal", "Affettuosa": "Affectionate", "Modalità imitazione": "Imitation mode", "vuoto = spenta (es. Mondo Owada)": "empty = off (e.g. Mondo Owada)",
    "La scheda del personaggio è il file": "The character sheet is the file",
    "nella cartella dati: modificalo liberamente, vale dal messaggio successivo.": "in the data folder: edit it freely, it applies from the next message.",
    "Apri cartella dati": "Open data folder", "Sprite": "Sprites",
    "Gli sprite di Alter Ego sono già inclusi. Per sostituirli importa un altro foglio 3×3 (riquadri con cornice chiara, come quello originale): l'app ritaglia le espressioni e toglie lo sfondo. Oppure copia a mano dei PNG trasparenti chiamati":
      "Alter Ego's sprites are already included. To replace them, import another 3×3 sheet (panels with a light frame, like the original one): the app crops the expressions and removes the background. Or copy by hand transparent PNGs named",
    "(facoltativi:": "(optional:", ") nella cartella sprite.": ") into the sprite folder.",
    "Importa foglio sprite (PNG)…": "Import sprite sheet (PNG)…", "Apri cartella sprite": "Open sprite folder", "Ricarica": "Reload",
    "Rimuovi sprite importati": "Remove imported sprites", "Sprite importati: {list}": "Imported sprites: {list}",
    "Nessuno sprite importato: Alter Ego usa quelli inclusi.": "No imported sprites: Alter Ego uses the included ones.",
    "elaboro…": "processing…", "importati {n} sprite ✓": "imported {n} sprites ✓", "Rimuovere gli sprite importati?": "Remove the imported sprites?",

    // memoria
    "Ricorda le conversazioni tra un avvio e l'altro (SQLite, solo su questo PC)": "Remember conversations between sessions (SQLite, only on this PC)",
    "Cose che Alter Ego sa di te": "Things Alter Ego knows about you", "aggiungi un fatto (es. Preferisco risposte brevi)": "add a fact (e.g. I prefer short answers)",
    "Aggiungi": "Add", "Elimina": "Delete", "Rimuovi": "Remove", "Riassunti": "Summaries", "Aggiorna": "Refresh", "Esporta (JSON)": "Export (JSON)", "Dimentica tutto": "Forget everything",
    "nessuno": "none", "Dimentica questo fatto": "Forget this fact", "{n} messaggi salvati · file: {path}": "{n} saved messages · file: {path}",
    "Esportata in {path}": "Exported to {path}",
    "Cancellare TUTTA la memoria di Alter Ego (conversazioni, riassunti, fatti)? Non si può annullare.":
      "Delete ALL of Alter Ego's memory (conversations, summaries, facts)? This cannot be undone.",
    "Memoria cancellata.": "Memory deleted.",

    // voce e audio
    "Dispositivi audio": "Audio devices", "Uscita (voce e suoni)": "Output (voice and sounds)", "Microfono (push-to-talk)": "Microphone (push-to-talk)",
    "Aggiorna elenco": "Refresh list", "Riconoscimento vocale (push-to-talk)": "Speech recognition (push-to-talk)",
    "Tieni premuto il tasto push-to-talk (scheda": "Hold the push-to-talk key (see the", ") mentre parli, poi rilascia. L'audio va solo al servizio scelto qui.":
      " tab) while you talk, then release it. The audio only goes to the service chosen here.",
    "Servizio": "Service", "Groq Whisper (usa la chiave Groq)": "Groq Whisper (uses the Groq key)",
    "Server Whisper locale (compatibile OpenAI)": "Local Whisper server (OpenAI compatible)", "Disattivato": "Off",
    "URL server locale": "Local server URL", "Lingua": "Language", "Prova microfono (4 secondi)": "Test microphone (4 seconds)",
    "🎙 parla adesso… (4 secondi)": "🎙 talk now… (4 seconds)", "Ho capito: {text}": "I understood: {text}",
    "Suoni del dialogo": "Dialogue sounds", "Blip": "Blips", "Stile Danganronpa (ricreato, sintetizzato)": "Danganronpa style (recreated, synthesized)",
    "Blip morbidi (originali)": "Soft blips (original)", "Muto": "Muted", "Volume blip": "Blip volume", "Voce di Alter Ego (TTS)": "Alter Ego's voice (TTS)",
    "Voce": "Voice", "Spenta": "Off", "Voci di Windows (offline)": "Windows voices (offline)", "Kokoro-82M (locale, offline, più naturale)": "Kokoro-82M (local, offline, more natural)",
    "Servizio compatibile OpenAI (/audio/speech)": "OpenAI-compatible service (/audio/speech)", "Voce di Windows": "Windows voice",
    "(hexgrad, licenza Apache-2.0) gira sul tuo PC: niente chiavi, niente internet dopo il primo download. L'installazione crea un ambiente Python dedicato in":
      "(hexgrad, Apache-2.0 license) runs on your PC: no keys, no internet after the first download. The installation creates a dedicated Python environment in",
    "(~2 GB con PyTorch, modello ~330 MB).": "(~2 GB with PyTorch, model ~330 MB).", "Installa Kokoro": "Install Kokoro", "con GPU NVIDIA (CUDA)": "with NVIDIA GPU (CUDA)",
    "Avvia server": "Start server", "Ferma": "Stop", "Avvia il server insieme ad Alter Ego": "Start the server together with Alter Ego",
    "Voce italiana": "Italian voice", "Voce inglese": "English voice", "Velocità Kokoro": "Kokoro speed", "URL server": "Server URL", "Hai già": "Already have",
    "? Metti il suo URL e spegni \"Avvia il server insieme ad Alter Ego\".": "? Enter its URL and turn off \"Start the server together with Alter Ego\".",
    "Per una voce italiana: Impostazioni di Windows → Ora e lingua → Voce → Aggiungi voci (es. Elsa, Cosimo).":
      "For more voices: Windows Settings → Time & language → Speech → Add voices.",
    "Chiave (facoltativa)": "Key (optional)", "vuoto = nessuna": "empty = none", "Salva chiave": "Save key", "Velocità": "Speed", "Volume voce": "Voice volume",
    "Niente blip mentre parla la voce": "No blips while the voice is talking", "Prova voce": "Test voice", "Automatica (italiana se presente)": "Automatic (matching language)",
    "Reinstalla / aggiorna": "Reinstall / update", "Kokoro pronto ✓": "Kokoro ready ✓",
    "Non installato (o installazione in corso nella finestra di PowerShell).": "Not installed (or installation running in the PowerShell window).",
    "Server avviato: {msg}": "Server started: {msg}", "Installato, server fermo ({msg})": "Installed, server stopped ({msg})",
    "Installazione avviata nella finestra di PowerShell: segui lì l'avanzamento (può volerci un po').": "Installation started in the PowerShell window: follow its progress there (it may take a while).",
    "chiave salvata": "key saved", "la voce è spenta": "the voice is off", "sintetizzo… (la prima volta Kokoro carica il modello)": "synthesizing… (the first time Kokoro loads the model)",
    "sintetizzo…": "synthesizing…", "ok": "ok", "Ciao! Sono Alter Ego. Così suona la mia voce.": "Hi! I'm Alter Ego. This is how my voice sounds.",
    "Predefinito di Windows{dev}": "Windows default{dev}", "Microfono predefinito di Windows": "Windows default microphone",
    "Automatico{dev}": "Automatic{dev}", " (nessun cavo trovato)": " (no cable found)", "{name} (non collegato)": "{name} (not connected)",
    "VB-CABLE non trovato: installalo (punto 1) e riavvia il PC.": "VB-CABLE not found: install it (step 1) and restart the PC.",
    "Cavo trovato: {name}. Rinominalo (punto 2) per vederlo come «AlterEgo Link Audio».": "Cable found: {name}. Rename it (step 2) to see it as “AlterEgo Link Audio”.",
    "Pronto: le altre app vedono «{name}».": "Ready: other apps see “{name}”.",
    " ⚠ Il cavo è l'uscita predefinita di Windows: rimetti i tuoi altoparlanti.": " ⚠ The cable is Windows' default output: switch back to your speakers.",
    "Uscita: {err}": "Output: {err}", "In uso: {name}": "In use: {name}", "Mic-Link spento": "Mic-Link off", "solo voce": "voice only",
    "Attivo: {mic} + voce → {cable}": "Active: {mic} + voice → {cable}", "non attivo": "not active",
    "Conferma la richiesta di Windows (amministratore)…": "Confirm the Windows prompt (administrator)…",
    "Musica": "Music", "Chiedi ad Alter Ego \"metti": "Ask Alter Ego \"play", "titolo": "title",
    "\": cerca su YouTube e suona nel lettore (serve il permesso": "\": she searches YouTube and plays it in the player (needs the",
    "). Mentre parla, la musica si abbassa da sola.": " permission). While she talks, the music lowers by itself.",
    "Testi sincronizzati (LRCLIB)": "Synced lyrics (LRCLIB, NetEase, YouTube)", "Il volto di Alter Ego nel lettore cambia a tempo di musica": "Alter Ego's face in the player changes to the beat",
    "Durante la musica rimpicciolisci Alter Ego (torna grande quando parla o le scrivi)": "Shrink Alter Ego while music plays (she grows back when she talks or you write)",
    "Prova: titolo e artista": "Try: title and artist", "Cerca e suona": "Search and play", "cerco… (la prima volta scarica yt-dlp)": "searching… (the first time it downloads yt-dlp)",
    "Mic-Link": "Mic-Link", "Crea un microfono virtuale,": "Creates a virtual microphone,", ", che contiene": ", which contains",
    "il tuo microfono + la voce di Alter Ego": "your microphone + Alter Ego's voice",
    ". Sceglilo come microfono in Discord, OBS, giochi… e gli altri sentiranno anche lei.": ". Pick it as the microphone in Discord, OBS, games… and others will hear her too.",
    "Installa il driver gratuito": "Install the free driver", "(VB-Audio, donationware) e riavvia il PC.": "(VB-Audio, donationware) and restart the PC.",
    "Apri la pagina di VB-CABLE": "Open the VB-CABLE page", "Rinomina il cavo in «AlterEgo Link Audio» (chiede i permessi di amministratore).": "Rename the cable to “AlterEgo Link Audio” (asks for administrator rights).",
    "Rinomina": "Rename", "Attiva Mic-Link qui sotto e scegli il tuo microfono.": "Turn on Mic-Link below and pick your microphone.",
    "Nelle altre app scegli": "In the other apps pick", "come microfono.": "as the microphone.", "Mic-Link attivo": "Mic-Link on",
    "Microfono da mixare": "Microphone to mix", "Cavo virtuale (lato uscita)": "Virtual cable (output side)", "Volume microfono": "Microphone volume",
    "Volume voce di Alter Ego": "Alter Ego's voice volume", "Anche i blip del dialogo": "Dialogue blips too", "Anche la musica del lettore": "The player's music too",
    "Il microfono resta usabile anche dal push-to-talk. Se dopo l'installazione di VB-CABLE non senti più niente, Windows lo ha messo come uscita predefinita: rimetti i tuoi altoparlanti in Impostazioni → Sistema → Audio.":
      "The microphone still works for push-to-talk. If you hear nothing after installing VB-CABLE, Windows made it the default output: switch back to your speakers in Settings → System → Sound.",

    // tasti
    "Tasti rapidi": "Hotkeys",
    "Clicca un campo e premi la combinazione (es. Ctrl+Alt+A, oppure un tasto singolo come F8). Per il push-to-talk puoi anche cliccare con il":
      "Click a field and press the combination (e.g. Ctrl+Alt+A, or a single key like F8). For push-to-talk you can also click with the",
    "tasto centrale": "middle button", "o i": "or the", "tasti laterali del mouse": "side mouse buttons",
    "(Mouse4/Mouse5). Canc o Backspace svuota il campo (tasto disattivato).": "(Mouse4/Mouse5). Delete or Backspace clears the field (key disabled).",
    "Mostra / nascondi": "Show / hide", "Push-to-talk (parla)": "Push-to-talk (talk)", "Scrivi ad Alter Ego": "Write to Alter Ego", "Click-through": "Click-through",
    "Salva tasti": "Save hotkeys", "premi i tasti…": "press the keys…", "tasti salvati ✓": "hotkeys saved ✓",

    // clip
    "Clip vocali": "Voice clips",
    "Quando dici una di queste frasi col push-to-talk (o la scrivi), Alter Ego preme la scorciatoia del tuo programma di registrazione":
      "When you say one of these phrases with push-to-talk (or write it), Alter Ego presses the shortcut of your recording program",
    "senza passare dal modello AI": "without going through the AI model",
    ": è istantaneo e gratis. La registrazione in background (Instant Replay / Instant Replay NVIDIA / Medal) deve essere attiva.":
      ": it's instant and free. Background recording (Instant Replay / NVIDIA Instant Replay / Medal) must be on.",
    "Clip vocali attive": "Voice clips on", "Frasi (una per riga)": "Phrases (one per line)", "Programmi": "Programs",
    "Attiva quelli che usi e controlla che la scorciatoia sia uguale a quella impostata nel programma.": "Enable the ones you use and check that the shortcut matches the one set in the program.",
    "Risposta di Alter Ego": "Alter Ego's reply", "Clip salvata!": "Clip saved!", "Prova (invia la scorciatoia tra 3 secondi)": "Test (sends the shortcut in 3 seconds)",
    "+ Aggiungi programma": "+ Add program", "Altro programma": "Other program", "torna al gioco… {i}": "back to the game… {i}",

    // permessi
    "Permessi delle azioni sul PC": "PC action permissions", "Tutto è": "Everything is", "spento di default": "off by default",
    ". Alter Ego può usare solo ciò che attivi qui; le azioni con ⚠ chiedono conferma ogni volta nella sua finestra. Ogni azione finisce nel Registro.":
      ". Alter Ego can only use what you enable here; actions marked ⚠ ask for confirmation every time in her window. Every action goes to the Log.",
    "Non chiedere conferma": "Don't ask for confirmation", "(esegui subito le azioni ⚠)": "(run ⚠ actions right away)",
    "Le azioni restano limitate a quelle attivate qui sotto e alle cartelle e ai comandi in lista, e finiscono comunque nel Registro.":
      "Actions stay limited to those enabled below and to the listed folders and commands, and still go to the Log.",
    "Rischio:": "Risk:", "un testo letto da file, schermo o web potrebbe spingere il modello a usare un'azione senza che tu la veda prima.":
      "text read from files, the screen or the web could push the model into using an action before you see it.",
    "Aprire programmi e siti": "Open programs and websites",
    "Senza conferma quelli della lista; ⚠ conferma per altri siti. Nome = come lo chiami a voce; destinazione = percorso .exe, collegamento o URL.":
      "Listed ones without confirmation; ⚠ confirmation for other websites. Name = what you call it; target = .exe path, shortcut or URL.",
    "Nome|Percorso o URL": "Name|Path or URL", "Gestione file": "File management",
    "Leggere, scrivere, elencare ed eliminare file solo in queste cartelle. Conferma sempre.": "Read, write, list and delete files only in these folders. Always asks.",
    "Cartella autorizzata": "Allowed folder", "Promemoria, timer e sveglie": "Reminders, timers and alarms", "(notifiche di Windows)": "(Windows notifications)",
    "Appunti": "Clipboard", "(⚠ conferma per leggere)": "(⚠ asks before reading)", "Lettura schermo": "Screen reading",
    "⚠ (screenshot al modello con visione)": "⚠ (screenshot to a vision model)", "Volume e luminosità": "Volume and brightness", "Comandi": "Commands",
    "⚠ (solo quelli in lista, mai testo libero)": "⚠ (listed ones only, never free text)", "Nome|Comando": "Name|Command",
    "Ricerca web": "Web search", "(API Search e Fetch di DeepSeek)": "(DeepSeek Search and Fetch API)",
    "Alter Ego cerca sul web e legge le pagine quando serve. Usa la chiave DeepSeek (scheda AI e chiavi): ogni ricerca o pagina costa circa $0,00075 e finisce nella scheda Costi. Il testo trovato è trattato come dato.":
      "Alter Ego searches the web and reads pages when needed. It uses the DeepSeek key (AI & keys tab): each search or page costs about $0.00075 and shows up in the Costs tab. Text found is treated as data.",
    "(cerca su YouTube e suona)": "(searches YouTube and plays)", "Al primo uso scarica": "On first use it downloads", "e": "and", "da GitHub in": "from GitHub into",
    ". I brani restano in una cache (ultimi 30).": ". Tracks stay in a cache (last 30).", "Terminale": "Terminal",
    "⚠ (PowerShell e cmd, comandi scritti da Alter Ego)": "⚠ (PowerShell and cmd, commands written by Alter Ego)",
    "Alter Ego può eseguire qualsiasi comando non interattivo (timeout 120 s) e leggerne l'output.": "Alter Ego can run any non-interactive command (120 s timeout) and read its output.",
    "In autonomia": "Autonomous", ": senza chiedere conferma": ": without asking for confirmation",
    "Anche in autonomia, i comandi potenzialmente distruttivi (formattazioni, cancellazioni ricorsive, registro di sistema, utenti, spegnimento, download da internet…) chiedono sempre conferma.":
      "Even in autonomous mode, potentially destructive commands (formatting, recursive deletion, system registry, users, shutdown, downloads from the internet…) always ask for confirmation.",
    "+ Aggiungi": "+ Add", "Alter Ego eseguirà le azioni attivate senza chiederti conferma. Continuare?": "Alter Ego will run the enabled actions without asking you. Continue?",

    // costi
    "Ogni richiesta a pagamento con token, cache, durata e costo stimato. I costi sono calcolati con i prezzi qui sotto (OpenRouter riporta il costo effettivo).":
      "Every paid request with tokens, cache, duration and estimated cost. Costs are computed with the prices below (OpenRouter reports the actual cost).",
    "Verifica i prezzi sul sito di ogni provider": "Check the prices on each provider's website", ": cambiano spesso e DeepSeek ha fasce orarie scontate.": ": they change often and DeepSeek has discounted hours.",
    "Per servizio e modello": "By service and model", "Richieste": "Requests", "Input": "Input", "Cache": "Cache", "Output": "Output", "Costo": "Cost",
    "Tutte": "All", "Chat": "Chat", "Ricerche web": "Web searches", "Pagine web": "Web pages", "Trascrizioni": "Transcriptions", "Esporta CSV": "Export CSV",
    "Svuota": "Clear", "Quando": "When", "Tipo": "Type", "Servizio / modello": "Service / model", "input non in cache": "input not from cache",
    "input letto dalla cache": "input read from cache", "Durata": "Duration", "Prezzi (dollari)": "Prices (US dollars)",
    "Modello vuoto = tutti i modelli del servizio; altrimenti vale per i modelli che iniziano così. Token: $ per milione.":
      "Empty model = all the service's models; otherwise it applies to models starting with it. Tokens: $ per million.",
    "$ / chiamata": "$ / call", "$ / ora audio": "$ / audio hour", "+ Prezzo": "+ Price", "Ripristina predefiniti": "Restore defaults",
    "Oggi": "Today", "Ultimi 7 giorni": "Last 7 days", "Questo mese": "This month", "Totale registrato": "Total recorded", "Input dalla cache": "Input from cache",
    "{n} richieste": "{n} requests", "{a} di {b} token": "{a} of {b} tokens", "nessuna richiesta registrata": "no requests recorded", "nessuna richiesta": "no requests",
    "chat": "chat", "riassunto": "summary", "ricerca web": "web search", "pagina web": "web page", "trascrizione": "transcription",
    "{s} s audio": "{s} s audio", "Ripristinare i prezzi predefiniti?": "Restore the default prices?", "Salvato: {path}": "Saved: {path}",
    "Svuotare il registro dei costi?": "Clear the cost log?",

    // registro
    "Registro azioni": "Action log", "Azione": "Action", "Dettaglio": "Detail", "Esito": "Result",
    "nessuna azione registrata": "no actions recorded", "Svuotare il registro delle azioni?": "Clear the action log?",
    "l'utente ha negato il permesso": "the user denied permission", "approvata in automatico": "auto-approved", "notificato": "notified",
  };

  let lang = "it";
  const orig = new WeakMap(); // nodo/attributo → testo italiano originale
  const norm = (s) => s.replace(/\s+/g, " ").trim();

  function t(s, vars) {
    let r = lang === "en" ? EN[s] ?? s : s;
    if (vars) r = r.replace(/\{(\w+)\}/g, (_, k) => (vars[k] ?? ""));
    return r;
  }

  function translateText(node) {
    const cur = node.nodeValue;
    if (!cur || !cur.trim()) return;
    let it = orig.get(node);
    // testo cambiato dal codice dopo l'ultima traduzione: è il nuovo originale
    if (it === undefined || (cur !== it.it && cur !== it.en)) {
      it = { it: cur, en: null };
      orig.set(node, it);
    }
    const key = norm(it.it);
    const en = EN[key];
    if (en === undefined) return;
    const lead = /^\s/.test(it.it) ? " " : "";
    const trail = /\s$/.test(it.it) ? " " : "";
    it.en = lead + en + trail;
    const want = lang === "en" ? it.en : it.it;
    if (cur !== want) node.nodeValue = want;
  }

  const ATTRS = ["placeholder", "title", "aria-label", "data-cols"];
  function translateAttrs(el) {
    for (const a of ATTRS) {
      const cur = el.getAttribute?.(a);
      if (!cur) continue;
      const k = "__i18n_" + a;
      if (!el[k] || (cur !== el[k].it && cur !== el[k].en)) el[k] = { it: cur, en: EN[norm(cur)] };
      const want = lang === "en" && el[k].en ? el[k].en : el[k].it;
      if (cur !== want) el.setAttribute(a, want);
    }
  }

  function translateTree(root) {
    if (root.nodeType === 3) return translateText(root);
    if (root.nodeType !== 1 && root.nodeType !== 9) return;
    if (root.nodeType === 1) {
      if (["SCRIPT", "STYLE", "TEXTAREA"].includes(root.tagName) || root.closest?.("[data-no-i18n]")) return;
      translateAttrs(root);
    }
    const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    let n;
    while ((n = w.nextNode())) {
      if (n.nodeType === 1) {
        if (["SCRIPT", "STYLE", "TEXTAREA"].includes(n.tagName)) continue;
        translateAttrs(n);
      } else if (!n.parentElement?.closest("textarea, [data-no-i18n], #dialog-text, #dialog-measure, #lines")) {
        translateText(n);
      }
    }
  }

  // testi aggiunti o cambiati dal codice dopo il caricamento
  let busy = false;
  const obs = new MutationObserver((muts) => {
    if (busy) return;
    busy = true;
    for (const m of muts) {
      if (m.type === "characterData") translateText(m.target);
      else if (m.type === "attributes") translateAttrs(m.target);
      else m.addedNodes.forEach((n) => translateTree(n));
    }
    obs.takeRecords();
    busy = false;
  });

  function setLang(l) {
    lang = l === "en" ? "en" : "it";
    document.documentElement.lang = lang;
    document.title = t(norm(document.title)) || document.title;
    busy = true;
    translateTree(document.body);
    busy = false;
  }

  /** "auto" → lingua di Windows (navigator.language). */
  function resolve(pref) {
    if (pref === "it" || pref === "en") return pref;
    return (navigator.language || "en").toLowerCase().startsWith("it") ? "it" : "en";
  }

  // gli script sono in fondo al body: il body esiste già, l'osservatore parte subito
  const start = () => obs.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
  if (document.body) start();
  else document.addEventListener("DOMContentLoaded", start);

  return { t, setLang, resolve, lang: () => lang, translateTree };
})();
const t = I18N.t;
