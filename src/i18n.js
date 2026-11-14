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
    // 1.8: modelli consigliati, memoria, Rizzo, VoiceStudio, Hermes
    "⭐ Modelli consigliati": "⭐ Recommended models",
    "di Liquid AI (1,7 miliardi di parametri, ~1 GB): pensato per agenti locali, usa bene gli": "by Liquid AI (1.7 billion parameters, ~1 GB): built for local agents, it handles",
    ", va anche senza GPU.": " well, and runs even without a GPU.",
    "di Google: strumenti, visione e ragionamento, serve più memoria.": "by Google: tools, vision and reasoning, needs more memory.",
    ": ottimo in chat e visione, ma su Ollama non supporta gli strumenti (niente azioni sul PC).": ": great for chat and vision, but on Ollama it doesn't support tools (no PC actions).",
    "⬇ Installa e usa": "⬇ Install and use",
    "Preparo il modello… (installa Ollama se manca, poi lo scarica)": "Preparing the model… (installs Ollama if missing, then downloads it)",
    "Gemma 4 E2B · strumenti, visione, audio": "Gemma 4 E2B · tools, vision, audio",
    "Gemma 4 E4B · strumenti, visione, audio": "Gemma 4 E4B · tools, vision, audio",
    "Gemma 3 1B · solo chat, ~0,8 GB": "Gemma 3 1B · chat only, ~0.8 GB",
    "Gemma 3 4B · chat e visione, niente strumenti, ~3,3 GB": "Gemma 3 4B · chat and vision, no tools, ~3.3 GB",
    "Gemma 3 12B · chat e visione, niente strumenti, ~8 GB": "Gemma 3 12B · chat and vision, no tools, ~8 GB",
    "Gemma 3 27B · chat e visione, niente strumenti, ~17 GB": "Gemma 3 27B · chat and vision, no tools, ~17 GB",
    "🗜 Compatta": "🗜 Compact",
    "Unisce i doppioni, toglie i fatti inutili e i messaggi vecchi già riassunti": "Merges duplicates, removes useless facts and old already-summarized messages",
    "Compattare la memoria? Il modello AI unisce i doppioni e toglie i fatti inutili; i messaggi vecchi già riassunti vengono eliminati (gli ultimi restano).": "Compact the memory? The AI model merges duplicates and removes useless facts; old already-summarized messages are deleted (the latest ones stay).",
    "Compatto…": "Compacting…",
    "Hai già Rizzo Flow? Incolla la sua cartella (es. D:\\AI\\rizzo-flow)": "Already have Rizzo Flow? Paste its folder (e.g. D:\\AI\\rizzo-flow)",
    "Usa questa cartella": "Use this folder",
    "Uso Rizzo Flow in {d}": "Using Rizzo Flow in {d}",
    "🎙 Usa VoiceStudio": "🎙 Use VoiceStudio",
    "Avvia VoiceStudio insieme ad Alter Ego quando è la voce in uso": "Start VoiceStudio with Alter Ego when it's the voice in use",
    "(debpalash, AGPL-3.0): voci locali e clonazione. Lo installa se manca (script ufficiale), lo avvia e imposta qui sotto il suo indirizzo. La voce si sceglie (o si clona) dentro VoiceStudio; il suo nome va nel campo «Voce».": "(debpalash, AGPL-3.0): local voices and cloning. It installs it if missing (official script), starts it and sets its address below. Pick (or clone) the voice inside VoiceStudio; its name goes in the “Voice” field.",
    "acceso ✓": "running ✓",
    "installato, spento": "installed, off",
    "Preparo VoiceStudio…": "Preparing VoiceStudio…",
    "(Nous Research): delega i compiti lunghi": "(Nous Research): delegate long tasks",
    "Hermes è un agente autonomo con strumenti, memoria e skill propri. Alter Ego gli passa i compiti lunghi (ricerche, progetti di codice, automazioni) e ti riassume il risultato; chiede conferma ogni volta. Con Rizzo Flow come router, è Rizzo a segnalare quando conviene.": "Hermes is an autonomous agent with its own tools, memory and skills. Alter Ego hands it long tasks (research, code projects, automations) and sums up the result for you; it asks for confirmation every time. With Rizzo Flow as the router, Rizzo flags when it's worth it.",
    "Hermes come cervello": "Hermes as the brain",
    ": ogni messaggio va a Hermes, Alter Ego mette voce ed espressioni": ": every message goes to Hermes, Alter Ego adds voice and expressions",
    "⬇ Installa Hermes": "⬇ Install Hermes",
    "⚙ Configura": "⚙ Set up",
    "L'installazione usa lo script ufficiale di hermes-agent.nousresearch.com; poi «Configura» apre la scelta del modello AI di Hermes.": "The installation uses the official script from hermes-agent.nousresearch.com; then “Set up” opens Hermes's AI model choice.",
    "Installo Hermes…": "Installing Hermes…",
    "Alter Ego – Catalogo modelli": "Alter Ego – Model catalog",
    "Catalogo modelli locali": "Local model catalog",
    "⭐ Consigliati": "⭐ Recommended",
    "LFM3: piccolo, ottimo con gli strumenti. Gemma 4: strumenti, visione, ragionamento. Gemma 3: chat e visione, senza strumenti su Ollama.": "LFM3: small, great with tools. Gemma 4: tools, vision, reasoning. Gemma 3: chat and vision, no tools on Ollama.",
    "Cerca un modello, oppure scegline uno consigliato qui sopra.": "Search for a model, or pick a recommended one above.",
    "Abliterated e uncensored sono versioni modificate da terzi senza i rifiuti del modello originale: usale con giudizio. I modelli scaricati compaiono nelle Impostazioni → Modelli locali.": "Abliterated and uncensored are third-party versions without the original model's refusals: use them wisely. Downloaded models show up in Settings → Local models.",
    "⭐ Catalogo dei modelli": "⭐ Model catalog",
    "Modelli consigliati (": "Recommended models (",
    "), ricerca nella libreria di Ollama e su Hugging Face, download da un indirizzo: tutto in una finestra a parte.": "), search in the Ollama library and on Hugging Face, download from an address: all in a separate window.",
    "🗂 Apri il catalogo": "🗂 Open the catalog",
    "«Usa» lo imposta come modello principale": "“Use” makes it the main model",
    // Garry's Mod / Shinri Trial
    "Un corpo è stato scoperto… Fai attenzione, per favore. Da adesso ogni dettaglio conta.": "A body has been discovered… Please be careful. From now on every detail matters.",
    "Un altro corpo… Non doveva succedere. Aiutami a scoprire la verità.": "Another body… This shouldn't have happened. Help me find the truth.",
    "Stai esaminando il corpo… Guarda le ferite, la posizione e cosa c'è intorno.": "You're examining the body… Look at the wounds, the position and what's around it.",
    "È iniziata l'indagine. Raccogli più prove che puoi, e se hai le mani sporche… lavale subito.": "The investigation has started. Gather as much evidence as you can, and if your hands are dirty… wash them right away.",
    "La partita è cominciata… Spero che nessuno ceda alla disperazione.": "The game has started… I hope nobody gives in to despair.",
    "Il processo di classe sta per iniziare. Usa le prove che hai raccolto… io credo in te!": "The class trial is about to begin. Use the evidence you gathered… I believe in you!",
    "È il momento di votare… Pensaci bene. Una scelta sbagliata condanna tutti.": "It's time to vote… Think carefully. A wrong choice dooms everyone.",
    "Avete trovato il colpevole… È la verità, anche se fa male.": "You found the culprit… It's the truth, even if it hurts.",
    "Il verdetto era sbagliato…! No… il colpevole l'ha fatta franca.": "The verdict was wrong…! No… the culprit got away with it.",
    "È finita senza vittime… Sono così sollevata! Ce l'avete fatta tutti insieme.": "It ended with no victims… I'm so relieved! You all made it together.",
    "È finita male… Mi dispiace tanto. La prossima volta andrà meglio, vero?": "It ended badly… I'm so sorry. Next time will be better, right?",
    "Blackout…! Resta vicino a qualcuno di cui ti fidi.": "Blackout…! Stay close to someone you trust.",
    "Travestimenti… adesso nessuno è chi sembra. Non fidarti dell'aspetto.": "Disguises… now nobody is who they seem. Don't trust appearances.",
    "Teletrasporto?! Guardati intorno, potresti essere ovunque…": "Teleport?! Look around, you could be anywhere…",
    "Anche tu… alla fine hai ceduto alla disperazione…": "You too… in the end you gave in to despair…",
    "Perché…? Speravo che la speranza vincesse… e invece anche tu hai ceduto alla disperazione.": "Why…? I hoped hope would win… and yet you too gave in to despair.",
    "Non posso crederci… Hai davvero scelto la disperazione? Io… non so cosa dire.": "I can't believe it… Did you really choose despair? I… don't know what to say.",
    "Ti hanno colpito! Stai bene? Allontanati, presto!": "You've been hit! Are you okay? Get away, quickly!",
    "No… non può essere… Ti prego, dimmi che è solo un gioco…": "No… it can't be… Please, tell me it's just a game…",
    "Hai bisogno di dormire… Riposa un po', per favore.": "You need some sleep… Please rest a little.",
    "Reagisci ai suoni di Shinri Trial (corpo scoperto, indagine, processo…)": "React to Shinri Trial sounds (body discovered, investigation, trial…)",
    "Mentre Garry's Mod è aperto, Alter Ego ascolta l'audio del PC e riconosce i suoni del gioco per reagire. Non tocca il gioco e non manda niente a nessuno: i suoni di riferimento vengono dall'addon Sounds @ ST del Workshop.":
      "While Garry's Mod is open, Alter Ego listens to the PC audio and recognizes the game's sounds to react. It doesn't touch the game and sends nothing to anyone: the reference sounds come from the Sounds @ ST Workshop addon.",
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
    "Ragionamento dei modelli \"thinking\"": "Reasoning of \"thinking\" models",
    "Nascosto (il modello ragiona, tu vedi solo la risposta)": "Hidden (the model reasons, you only see the answer)",
    "Spento (risposte più rapide, meno token)": "Off (faster answers, fewer tokens)",
    "Risparmio token": "Token saving",
    "Tecniche per spendere meno sulle API senza peggiorare le risposte. La lunghezza delle risposte non viene limitata.": "Techniques to spend less on the APIs without worse answers. Answer length is not limited.",
    "Massimo risparmio": "Maximum saving",
    "Predefinite": "Defaults",
    "Strumenti solo quando servono": "Tools only when needed",
    ": le definizioni (1–3 mila token) partono solo se la domanda chiede un'azione": ": the definitions (1–3 thousand tokens) are sent only when the question asks for an action",
    "Se il modello si accorge che gli serviva uno strumento lo dice e la domanda si ripete subito con gli strumenti: nessuna azione persa.": "If the model realizes it needed a tool it says so and the question is repeated right away with the tools: no action is lost.",
    "Descrizioni degli strumenti compatte": "Compact tool descriptions",
    "Accorcia le risposte vecchie nella cronologia (le ultime restano intere; la cache del provider continua a funzionare)": "Shorten old answers in the history (the latest stay whole; the provider cache keeps working)",
    "Fatti su di te inviati": "Facts about you sent",
    "Tutti": "All",
    "I 20 più pertinenti": "The 20 most relevant",
    "I 10 più pertinenti": "The 10 most relevant",
    "I 5 più pertinenti": "The 5 most relevant",
    "Screenshot al modello": "Screenshots to the model",
    "1280 px (dettagliati)": "1280 px (detailed)",
    "768 px (meno token)": "768 px (fewer tokens)",
    "Pagine web lette": "Web pages read",
    "8000 caratteri": "8000 characters",
    "5000 caratteri": "5000 characters",
    "3000 caratteri (meno token)": "3000 characters (fewer tokens)",
    "Router locale": "Local router",
    "Parole chiave (gratis, istantaneo)": "Keywords (free, instant)",
    "Rizzo Flow (modello locale, più preciso)": "Rizzo Flow (local model, more accurate)",
    "Spento": "Off",
    "URL di Rizzo Flow": "Rizzo Flow URL",
    "Prova": "Test",
    "Modello economico per le domande semplici": "Cheap model for simple questions",
    "Saluti e domande brevi (secondo il router) vanno a questo modello; tutto il resto al provider principale.": "Greetings and short questions (according to the router) go to this model; everything else to the main provider.",
    "Rizzo Flow risponde ✓": "Rizzo Flow is answering ✓",
    "Non raggiungibile: avvialo con «uv run rizzo serve»": "Not reachable: start it with “uv run rizzo serve”",
    "Google Gemini (usa la chiave Gemini)": "Google Gemini (uses the Gemini key)",
    "Se non funziona usa": "If it fails, use",
    "Nessuno": "None",
    "Google Gemini": "Google Gemini",
    "Groq Whisper": "Groq Whisper",
    "Server Whisper locale": "Local Whisper server",
    "Modello Gemini": "Gemini model",
    "Spotify (Premium)": "Spotify (Premium)",
    "Client ID": "Client ID",
    "Collega account": "Connect account",
    "Scollega": "Disconnect",
    "Usa Spotify per «metti…», pausa, avanti e volume": "Use Spotify for “play…”, pause, next and volume",
    "I brani suonati su Spotify finiscono anche nei recenti della Music Venue. Il token resta nel Credential Manager di Windows.": "Tracks played on Spotify also go to the Music Venue's recent list. The token stays in the Windows Credential Manager.",
    "Collegato: {name}": "Connected: {name}",
    "Non collegato": "Not connected",
    "Si apre il browser: accedi a Spotify e consenti l'accesso…": "The browser opens: log in to Spotify and allow access…",
    "Ricerca web": "Web search",
    "Motore di ricerca": "Search engine",
    "DeepSeek Search (chiave DeepSeek, ~$0,00075 a ricerca)": "DeepSeek Search (DeepSeek key, ~$0.00075 per search)",
    "Google tramite Gemini (chiave Gemini, risposta già riassunta)": "Google via Gemini (Gemini key, answer already summarized)",
    "Tavily (chiave Tavily, 1000 ricerche gratis al mese)": "Tavily (Tavily key, 1000 free searches a month)",
    "Brave Search (chiave Brave)": "Brave Search (Brave key)",
    "DuckDuckGo (gratis, senza chiave)": "DuckDuckGo (free, no key)",
    "Se non risponde prova gli altri disponibili (DuckDuckGo per ultimo)": "If it doesn't answer, try the other available ones (DuckDuckGo last)",
    "Lettura delle pagine": "Reading pages",
    "Automatica": "Automatic",
    "Dal PC (gratis)": "From the PC (free)",
    "DeepSeek Fetch (a pagamento, legge meglio i siti difficili)": "DeepSeek Fetch (paid, reads difficult sites better)",
    "Modello Gemini per la ricerca": "Gemini model for search",
    "Chiave Tavily": "Tavily key",
    "Chiave Brave": "Brave key",
    "Salva": "Save",
    "chiave salvata ✓": "key saved ✓",
    "Alter Ego cerca sul web e legge le pagine quando serve. Il testo trovato è trattato come dato; le ricerche finiscono nella scheda Costi.": "Alter Ego searches the web and reads pages when needed. Text found is treated as data; searches go to the Costs tab.",
    "Riduci a lettore": "Shrink to player",
    "Separa in una finestra a parte": "Detach into its own window",
    "Aggancia al lettore": "Dock to the player",
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
    "Il gioco multiplayer Alter Net, in una finestra a schermo intero senza bordi. Server:": "The Alter Net multiplayer game, in a borderless fullscreen window. Server:",
    "▶ Apri Alter Net": "▶ Open Alter Net",
    "Cambia server…": "Change server…",
    "copia offline presente": "offline copy installed",
    "copia offline non ancora pronta: si crea da sola giocando online": "offline copy not ready yet: it builds itself while you play online",
    "F11 alterna schermo intero e finestra, Esc due volte chiude. Se il server non risponde si apre la versione offline (giocatore singolo). Puoi anche dirlo a lei: «apri Alter Net», «chiudi Alter Net».": "F11 toggles fullscreen and windowed, Esc twice closes. If the server doesn't answer, the offline version opens (single player). You can also tell her: “open Alter Net”, “close Alter Net”.",
    "Alter Ego gli chiede «serve un'azione?» e «è una domanda semplice?». Se non risponde si usano le parole chiave.": "Alter Ego asks it “is an action needed?” and “is this a simple question?”. If it doesn't answer, keywords are used.",
    "Rizzo Flow su questo PC:": "Rizzo Flow on this PC:",
    "Modello 4B (4,4 GB, più preciso)": "4B model (4.4 GB, more accurate)",
    "Modello 1.7B (1,8 GB, più leggero)": "1.7B model (1.8 GB, lighter)",
    "⬇ Installa Rizzo Flow": "⬇ Install Rizzo Flow",
    "⟳ Reinstalla / aggiorna": "⟳ Reinstall / update",
    "▶ Avvia": "▶ Start",
    "■ Ferma": "■ Stop",
    "Cartella": "Folder",
    "Avvia il server di Rizzo Flow insieme ad Alter Ego (si chiude quando esci)": "Start the Rizzo Flow server together with Alter Ego (it closes when you quit)",
    "L'installazione scarica uv (se manca), il codice da GitHub e il modello, poi imposta il router su Rizzo Flow. Funziona anche senza GPU, più lento.": "The installation downloads uv (if missing), the code from GitHub and the model, then sets the router to Rizzo Flow. It also works without a GPU, more slowly.",
    "installazione in corso…": "installing…",
    "non installato": "not installed",
    "installato, server attivo": "installed, server running",
    "installato, server in avvio…": "installed, server starting…",
    "installato, server spento": "installed, server off",
    "Chiedo conferma…": "Asking for confirmation…",
    "Avvio il server (il modello impiega un po' a caricarsi)…": "Starting the server (the model takes a while to load)…",
    "Non raggiungibile: installalo o avvialo qui sotto": "Unreachable: install or start it below",
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
    // ---------- 1.6: sistema operativo, avvio, AI avanzate, Ollama, permessi, SQL, log ----------
    "Automatica (lingua del sistema)": "Automatic (system language)",
    "Avvia Alter Ego all'accesso al desktop": "Start Alter Ego when you log in to the desktop",
    "Avvio silenzioso": "Silent start",
    ": all'accesso parte nascosta, solo l'icona nella tray (si apre con la tray o con Ctrl+Alt+A)": ": at login it starts hidden, only the tray icon (open it from the tray or with Ctrl+Alt+A)",
    "Finestra": "Window", "Sempre in primo piano sopra le altre finestre": "Always on top of other windows",
    "Le chiavi sono salvate nel": "Keys are stored in the", "portachiavi del sistema": "system keyring", "(Secret Service / keyutils), mai in file.": "(Secret Service / keyutils), never in files.",
    "Impostazioni avanzate": "Advanced settings", "Token massimi per risposta": "Max tokens per answer", "0 = predefinito": "0 = default",
    "Attesa massima della risposta (s)": "Max wait for the answer (s)", "Passaggi massimi con gli strumenti": "Max tool steps",
    "Passaggi: quante volte di fila può usare strumenti per una sola richiesta (le richieste complesse ne usano di più). Se un provider non risponde entro l'attesa massima si passa al successivo.":
      "Steps: how many times in a row she can use tools for a single request (complex requests use more). If a provider doesn't answer within the max wait, the next one is used.",
    "Finestra di contesto (token)": "Context window (tokens)", "Quella del modello (predefinita)": "The model's own (default)", "Altro…": "Other…",
    "Modello in memoria dopo una risposta": "Keep the model loaded after an answer", "Predefinito di Ollama (5 minuti)": "Ollama default (5 minutes)",
    "30 minuti": "30 minutes", "2 ore": "2 hours", "Sempre (finché Ollama è aperto)": "Always (while Ollama is running)", "Scarica subito": "Unload right away",
    "Livelli sulla GPU": "GPU layers", "-1 = automatico, 0 = solo CPU": "-1 = automatic, 0 = CPU only",
    "Una finestra di contesto più grande ricorda più conversazione e risultati degli strumenti, ma usa più memoria (RAM/VRAM). Con almeno una di queste opzioni Alter Ego parla con Ollama tramite la sua API nativa (":
      "A bigger context window remembers more conversation and tool results, but uses more memory (RAM/VRAM). With at least one of these options Alter Ego talks to Ollama through its native API (",
    "Voce di sistema (espeak-ng)": "System voice (espeak-ng)", "Voci di sistema (espeak-ng, offline)": "System voices (espeak-ng, offline)",
    "L'installazione crea un ambiente Python dedicato in": "The installation creates a dedicated Python environment in",
    "Su Linux l'installazione automatica non c'è: avvia Kokoro-FastAPI (o un server compatibile) e metti qui sotto il suo URL.": "On Linux there is no automatic installation: start Kokoro-FastAPI (or a compatible server) and put its URL below.",
    "Le voci di sistema usano espeak-ng: installalo (es.": "System voices use espeak-ng: install it (e.g.", "). Per una voce più naturale usa Kokoro.": "). For a more natural voice use Kokoro.",
    "I brani suonati su Spotify finiscono anche nei recenti della Music Venue.": "Tracks played on Spotify also go to the Music Venue's recent list.",
    "Il token resta nel Credential Manager di Windows.": "The token stays in the Windows Credential Manager.", "Il token resta nel portachiavi del sistema.": "The token stays in the system keyring.",
    "Crea un cavo virtuale con PipeWire/PulseAudio (resta fino al riavvio; per renderlo fisso aggiungi i comandi al tuo avvio):": "Create a virtual cable with PipeWire/PulseAudio (it lasts until reboot; to keep it, add the commands to your startup):",
    "Attiva Mic-Link qui sotto, scegli il tuo microfono e come cavo virtuale": "Turn on Mic-Link below, choose your microphone and, as the virtual cable,",
    "Il microfono resta usabile anche dal push-to-talk.": "The microphone still works for push-to-talk.",
    "Se dopo l'installazione di VB-CABLE non senti più niente, Windows lo ha messo come uscita predefinita: rimetti i tuoi altoparlanti in Impostazioni → Sistema → Audio.":
      "If you hear nothing after installing VB-CABLE, Windows made it the default output: switch back to your speakers in Settings → System → Sound.",
    "Predefinito del sistema{dev}": "System default{dev}", "Microfono predefinito del sistema": "System default microphone",
    "Leggere, scrivere, cercare, spostare ed eliminare file solo in queste cartelle. Conferma ogni volta (salvo «Non chiedere conferma»).": "Read, write, search, move and delete files only in these folders. Asks every time (unless “Don't ask for confirmation”).",
    "Tutto il disco": "Whole disk", ": qualsiasi file e cartella del PC, non solo quelli in lista": ": any file and folder on the PC, not only those in the list",
    "Le eliminazioni e le modifiche nelle cartelle di sistema (": "Deletions and changes in system folders (", "C:\\Windows, Programmi…": "C:\\Windows, Program Files…", ") chiedono sempre conferma.": ") always ask for confirmation.",
    "(notifiche del sistema)": "(system notifications)", "(se non sono già installati). I brani restano in una cache (ultimi 30).": "(if they aren't installed already). Tracks stay in a cache (last 30).",
    "Righelli con le coordinate e cursore cerchiato negli screenshot (clic molto più precisi)": "Coordinate rulers and circled cursor in screenshots (much more precise clicks)",
    "Screenshot di controllo dopo ogni clic (serve «Lettura schermo»; più token, meno errori)": "Check screenshot after every click (needs “Screen reading”; more tokens, fewer mistakes)",
    "Per i punti piccoli Alter Ego ingrandisce la zona prima di cliccare.": "For small targets Alter Ego zooms into the area before clicking.",
    "Nelle app di Windows legge anche la posizione esatta di pulsanti e campi (UI Automation).": "In Windows apps she also reads the exact position of buttons and fields (UI Automation).",
    "Su Linux serve": "On Linux you need", "(X11; su Wayland solo per le app XWayland).": "(X11; on Wayland only for XWayland apps).",
    "PowerShell e cmd": "PowerShell and cmd", "bash e sh": "bash and sh", ", comandi scritti da Alter Ego)": ", commands written by Alter Ego)",
    "Alter Ego può eseguire qualsiasi comando non interattivo e leggerne l'output. Ricorda la cartella di lavoro tra un comando e l'altro.": "Alter Ego can run any non-interactive command and read its output. She remembers the working folder between commands.",
    "Durata massima di un comando": "Max duration of a command", "1 minuto": "1 minute", "2 minuti": "2 minutes", "5 minuti": "5 minutes", "15 minuti": "15 minutes",
    "Anche in autonomia, i comandi potenzialmente distruttivi (formattazioni, cancellazioni ricorsive, registro di sistema, utenti, spegnimento, download da internet":
      "Even on her own, potentially destructive commands (formatting, recursive deletes, system registry, users, shutdown, downloads from the internet",
    ", sudo": ", sudo", "…) chiedono sempre conferma.": "…) always ask for confirmation.",
    "Database SQL Server": "SQL Server database", "(su questo PC)": "(on this PC)",
    "Alter Ego si collega a SQL Server su questo computer e legge i dati (SELECT). Quando serve una connessione apre un modulo: server, database, utente e password li scrivi tu e":
      "Alter Ego connects to SQL Server on this computer and reads data (SELECT). When a connection is needed she opens a form: you type server, database, user and password, and",
    "la password non va al modello AI": "the password is not sent to the AI model", ". Si accettano solo server locali.": ". Only local servers are accepted.",
    "Modifiche": "Changes", "⚠: anche INSERT, UPDATE, DELETE, CREATE, ALTER… (conferma ogni volta; DROP, TRUNCATE e DELETE/UPDATE senza WHERE sempre)":
      "⚠: also INSERT, UPDATE, DELETE, CREATE, ALTER… (asks every time; DROP, TRUNCATE and DELETE/UPDATE without WHERE always)",
    "Mostra i risultati nella finestra SQL": "Show results in the SQL window", "Righe mandate al modello per ogni query": "Rows sent to the model per query",
    "Apri la finestra SQL": "Open the SQL window", "Connesso: {label}": "Connected: {label}", "Non connesso": "Not connected",
    "Alter Ego potrà leggere, scrivere e spostare qualsiasi file del PC (con conferma, salvo «Non chiedere conferma»). Continuare?": "Alter Ego will be able to read, write and move any file on the PC (with confirmation, unless “Don't ask for confirmation”). Continue?",
    "Alter Ego potrà modificare dati e struttura dei database (con conferma). Continuare?": "Alter Ego will be able to change database data and structure (with confirmation). Continue?",
    "Log": "Log", "Log completo": "Full log",
    "Tutto quello che succede: richieste al modello, provider usati e fallback, strumenti con esito e durata, errori, avvisi. Chiavi e password non finiscono mai qui. Utile per capire perché qualcosa non va (e da allegare a una segnalazione).":
      "Everything that happens: model requests, providers used and fallbacks, tools with result and duration, errors, warnings. Keys and passwords never end up here. Useful to find out why something goes wrong (and to attach to a report).",
    "Tutto": "Everything", "Avvisi ed errori": "Warnings and errors", "Solo errori": "Errors only", "In tempo reale": "Live", "Copia": "Copy",
    "Livello": "Level", "Area": "Area", "Messaggio": "Message", "niente da mostrare": "nothing to show", "Svuotare il log completo?": "Clear the full log?", "copiato": "copied",
    "avviso": "warning", "errore": "error",
    "Sto ancora lavorando… premi Esc per interrompere.": "Still working… press Esc to stop.", "Interrompo…": "Stopping…", "Va bene, mi fermo qui.": "All right, I'll stop here.",
    "guardo da vicino": "taking a closer look", "leggo i controlli della finestra": "reading the window's controls", "cerco i file": "searching files", "creo la cartella": "creating the folder",
    "sposto il file": "moving the file", "mi collego al database": "connecting to the database", "interrogo il database": "querying the database", "chiudo il database": "closing the database",
    // modulo e finestra SQL
    "(hexgrad, licenza Apache-2.0) gira sul tuo PC: niente chiavi, niente internet dopo il primo download.": "(hexgrad, Apache-2.0 license) runs on your PC: no keys, no internet after the first download.",
    ": se una è esaurita, in rate limit o non valida, passa alla successiva.": ": if one is exhausted, rate limited or invalid, it moves to the next one.",
    "Annulla": "Cancel", "Alter Ego – SQL Server": "Alter Ego – SQL Server", "Connetti…": "Connect…", "Disconnetti": "Disconnect", "▶ Esegui": "▶ Run", "Copia come CSV": "Copy as CSV",
    "SELECT TOP 100 * FROM …   (Ctrl+Invio esegue)": "SELECT TOP 100 * FROM …   (Ctrl+Enter runs)",
    "Le query di Alter Ego compaiono qui insieme alle tue. Le tue non chiedono conferma; servono comunque i permessi «Database SQL Server» (e «Modifiche» per cambiare dati o struttura).":
      "Alter Ego's queries show up here together with yours. Yours don't ask for confirmation, but they still need the “SQL Server database” permission (and “Changes” to modify data or structure).",
    "tu": "you", "{n} righe{more} · {ms} ms": "{n} rows{more} · {ms} ms", "{n} righe modificate · {ms} ms": "{n} rows affected · {ms} ms",
    "Permesso «Database SQL Server» spento": "“SQL Server database” permission off",
    // ---------- 1.7: modelli locali, LFM3, GPU, token, app aperte, Graphify ----------
    "Modelli locali": "Local models", "Modelli locali (Ollama)": "Local models (Ollama)", "I modelli girano sul tuo PC con": "Models run on your PC with",
    ": niente chiavi, niente costi, nessun dato esce dal computer. Alter Ego installa Ollama se manca, scarica i modelli e li configura.": ": no keys, no costs, no data leaves the computer. Alter Ego installs Ollama if it's missing, downloads the models and sets them up.",
    "⬇ Installa / aggiorna Ollama": "⬇ Install / update Ollama", "Avvia Ollama insieme ad Alter Ego quando è il provider attivo": "Start Ollama with Alter Ego when it's the active provider",
    "💧 LFM3 di Liquid AI": "💧 LFM3 by Liquid AI", "Modello da 1,7 miliardi di parametri per agenti locali:": "A 1.7-billion-parameter model for local agents:",
    ", ragionamento, contesto fino a 256K token, 16 lingue tra cui l'italiano. Circa 1 GB in Q4: va anche senza GPU. Licenza LFM Open License v1.0. Liquid AI non lo consiglia per il ragionamento complesso a molti passaggi (è piccolo).":
      ", reasoning, context up to 256K tokens, 16 languages including Italian. About 1 GB in Q4: it runs even without a GPU. LFM Open License v1.0. Liquid AI doesn't recommend it for complex multi-step reasoning (it's small).",
    "usalo come modello principale": "use it as the main model", "⬇ Installa e usa LFM3": "⬇ Install and use LFM3",
    "LFM3 1.7B · Q4, ~1,1 GB (consigliato)": "LFM3 1.7B · Q4, ~1.1 GB (recommended)", "LFM3 1.7B · Q4_0 QAD, ~1 GB (addestrato per la quantizzazione)": "LFM3 1.7B · Q4_0 QAD, ~1 GB (quantization-aware)",
    "LFM3 1.7B · Q8_0, ~1,8 GB (più preciso)": "LFM3 1.7B · Q8_0, ~1.8 GB (more precise)", "LFM3-VL 2B · vede gli screenshot, ~1,3 GB (sperimentale)": "LFM3-VL 2B · sees screenshots, ~1.3 GB (experimental)",
    "Modelli installati": "Installed models", "Parametri": "Parameters", "Quant.": "Quant.", "Dimensione": "Size", "In memoria": "In memory", "Usa": "Use",
    "GPU 100%": "GPU 100%", "solo CPU": "CPU only", "GPU {p}% · CPU {c}%": "GPU {p}% · CPU {c}%", "nessun modello installato": "no models installed", "Ollama è spento": "Ollama is off",
    "Eliminare il modello {name} da Ollama?": "Delete the model {name} from Ollama?",
    "Ollama non è installato. Premi «Installa / aggiorna Ollama» (o «Installa e usa LFM3»: fa tutto da solo).": "Ollama is not installed. Press “Install / update Ollama” (or “Install and use LFM3”: it does everything by itself).",
    "Ollama installato ma spento ({bin}).": "Ollama installed but off ({bin}).", "Ollama {v} acceso su {base}{lfm}": "Ollama {v} running at {base}{lfm}",
    " · per LFM3 serve una versione più recente": " · LFM3 needs a newer version", "Ollama {v} pronto.": "Ollama {v} ready.", "Ollama {v} acceso.": "Ollama {v} running.",
    "Preparo LFM3… (installa Ollama se manca, poi scarica il modello)": "Preparing LFM3… (installs Ollama if missing, then downloads the model)",
    "Scaricato: {name}. Premi «Usa» nella tabella per usarlo.": "Downloaded: {name}. Press “Use” in the table to use it.", "{name} pronto": "{name} ready",
    "Cerca e scarica": "Search and download", "Libreria ufficiale di Ollama": "Official Ollama library", "Modelli della community (ollama.com)": "Community models (ollama.com)",
    "Hugging Face (GGUF)": "Hugging Face (GGUF)", "es. qwen, gemma, llama, mistral…": "e.g. qwen, gemma, llama, mistral…", "Cerca": "Search", "Cerco…": "Searching…",
    "abliterated": "abliterated", "uncensored": "uncensored", "heretic": "heretic", "strumenti": "tools", "visione": "vision", "ragionamento": "reasoning",
    "Qualsiasi dimensione": "Any size", "fino a 2B": "up to 2B", "fino a 4B": "up to 4B", "fino a 8B (≈ 8 GB di VRAM)": "up to 8B (≈ 8 GB of VRAM)", "fino a 14B": "up to 14B", "fino a 32B": "up to 32B",
    "Abliterated e uncensored sono versioni modificate da terzi senza i rifiuti del modello originale: usale con giudizio. I filtri «strumenti/visione/ragionamento» valgono per la libreria ufficiale e, quando dichiarato, per Hugging Face.":
      "Abliterated and uncensored are third-party versions without the original model's refusals: use them with judgment. The “tools/vision/reasoning” filters apply to the official library and, when declared, to Hugging Face.",
    "Nessun modello trovato con questi filtri.": "No models found with these filters.", "quantizzazione predefinita (Q4_K_M)": "default quantization (Q4_K_M)", "⬇ Scarica": "⬇ Download",
    "Da un indirizzo": "From an address", "hf.co/utente/modello-GGUF:Q4_K_M · link di Hugging Face · ollama.com/… · https://…/file.gguf": "hf.co/user/model-GGUF:Q4_K_M · Hugging Face link · ollama.com/… · https://…/file.gguf",
    "Da Hugging Face si può indicare anche il file (link al .gguf) o la quantizzazione dopo «:». Un .gguf da altri siti viene scaricato, controllato (SHA-256) e importato in Ollama.":
      "From Hugging Face you can also give the file (link to the .gguf) or the quantization after “:”. A .gguf from other sites is downloaded, checked (SHA-256) and imported into Ollama.",
    "Scheda video": "Graphics card", "Nessuna scheda video trovata.": "No graphics card found.", "integrata": "integrated", "Ollama usa: {list}": "Ollama uses: {list}",
    "Ollama non ha ancora segnalato le GPU (avvialo, poi «Aggiorna»): senza GPU usa la CPU.": "Ollama hasn't reported its GPUs yet (start it, then “Refresh”): without a GPU it uses the CPU.",
    "Motore": "Backend", "Automatico (decide Ollama)": "Automatic (Ollama decides)", "Vulkan (consigliato per AMD RX 6000 e su Windows)": "Vulkan (recommended for AMD RX 6000 and on Windows)",
    "ROCm (AMD RX 7000/9000, Linux)": "ROCm (AMD RX 7000/9000, Linux)", "Solo CPU": "CPU only", "Scheda da usare": "Card to use", "Decide Ollama": "Ollama decides",
    "Flash attention": "Flash attention", "Cache del contesto": "Context cache", "f16 (piena)": "f16 (full)", "q8_0 (metà memoria, consigliata)": "q8_0 (half the memory, recommended)", "q4_0 (un quarto)": "q4_0 (a quarter)",
    "Applica e riavvia Ollama": "Apply and restart Ollama", "Suggerito: {b}.": "Suggested: {b}.",
    "Sui PC con CPU Intel e scheda AMD, Ollama può scegliere la grafica integrata: indica la scheda dedicata. «Emulazione CUDA» (ZLUDA) non è supportata da Ollama; con le Radeon la strada veloce e stabile è Vulkan, già incluso.":
      "On PCs with an Intel CPU and an AMD card, Ollama may pick the integrated graphics: choose the dedicated card. “CUDA emulation” (ZLUDA) isn't supported by Ollama; with Radeon cards the fast, stable path is Vulkan, already included.",
    "Su Windows le impostazioni vanno nelle variabili dell'utente e Ollama viene riavviato.": "On Windows the settings go into the user's environment variables and Ollama is restarted.",
    "Automatico (spento per le domande semplici, nascosto per le altre)": "Automatic (off for simple questions, hidden for the others)",
    "Strumenti a gruppi": "Tools in groups", ": manda solo i gruppi che la domanda chiede (file, web, schermo…); gli altri li carica lei se servono": ": sends only the groups the question asks for (files, web, screen…); she loads the others if needed",
    "Ragionamenti lunghi compatti": "Compact long reasoning", ": nelle richieste con molti passaggi i risultati vecchi degli strumenti, già usati, vengono accorciati": ": in requests with many steps, old tool results that were already used get shortened",
    "Contesto": "Context", "Alter Ego sa quali applicazioni principali sono aperte (solo i nomi, mai i titoli delle finestre)": "Alter Ego knows which main applications are open (names only, never window titles)",
    "L'elenco va in fondo alla domanda, non nel prompt di sistema: la cache del provider continua a funzionare.": "The list goes at the end of the question, not in the system prompt: the provider's cache keeps working.",
    "Mappa dei progetti": "Project maps", "(Graphify)": "(Graphify)",
    "Alter Ego trasforma una cartella di codice e documenti in una mappa (grafo di conoscenza) e la interroga invece di leggere i file uno per uno: risposte su progetti grandi con molti meno token. Il codice si analizza in locale, senza AI. Usa le cartelle della":
      "Alter Ego turns a folder of code and documents into a map (knowledge graph) and queries it instead of reading files one by one: answers about large projects with far fewer tokens. Code is analysed locally, without AI. It uses the folders of",
    "(o tutto il disco); le mappe stanno nella cartella dati di Alter Ego, non nei tuoi progetti.": "(or the whole disk); maps are kept in Alter Ego's data folder, not in your projects.",
    "⬇ Installa Graphify": "⬇ Install Graphify", "Aggiorna Graphify": "Update Graphify", "installato ✓": "installed ✓", "non installato": "not installed", "installazione…": "installing…",
    "Graphify": "Graphify", "(Graphify Labs, Apache-2.0) si installa con uv dal pacchetto ufficiale «graphifyy».": "(Graphify Labs, Apache-2.0) is installed with uv from the official “graphifyy” package.",
    "prendo altri strumenti": "getting more tools", "mappo il progetto": "mapping the project", "consulto la mappa": "checking the map",
    "Query": "Query", "Nessun risultato ancora: le query di Alter Ego e le tue compaiono qui.": "No results yet: Alter Ego's queries and yours show up here.",
    "Le tue query non chiedono conferma, ma servono i permessi «Database SQL Server» (e «Modifiche» per cambiare dati o struttura).": "Your queries don't ask for confirmation, but they need the “SQL Server database” permission (and “Changes” to modify data or structure).",
    "{n} righe modificate.": "{n} rows affected.",
    // 1.9: pet
    "Alter Ego – Pet": "Alter Ego – Pet",
    "Trascinami e lanciami · doppio click: torno nella TV": "Drag me and throw me · double-click: back into the TV",
    "Pet": "Pet",
    "Nei momenti di calma Alter Ego esce dalla TV e vive sul tuo desktop: cammina, salta, si siede sopra le finestre aperte e sulla barra delle applicazioni. Prendila col mouse per spostarla o lanciarla; un click la fa saltare, un": "In quiet moments Alter Ego comes out of the TV and lives on your desktop: she walks, jumps, sits on top of open windows and on the taskbar. Grab her with the mouse to move or throw her; a click makes her jump, a",
    "doppio click": "double-click",
    "la riporta nella TV.": "brings her back into the TV.",
    "Torna nella TV da sola quando le parli (push-to-talk o box di testo), quando deve dirti qualcosa, con": "She goes back into the TV by herself when you talk to her (push-to-talk or text box), when she has something to tell you, with",
    "o dopo il tempo scelto qui sotto. Non esce mentre suona la musica, e si nasconde quando un gioco o un video è a schermo intero.": "or after the time chosen below. She doesn't go out while music is playing, and hides when a game or a video is full screen.",
    "Su Linux sta in piedi solo sulla barra e sul fondo dello schermo (le finestre si leggono solo su Windows).": "On Linux she only stands on the taskbar and the bottom of the screen (windows are only read on Windows).",
    "Esce da sola nei momenti di calma": "Goes out by herself in quiet moments",
    "Dopo quanti minuti di calma": "After how many quiet minutes",
    "Quanto spesso": "How often",
    "Raramente": "Rarely",
    "A volte": "Sometimes",
    "Spesso": "Often",
    "Torna nella TV da sola dopo": "Goes back into the TV by herself after",
    "10 minuti": "10 minutes",
    "20 minuti": "20 minutes",
    "45 minuti": "45 minutes",
    "Mai (finché non la chiami)": "Never (until you call her)",
    "{n} minuti": "{n} minutes",
    "Esplora adesso": "Explore now",
    "Richiama nella TV": "Call back into the TV",
    "è fuori a esplorare": "is out exploring",
    "è nella TV": "is in the TV",
    "Sprite del pet": "Pet sprites",
    "Il pet in uso è": "The pet in use is",
    ". Puoi importarne un altro da un file": ". You can import another one from a",
    "con gli indirizzi delle immagini (": "file with the image addresses (",
    "…) e": "…) and",
    ": le immagini vengono scaricate una volta nella cartella dei pet. Il foglio sprite ha 2 righe di fotogrammi quadrati: fermo, fermo, salto, caduta · corsa.": ": the images are downloaded once into the pets folder. The sprite sheet has 2 rows of square frames: still, still, jump, fall · run.",
    "Pet in uso": "Pet in use",
    "Importa pet (JSON)…": "Import pet (JSON)…",
    "Apri cartella pet": "Open pets folder",
    "Elimina pet importato": "Delete imported pet",
    " (sprite di {a})": " (sprites by {a})",
    "scarico le immagini…": "downloading the images…",
    "importato: {name} ✓": "imported: {name} ✓",
    "Eliminare il pet importato?": "Delete the imported pet?",
    "eliminato": "deleted",
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
