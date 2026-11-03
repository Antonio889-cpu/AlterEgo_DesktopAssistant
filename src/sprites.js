// Sprite inclusi nella build (src/assets/sprites/, repo privata) e
// alias per gli stati senza sprite proprio (vedi docs/MANCANTI.md).
// Gli sprite importati dall'utente (%APPDATA%\AlterEgo\sprites) hanno la precedenza: vedi face.js.
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
