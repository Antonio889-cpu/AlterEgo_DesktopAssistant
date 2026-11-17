# Cervello di moscerino (flybrain.bin)

Costruisce `src-tauri/resources/flybrain.bin`, il circuito che dà ad Alter Ego i riflessi in Garry's Mod
(`src-tauri/src/gmodlink/fly.rs`, `src-tauri/gmod_addon/lua/alterego/fly.lua`).

## Dati (CC-BY 4.0, non nel repository)

Nella cartella `$FLY_DATA`:

- `conn.feather`: `proofread_connections_783.feather` da https://zenodo.org/records/10676866 (852 MB)
- `ann.tsv`: `supplemental_files/Supplemental_file1_neuron_annotations.tsv` da https://github.com/flyconnectome/flywire_annotations

```
pip install pyarrow pandas scipy
FLY_DATA=/percorso/dati python3 tools/flybrain/build.py
```

## Come funziona

- `connectome.py`: carica le connessioni (somma delle sinapsi per coppia di neuroni) e dà un segno a ognuna dal
  neurotrasmettitore previsto: acetilcolina eccita, GABA e glutammato inibiscono (come Shiu et al. 2024);
  dopamina, serotonina e octopamina sono modulatori lenti e non hanno effetto rapido. Le cellule di Kenyon sono
  colinergiche (la previsione dalle immagini spesso dice dopamina). Senza queste correzioni, e senza togliere le sinapsi
  asso-assoniche tra cellule di Kenyon e tra neuroni sensoriali, il modello LIF va in "crisi" (migliaia di neuroni
  del corpo a fungo sempre accesi).
- `sim.py`: modello leaky integrate-and-fire con i parametri di Shiu et al. (Nature 2024), passo 0,5 ms; i neuroni
  sensoriali ricevono spike di Poisson a 150 Hz × intensità.
- `build.py`: simula tutto il cervello (139.000 neuroni) con 40 combinazioni di stimoli e tiene i neuroni che sparano
  almeno una volta, più ingressi e uscite: ~9.500 neuroni e ~690.000 sinapsi (2,8 MB). Un neurone che non spara mai non
  cambia la dinamica degli altri.

## Sensi e uscite

| Senso | Neuroni |
|---|---|
| pain | termocettori del caldo (TRN_VP2) + setole meccanosensoriali della testa |
| loom_l / loom_r | LC4 + LPLC2 di ciascun lato (oggetti che si ingrandiscono) |
| sweet | recettori gustativi dello zucchero |
| touch | organo di Johnston (grooming) + setole delle antenne |
| smell_food / smell_danger | ORN DM1, DM2, DM4, VA2, DL5 / ORN V (CO₂) e DA2 (geosmina) |
| heat, cold, wet, water | TRN_VP2 / TRN_VP3, HRN_VP1l / HRN_VP5 / gusto dell'acqua |
| sound, light, wind | organo di Johnston uditivo / ocelli / organo di Johnston vento-gravità |

| Uscita | Neuroni | Nel gioco |
|---|---|---|
| escape | DNp01 (Giant Fiber), DNp02, DNp04, DNp06, DNp11 | salta e scappa |
| backward | MDN (moonwalker) | indietreggia |
| turn_l / turn_r | DNa02 | lato verso cui schiva |
| groom | DNg/DNge che rispondono alle setole | si spolvera |
| feed | motoneuroni e DN che rispondono al dolce (proboscide) | mangia l'anguria |
| drink, listen, squint, brace | DN che rispondono ad acqua, suono, luce, vento | annuisce, si gira, distoglie lo sguardo, alza le braccia |

Le uscite non legate a un tipo noto (groom, feed, drink…) sono i neuroni discendenti e motori che rispondono in modo più
specifico a quel senso nella simulazione del cervello intero.
