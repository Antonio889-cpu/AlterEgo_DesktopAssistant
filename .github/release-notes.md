## Alter Ego 1.0.1

⚠ **Contiene gli sprite originali di Alter Ego (© Spike Chunsoft): solo per uso personale, non distribuire.**

### Novità
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
- `AlterEgo_1.0.1_setup.exe`: installer (aggiorna la 1.0.0 mantenendo impostazioni, chiavi e memoria).
- `AlterEgo_1.0.1_portable.zip`: estrai e avvia `alter-ego.exe`.
- `SHA256SUMS.txt`: impronte dei file.

Build non firmata: SmartScreen può chiedere *Ulteriori informazioni → Esegui comunque*.
