# Origine degli asset

| File | Origine | Licenza / note |
|---|---|---|
| `assets/sprites/raw/alter_ego_sheet.png` | Foglio 3×3 fornito dall'utente (2 novembre 2026), screenshot già tinti di verde. Il 9° pannello (Mondo Owada) non è Alter Ego ed è ignorato. | © Spike Chunsoft (Danganronpa). Solo uso personale. Escluso da git. |
| `src/assets/sprites/*.png` | Generati da `tools/process_sprites.py` a partire dal foglio sopra (solo build private). | Come sopra. Esclusi da git. |
| `%APPDATA%\AlterEgo\sprites\*.png` | Generati dall'app con *Importa foglio sprite* (stessa elaborazione, in `src-tauri/src/sprites.rs`). | Come sopra. Restano solo sul PC dell'utente. |
| Volto segnaposto (kaomoji), suoni dei blip | Creati per questo progetto (testo e sintesi audio). | Nessun vincolo. |
| `docs/screenshots/*.png` | Catture dell'interfaccia con il volto segnaposto (nessuno sprite originale). | Nessun vincolo. |
| `src/assets/fonts/DotGothic16-Regular.ttf` | github.com/google/fonts (`ofl/dotgothic16`) | SIL Open Font License 1.1 (`OFL.txt`). |
| Logo laptop, sfondo a circuiti, icona app | Disegnati per questo progetto (SVG/script). | Nessun vincolo. |

Nessun asset è stato scaricato da wiki o siti di terzi: gli sprite vengono tutti dal foglio fornito.

**Attenzione licenza:** gli sprite di Alter Ego sono materiale protetto da copyright.
Vanno bene per uso personale; non pubblicarli e non distribuire build che li contengono.
La release pubblica non contiene sprite: l'utente li importa dall'app.
