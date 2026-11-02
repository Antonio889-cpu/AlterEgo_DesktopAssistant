**Alter Ego – Desktop Assistant** per Windows 10/11. Compilata automaticamente da GitHub Actions su Windows.

## Download

| File | Cosa |
|---|---|
| `AlterEgo_<versione>_setup.exe` | Installer (consigliato) |
| `AlterEgo_<versione>_portable.zip` | Versione portable: estrai e avvia `alter-ego.exe` |
| `SHA256SUMS.txt` | Checksum per verificare i file |

L'app non è firmata: Windows SmartScreen può avvisare (*Ulteriori informazioni → Esegui comunque*).
Dopo l'avvio l'icona compare nella **system tray**.

## Novità

- Finestra in stile Alter Ego (CRT, scanline, glow, box dialogo pixel), sempre in primo piano, click-through opzionale.
- 5 servizi AI (Gemini, OpenRouter, Groq, DeepSeek, Ollama) con fallback e **più chiavi API per servizio** a rotazione.
- Push-to-talk con Whisper, anche con i **tasti laterali del mouse**.
- **Clip vocali** ("Alter Ego clippa") per AMD Adrenalin, NVIDIA App e Medal, senza passare dall'AI.
- Azioni sul PC con permessi (tutto spento di default), conferme e registro.
- Memoria locale con riassunti e fatti, personalità regolabile, risposte IT / IT+EN / EN.
- Voce (voci di Windows o servizio compatibile OpenAI) e blip del dialogo.

## Importante: sprite

Gli sprite originali di Alter Ego sono © Spike Chunsoft e **non sono inclusi**. Senza sprite l'app usa un volto segnaposto;
puoi importare il tuo foglio sprite da *Impostazioni → Personalità → Sprite*.

## Da sapere

Sviluppata e testata su Linux, compilazione per Windows inclusa. Su Windows reali vanno ancora verificati:
tasti simulati per le clip, push-to-talk col mouse, notifiche, avvio con Windows, screenshot e consumi.
Segnala i problemi nelle Issues.

Guida completa nel [README](https://github.com/Antonio889-cpu/AlterEgo_DesktopAssistant#readme).
