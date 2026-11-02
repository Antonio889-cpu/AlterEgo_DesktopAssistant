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
