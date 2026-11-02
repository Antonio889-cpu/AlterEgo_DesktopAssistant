// Mappa stati → sprite. File in assets/sprites/ (generati da tools/process_sprites.py).
// Gli stati senza sprite proprio usano quello più vicino: vedi docs/MANCANTI.md.
window.SPRITES = {
  files: {
    idle: "idle.png",
    happy: "happy.png",
    embarrassed: "embarrassed.png",
    worried: "worried.png",
    thinking: "thinking.png",
    sad: "sad.png",
    surprised: "surprised.png",
    error: "error.png",
  },
  alias: {
    listening: "idle",
    talk_open: "idle",
    sleep: "sad",
    // assenti: nessuna animazione finché non esiste lo sprite
    blink: null,
    talk_closed: null,
  },
};
