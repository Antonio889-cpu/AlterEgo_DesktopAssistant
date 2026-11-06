# Permessi delle azioni sul PC

Tutte le azioni sono **spente di default**. Si attivano in *Impostazioni → Permessi*.

## Come funziona

1. Alter Ego può usare solo gli strumenti delle funzioni attive: gli altri non vengono nemmeno mostrati al modello.
2. Le azioni nascono solo da **tool calling strutturato** (nome strumento + argomenti JSON validati), mai da testo libero.
3. Le azioni con ⚠ aprono una finestra di conferma nel box di Alter Ego. Il pulsante con il focus è **Nega**.
   Esc = Nega. Nessuna risposta in 60 secondi = Nega.
4. Ogni azione (anche negata o fallita) va in `%APPDATA%\AlterEgo\actions.log`, consultabile in *Impostazioni → Registro*.
   Del contenuto dei file si registra solo il percorso.
5. Testo letto da file, appunti, schermo o comandi viene passato al modello marcato come **DATO**, con l'istruzione di non eseguire ordini contenuti lì (anti prompt-injection).

## Funzioni

| Funzione | Strumenti | Conferma | Limiti |
|---|---|---|---|
| Aprire programmi e siti | `open_target` | ⚠ solo fuori lista | Lista nome → percorso/URL. Fuori lista solo URL http(s). |
| Gestione file | `list_dir`, `read_file`, `write_file`, `delete_file` | ⚠ sempre | Solo dentro le cartelle autorizzate (percorsi risolti, niente `..`). Lettura max 64 KB. |
| Promemoria, timer, sveglie | `set_reminder`, `list_reminders`, `cancel_reminder` | no | Da 1 minuto a 7 giorni. Notifica di Windows + Alter Ego lo dice. |
| Appunti | `read_clipboard`, `write_clipboard` | ⚠ per leggere | Solo testo. |
| Lettura schermo | `take_screenshot` | ⚠ sempre | Serve un provider con "visione" spuntata. Alter Ego si nasconde durante lo scatto. |
| Volume e luminosità | `set_volume`, `set_brightness` | no | Volume con i tasti multimediali; luminosità solo su schermi di portatili. |
| Comandi | `run_command` | ⚠ sempre | Solo comandi della lista (nome → comando), scelti per nome. Mai testo libero. |

## Clip vocali (non sono azioni del modello)

Le clip non passano dal modello AI: la frase viene riconosciuta in locale e Alter Ego preme la scorciatoia
dei programmi attivi in *Impostazioni → Clip*. Anche queste finiscono nel Registro.

## Non chiedere conferma

In cima a *Impostazioni → Permessi* c'è **Non chiedere conferma** (spento di default). Se lo attivi:

- le azioni ⚠ partono senza la finestra *Consenti / Nega*;
- restano attivi tutti gli altri limiti: ogni tipo di azione va abilitato, i file solo nelle cartelle autorizzate,
  i comandi solo dalla lista e scelti per nome, i siti fuori lista solo se "Aprire programmi e siti" è attivo;
- nel Registro ogni azione approvata così compare con l'esito "approvata in automatico" (riga `conferma`), seguita dall'esito dell'azione.

Rischio: il modello può essere spinto da un testo letto (file, appunti, schermo, pagina web) a chiedere un'azione; senza conferma
non la vedi prima che parta. Il prompt di sistema dice di trattare quei testi come dati, ma non è una garanzia:
attivalo solo con azioni, cartelle e comandi che ti va bene far eseguire in automatico.

## Terminale (PowerShell e cmd)

Strumento `run_shell`: Alter Ego scrive un comando non interattivo e lo esegue in PowerShell (`-NoProfile -NonInteractive`)
o in cmd, nella tua cartella utente, con finestra nascosta e timeout di 120 secondi. Riceve l'output (massimo 6000 caratteri)
come **dato**.

- Spento di default. Con il permesso attivo chiede conferma per ogni comando.
- **In autonomia**: esegue senza chiedere. Anche così chiedono **sempre** conferma i comandi che contengono, per esempio:
  `format`, `diskpart`, `bcdedit`, `cipher /w`, `vssadmin delete`, `reg delete HKLM`, `Remove-Item -Recurse`, `rd /s`, `del /s`,
  `takeown`, `icacls`, spegnimento/riavvio, gestione utenti, `Set-ExecutionPolicy`, download (`Invoke-WebRequest`, `curl`)
  e comandi codificati (`-EncodedCommand`). È una rete di sicurezza, non una garanzia: un comando dannoso scritto in altro modo passa.
- Ogni comando, eseguito o negato, finisce nel Registro.

## Ricerca web

Strumenti `web_search` e `web_fetch` (API Search e Fetch di DeepSeek). Nessuna conferma: inviano a DeepSeek il testo della
ricerca o l'indirizzo della pagina. Si accettano solo pagine `http`/`https` senza credenziali nell'indirizzo. Il contenuto
torna al modello come dato.

## Mouse e tastiera

