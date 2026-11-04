# Origine degli asset

| File | Origine | Licenza / note |
|---|---|---|
| `assets/sprites/raw/alter_ego_sheet.png` | Foglio 3×3 fornito dall'utente (2 novembre 2026), ricavato da sprite di The Models Resource, screenshot già tinti di verde. Il 9° pannello (Mondo Owada) non è Alter Ego ed è ignorato. | © Spike Chunsoft (Danganronpa). Incluso nella repo dalla 1.0.1. |
| `src/assets/sprites/*.png` | Generati da `tools/process_sprites.py` a partire dal foglio sopra. | Come sopra: inclusi nella repo e nell'app. |
| `%APPDATA%\AlterEgo\sprites\*.png` | Generati dall'app con *Importa foglio sprite* (stessa elaborazione, in `src-tauri/src/sprites.rs`). | Dipende dal foglio importato. Restano sul PC dell'utente. |
| Suoni dei blip e jingle | Sintetizzati nel codice (`src-tauri/src/audio/synth.rs`). | Nessun vincolo. |
| `docs/screenshots/*.png` | Catture dell'interfaccia (con gli sprite originali). | Contengono gli sprite. |
| Kokoro-82M (modello e voci), libreria `kokoro` | Scaricati da Hugging Face / PyPI durante l'installazione di Kokoro (`tools/kokoro/setup.ps1`), non nella repo. | Apache-2.0 (hexgrad). |
| VB-CABLE | Scaricato dall'utente dal sito di VB-Audio, non nella repo. | Donationware di VB-Audio Software. |
| `src/assets/fonts/DotGothic16-Regular.ttf` | github.com/google/fonts (`ofl/dotgothic16`) | SIL Open Font License 1.1 (`OFL.txt`). |
| Logo laptop, sfondo a circuiti, icona app | Disegnati per questo progetto (SVG/script). | Nessun vincolo. |

Gli sprite vengono tutti dal foglio fornito (origine: The Models Resource).

Gli sprite di Alter Ego sono © Spike Chunsoft e non rientrano nella licenza MIT del codice.
