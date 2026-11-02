// Suoni del dialogo (blip sintetizzati, nessun file audio esterno) e voce TTS.
// - "dr":   blip acuto e corto, ispirato al suono dei dialoghi di Danganronpa (ricreato da zero)
// - "soft": blip originale più morbido
// TTS: voci di Windows (Web Speech API di WebView2) oppure endpoint /audio/speech dal backend.

const Audio2 = (() => {
  const { invoke } = window.__TAURI__.core;
  let ctx = null;
  let charCount = 0;
  let speaking = false;
  let current = null; // <audio> remoto in riproduzione
  const listeners = [];

  const cfg = () => window.appConfig?.audio || { blips: "dr", blip_volume: 0.35, tts: "off" };
  function emitSpeaking(on) { speaking = on; listeners.forEach((fn) => fn(on)); }

  function ac() {
    if (!ctx) ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (ctx.state === "suspended") ctx.resume();
    return ctx;
  }

  /** Un blip ogni 2 caratteri, così non diventa un ronzio. */
  function blip(ch) {
    const c = cfg();
    if (c.blips === "off" || c.blip_volume <= 0) return;
    if (speaking && c.mute_blips_with_tts) return;
    if (++charCount % 2) return;
    const a = ac();
    const t = a.currentTime;
    const osc = a.createOscillator();
    const gain = a.createGain();
    if (c.blips === "dr") {
      osc.type = "square";
      // piccola variazione di tono per carattere: suona meno meccanico
      osc.frequency.setValueAtTime(880 + ((ch?.codePointAt(0) || 0) % 5) * 40, t);
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.18 * c.blip_volume, t + 0.004);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.045);
      osc.connect(gain).connect(a.destination);
      osc.start(t);
      osc.stop(t + 0.05);
    } else {
      osc.type = "triangle";
      osc.frequency.setValueAtTime(520, t);
      osc.frequency.exponentialRampToValueAtTime(380, t + 0.06);
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.35 * c.blip_volume, t + 0.006);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.07);
      osc.connect(gain).connect(a.destination);
      osc.start(t);
      osc.stop(t + 0.08);
    }
  }

  /** Breve jingle (clip salvata, promemoria). */
  function chime() {
    const c = cfg();
    if (c.blips === "off") return;
    const a = ac();
    [660, 990].forEach((f, i) => {
      const t = a.currentTime + i * 0.09;
      const o = a.createOscillator();
      const g = a.createGain();
      o.type = "sine";
      o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, t);
      g.gain.exponentialRampToValueAtTime(0.25 * Math.max(c.blip_volume, 0.2), t + 0.01);
      g.gain.exponentialRampToValueAtTime(0.0001, t + 0.18);
      o.connect(g).connect(a.destination);
      o.start(t);
      o.stop(t + 0.2);
    });
  }

  function voices() {
    return (window.speechSynthesis?.getVoices() || []).map((v) => ({ name: v.name, lang: v.lang }));
  }

  function pickVoice(name, lang) {
    const all = window.speechSynthesis.getVoices();
    return all.find((v) => v.name === name) || all.find((v) => v.lang?.toLowerCase().startsWith(lang)) || null;
  }

  function stop() {
    window.speechSynthesis?.cancel();
    if (current) { current.pause(); current = null; }
    if (speaking) emitSpeaking(false);
  }

  /** Legge il testo con la voce scelta. Non blocca: il typewriter va in parallelo. */
  async function speak(text, lang = "it") {
    const c = cfg();
    stop();
    if (!text || c.tts === "off") return;
    if (c.tts === "windows") {
      if (!window.speechSynthesis) return;
      const u = new SpeechSynthesisUtterance(text);
      u.voice = pickVoice(c.tts_voice, lang);
      u.lang = u.voice?.lang || (lang === "en" ? "en-US" : "it-IT");
      u.rate = c.tts_rate;
      u.volume = c.tts_volume;
      u.onstart = () => emitSpeaking(true);
      u.onend = u.onerror = () => emitSpeaking(false);
      window.speechSynthesis.speak(u);
    } else if (c.tts === "openai") {
      try {
        const b64 = await invoke("tts_speak", { text });
        const el = new window.Audio("data:audio/mpeg;base64," + b64);
        el.volume = c.tts_volume;
        current = el;
        el.onplay = () => emitSpeaking(true);
        el.onended = el.onerror = () => { if (current === el) current = null; emitSpeaking(false); };
        await el.play();
      } catch (e) {
        Chat.notify("Voce non disponibile: " + e);
      }
    }
  }

  // le voci di Windows arrivano in modo asincrono
  window.speechSynthesis?.addEventListener?.("voiceschanged", () => {});

  return { blip, chime, speak, stop, voices, onSpeaking: (fn) => listeners.push(fn), isSpeaking: () => speaking };
})();
