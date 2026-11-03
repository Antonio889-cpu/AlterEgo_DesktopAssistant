// Suoni del dialogo e voce. L'audio lo produce il backend (src-tauri/src/audio/):
// così esce dal dispositivo scelto nelle impostazioni e, con Mic-Link, anche nel
// microfono virtuale "AlterEgo Link Audio".
// - blip "dr" (stile Danganronpa, ricreato da zero) o "soft", sintetizzati
// - voce: Windows, Kokoro-82M locale o endpoint compatibile OpenAI

const Audio2 = (() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  let charCount = 0;
  let speaking = false;
  const listeners = [];

  const cfg = () => window.appConfig?.audio || { blips: "dr", blip_volume: 0.35, tts: "off" };
  function emitSpeaking(on) {
    if (speaking === on) return;
    speaking = on;
    listeners.forEach((fn) => fn(on));
  }
  listen("voice", (e) => emitSpeaking(!!e.payload));

  /** Un blip ogni 2 caratteri, così non diventa un ronzio. */
  function blip(ch) {
    const c = cfg();
    if (c.blips === "off" || c.blip_volume <= 0) return;
    if (speaking && c.mute_blips_with_tts) return;
    if (++charCount % 2) return;
    invoke("audio_sfx", { kind: "blip", seed: ch?.codePointAt(0) || 0 }).catch(() => {});
  }

  /** Breve jingle (clip salvata, promemoria). */
  function chime() {
    invoke("audio_sfx", { kind: "chime", seed: 0 }).catch(() => {});
  }

  function stop() {
    invoke("audio_stop").catch(() => {});
    emitSpeaking(false);
  }

  /** Legge il testo con la voce scelta. Non blocca: il typewriter va in parallelo. */
  function speak(text, lang = "it") {
    if (!text || cfg().tts === "off") return;
    invoke("tts_say", { text, lang }).catch((e) => {
      emitSpeaking(false);
      Chat.notify(t("Voce non disponibile: {err}", { err: e }));
    });
  }

  return { blip, chime, speak, stop, onSpeaking: (fn) => listeners.push(fn), isSpeaking: () => speaking };
})();
