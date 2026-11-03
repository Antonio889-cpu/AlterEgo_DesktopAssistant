## Alter Ego 1.1.0

⚠ **Contiene gli sprite originali di Alter Ego (© Spike Chunsoft): solo per uso personale, non distribuire.**

### Novità
- **Musica (Music Search and Play)**: "metti *titolo*" → Alter Ego cerca su YouTube (yt-dlp, scaricato al primo uso insieme a Deno) e suona il brano in un **lettore in stile CRT**: spettro delle frequenze, titolo e artista, avanzamento con salto, casuale / indietro / play-pausa / avanti / ripeti, volume, **testo sincronizzato** (LRCLIB) e il **volto di Alter Ego che cambia a tempo di musica**. Coda dei brani, cache degli ultimi 30, volume che si abbassa mentre lei parla, musica anche in Mic-Link (facoltativo).
- **Ricerca web** con le nuove API **Search** e **Fetch** di DeepSeek (stesse chiavi della chat, ~$0,75 / 1000 chiamate).
- **Pannello Costi**: ogni richiesta (chat, riassunti, ricerche, pagine, trascrizioni) con token, cache, durata e costo; totali di oggi / 7 giorni / mese, riepilogo per modello, prezzi modificabili, esportazione CSV. OpenRouter riporta il costo effettivo.
- **Terminale**: Alter Ego può eseguire comandi PowerShell o cmd, con conferma oppure **in autonomia** (i comandi distruttivi chiedono sempre conferma).
- DeepSeek: modello predefinito `deepseek-flash` per le nuove configurazioni.

### Correzioni
- Il box di testo per scrivere non si sovrappone più al dialogo in corso (il dialogo si nasconde e torna quando chiudi il box).
- L'animazione "vecchia TV" parte solo quando la finestra viene davvero mostrata o nascosta, non più ogni volta che si apre il box di testo.

## Alter Ego 1.0.4

### Correzioni
- **Kokoro fermo su "In ascolto… (caricamento del modello…)"**: il server restava bloccato per sempre durante il caricamento di PyTorch. Causa: per chiudersi insieme ad Alter Ego leggeva stdin in un thread, e su Windows una lettura bloccata su una pipe blocca anche gli altri thread che usano gli handle standard (stallo). Ora il server controlla direttamente se il processo di Alter Ego è ancora vivo, senza usare stdin.
- Il log del server indica anche quanto ci ha messo PyTorch a caricarsi.

## Alter Ego 1.0.3

### Correzioni
- **Kokoro "connessione rifiutata (os error 10061)" durante il caricamento**: mentre Python carica PyTorch il server non riesce ad accettare connessioni e Windows le rifiutava. Ora Alter Ego segue l'avanzamento dal log del server (PyTorch → libreria → modello → voce) e la mostra nelle impostazioni; la prima frase aspetta fino a 5 minuti che il modello sia pronto. Coda di connessioni del server più ampia.
- Il server Kokoro si avvia **offline** (modello e voci già scaricati dall'installazione): un proxy o una rete lenta non bloccano più il caricamento.
- L'installazione scarica tutte le voci italiane e inglesi della lista (prima solo Sara e Heart). Se usi Nicola o altre voci inglesi, premi **Reinstalla / aggiorna** una volta.

## Alter Ego 1.0.2

### Correzioni
- **Kokoro "Server avviato: error sending request"**: le richieste ai servizi su questo PC (Kokoro, Ollama, Whisper locale) non passano più dal proxy di sistema di Windows, che le intercettava anche per 127.0.0.1 (VPN, antivirus, proxy aziendali).
- Messaggi d'errore di rete completi (con la causa vera) e, se il server Kokoro è avviato ma non pronto, l'ultima riga del suo log.
- Attesa più lunga per la risposta del server locale (su Windows una porta chiusa risponde dopo ~2 s).

## Novità della 1.0.1

### Funzioni
- **Sprite inclusi** direttamente nella versione principale (resta possibile importarne altri dalle impostazioni).
- **Voce locale Kokoro-82M**: installazione con un clic (Impostazioni → Voce e audio), voci italiane Sara e Nicola, offline dopo il primo download. Il server parte e si chiude insieme ad Alter Ego.
- **Mic-Link**: microfono virtuale **AlterEgo Link Audio** con il tuo microfono + la voce di Alter Ego, per Discord, OBS e giochi (richiede il driver gratuito VB-CABLE; rinomina automatica del dispositivo).
- **Scelta dei dispositivi audio**: uscita per voce e suoni, microfono per il push-to-talk.
- **Dialogo automatico**: se non clicchi, il testo va avanti da solo (aspetta anche la fine della voce). Regolabile in Generale.
- **Animazione da vecchia TV** all'apertura e alla chiusura della finestra.
- **Non chiedere conferma**: opzione (spenta di default) per eseguire subito le azioni che chiedevano l'approvazione manuale.
- **DeepSeek più economico**: prompt e cronologia costruiti per sfruttare la cache del prefisso; statistiche della cache in AI e chiavi.

### Modifiche
- Tutto l'audio (voce, blip, jingle) ora passa dal backend: esce dal dispositivo scelto e può andare in Mic-Link.
- Voci di Windows tramite le API di sistema (WinRT), con scelta automatica della voce per la lingua.

### File
- `AlterEgo_1.1.0_setup.exe`: installer (aggiorna le versioni precedenti mantenendo impostazioni, chiavi e memoria).
- `AlterEgo_1.1.0_portable.zip`: estrai e avvia `alter-ego.exe`.
- `SHA256SUMS.txt`: impronte dei file.

Build non firmata: SmartScreen può chiedere *Ulteriori informazioni → Esegui comunque*.
