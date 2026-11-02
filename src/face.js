// Volto di Alter Ego. Fase 2: immagine provvisoria.
// Fase 3: macchina a stati con gli sprite (idle, blink, talk_open/closed, emozioni…).

const Face = (() => {
  let state = "idle";
  let talking = false;
  return {
    set(next) { state = next; },
    get: () => state,
    setTalking(on) { talking = on; },
    isTalking: () => talking,
  };
})();