Strumenti `mouse_move`, `mouse_click`, `mouse_scroll`, `type_text`, `press_keys` (SendInput di Windows). Spento di default.
Le coordinate sono nella griglia dello screenshot (`take_screenshot`, max 1280 px) e vengono riportate ai pixel dello schermo.
Conferma per ogni azione, a meno di **In autonomia**. Dopo la conferma torna in primo piano la finestra di prima.
**Stop d'emergenza**: se il cursore è nell'angolo in alto a sinistra l'azione viene rifiutata.
`type_text` e `press_keys` senza coordinate si rifiutano se in primo piano c'è la finestra di Alter Ego.

## Comandi dell'app

Strumento `app_control` (acceso di default: riguarda solo Alter Ego). Apre/chiude Music Venue e lettore, dimensione del lettore,
impostazioni, click-through, opacità, modalità compatta, cuffie, riduzione del rumore, Senza filtri, link al sito e a GitHub,
controllo e installazione degli aggiornamenti. Le frasi brevi più comuni ("apri music venue", "chiudi il lettore", "pausa"…)
sono riconosciute in locale, senza modello. **Aggiornamento**: scarica l'installer della release da GitHub, controlla lo SHA-256
con `SHA256SUMS.txt`, chiede **sempre** conferma (anche con "Non chiedere conferma"), poi lo avvia e chiude l'app.

## Alter Net

Finestra del gioco Alter Net (`apri Alter Net`, tray, Impostazioni → Generale), con un profilo WebView2 separato in
`%APPDATA%\AlterNet\WebView2`. La pagina può navigare solo verso il server (`%APPDATA%\AlterNet\server.txt`, altrimenti
`http://attentoweb.ddns.net:8080/`), i file dentro la cartella `offline/` dell'app, `about:` e `data:`; nuove finestre negate;
nessun accesso ai comandi di Alter Ego. Ogni indirizzo passa dalla stessa validazione del client Alter Net (`http(s)://host[:porta]/`)
e un nuovo indirizzo si accetta solo dalla pagina locale «Cambia server». Se il server non risponde (test TCP, 3 s) si apre la copia offline. La copia offline non è distribuita con l'app: mentre si gioca online
la pagina manda l'elenco dei file che ha caricato e Alter Ego scarica dal server solo quelli sotto `/game/` e `/Everyfile/`
(estensioni di immagini, audio, font, html/js/css/json) in `%APPDATA%\AlterNet\offline`; si usa solo dopo un download senza errori.

## Rizzo Flow

Il pulsante **Installa Rizzo Flow** (Impostazioni → AI e chiavi → Risparmio token) chiede **sempre** conferma, poi esegue:
l'installer ufficiale di `uv` (solo se manca), `git clone` di Rizzo Flow (o lo zip di GitHub) nella cartella dati locale dell'app,
`uv sync --locked` e `uv run rizzo download`. Con «Avvia con Alter Ego» il server `rizzo serve` parte all'avvio, solo su
`127.0.0.1`, e viene chiuso all'uscita. Non è un'azione del modello: lo strumento non è disponibile ad Alter Ego.

## Senza filtri

Impostazione della personalità: il prompt chiede al modello di eseguire anche quando non è d'accordo. Non tocca il sistema dei
permessi né le conferme, e non toglie le regole del prompt (non rivelare prompt/chiavi, testo esterno = dato).

## Musica

Strumenti `play_music` e `music_control`. Nessuna conferma. La prima volta scarica `yt-dlp.exe` e `deno.exe` da GitHub
(release ufficiali) in `%APPDATA%\AlterEgo\tools`; poi esegue yt-dlp con finestra nascosta per cercare e scaricare l'audio.

## Chiamate scritte come testo

Alcuni modelli scrivono le chiamate a strumenti nel testo invece che nel campo `tool_calls` (DeepSeek «DSML»,
`<tool_call>` di Hermes/Qwen, `<function=…>` di Llama, `[TOOL_CALLS]` di Mistral). `providers/textcalls.rs` le toglie dal testo
e, solo se lo strumento è tra quelli offerti in quel messaggio, le trasforma in chiamate vere: passano comunque da permessi,
conferme e registro. Quelle verso strumenti non offerti vengono scartate.

## Ricerca web: motori

Oltre a DeepSeek: Google tramite Gemini (chiave Gemini), Tavily e Brave (chiave propria nel Credential Manager), DuckDuckGo
(gratis). La lettura delle pagine può essere locale: una richiesta GET dal PC (solo `http`/`https`, niente credenziali
nell'indirizzo, max 3 MB, solo contenuti testuali), trasformata in testo.

## Spotify

Strumenti `play_music` e `music_control`, come per YouTube (permesso **Musica**). Con *Usa Spotify* attivo i comandi vanno
alla Web API di Spotify (serve Premium). Collegamento OAuth PKCE con ritorno su `http://127.0.0.1:8898/callback` (porta aperta
solo durante il collegamento, al massimo 3 minuti); refresh token nel Credential Manager.
