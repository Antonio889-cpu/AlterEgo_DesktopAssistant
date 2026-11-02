# Sprite mancanti

Il foglio fornito ha 8 espressioni. Mappatura usata (`src/sprites.js`):

| Stato | Sprite usato | Pannello del foglio |
|---|---|---|
| `idle` | idle | 1 – neutra, bocca socchiusa |
| `happy` | happy | 2 – occhi chiusi, sorriso |
| `embarrassed` | embarrassed | 3 – rossore, mano alla bocca |
| `worried` | worried | 4 – neutra con goccia di sudore |
| `thinking` | thinking | 5 – mano al mento, sguardo di lato |
| `sad` | sad | 6 – occhi chiusi, broncio |
| `surprised` | surprised | 7 – occhi sgranati, bocca aperta |
| `error` | error | 8 – occhi sgranati, bocca aperta, sudore |

## Da decidere / ridisegnare

| Stato | Ora usa | Effetto |
|---|---|---|
| `blink` | — (disattivato) | Niente battito di ciglia. Basta aggiungere `blink.png` (occhi chiusi, espressione neutra) e la voce in `sprites.js`. |
| `talk_closed` | — | Senza bocca chiusa non c'è l'animazione bocca aperta/chiusa: mentre parla il volto fa un leggero sobbalzo. |
| `talk_open` | idle | idle ha già la bocca socchiusa. |
| `listening` | idle | |
| `sleep` | sad | sad ha gli occhi chiusi; in più il volto si attenua e "respira". |

Per aggiungere uno sprite: salvalo in `assets/sprites/raw/`, estendi `tools/process_sprites.py`
(o mettilo direttamente in `src/assets/sprites/` con sfondo trasparente e stesso formato 612×483),
poi aggiorna `src/sprites.js`.
