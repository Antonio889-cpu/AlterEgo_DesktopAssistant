// Macchina a stati di Alter Ego: modalità (idle, listening, thinking, speaking, sleep)
// + emozione dell'ultima risposta. Pilota Face.

const State = (() => {
  const EMOTIONS = ["idle", "happy", "surprised", "embarrassed", "sad", "worried", "thinking", "error"];
  const SLEEP_AFTER_MS = 10 * 60 * 1000;
  const EMOTION_HOLD_MS = 8000;

  let mode = "idle";
  let emotion = "idle";
  let sleepTimer = null;
  let holdTimer = null;

  function render() {
    const face = mode === "speaking" || mode === "idle" ? emotion : mode;
    Face.set(face);
    document.body.dataset.mode = mode;
  }

  function touch() {
    clearTimeout(sleepTimer);
    if (mode === "sleep") { mode = "idle"; render(); }
    sleepTimer = setTimeout(() => { if (mode === "idle") { mode = "sleep"; emotion = "idle"; render(); } }, SLEEP_AFTER_MS);
  }

  return {
    EMOTIONS,
    /** Cambia modalità. */
    setMode(next) {
      clearTimeout(holdTimer);
      mode = next;
      touch();
      render();
    },
    /** Emozione dalla risposta AI ([happy] ecc.). Valori sconosciuti → idle. */
    setEmotion(e) {
      emotion = EMOTIONS.includes(e) ? e : "idle";
      render();
    },
    /** Dopo la risposta: torna idle, l'emozione resta per qualche secondo.
     *  Non interrompe ascolto/pensiero in corso (a meno di force). */
    settle(force = false) {
      if (!force && (mode === "listening" || mode === "thinking")) return;
      mode = "idle";
      render();
      clearTimeout(holdTimer);
      holdTimer = setTimeout(() => { if (mode === "idle") { emotion = "idle"; render(); } }, EMOTION_HOLD_MS);
    },
    touch,
    mode: () => mode,
  };
})();
