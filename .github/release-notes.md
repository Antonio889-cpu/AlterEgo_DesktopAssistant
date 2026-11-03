## Alter Ego 1.0.3

⚠ **Contiene gli sprite originali di Alter Ego (© Spike Chunsoft): solo per uso personale, non distribuire.**

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
- `AlterEgo_1.0.3_setup.exe`: installer (aggiorna le versioni precedenti mantenendo impostazioni, chiavi e memoria).
- `AlterEgo_1.0.3_portable.zip`: estrai e avvia `alter-ego.exe`.
- `SHA256SUMS.txt`: impronte dei file.

Build non firmata: SmartScreen può chiedere *Ulteriori informazioni → Esegui comunque*.
