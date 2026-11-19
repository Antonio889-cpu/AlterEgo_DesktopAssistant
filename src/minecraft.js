// Impostazioni → Giochi → Minecraft: stato, entra/esci, installazione (Node + Mineflayer), skin con anteprima.
// Backend: src-tauri/src/mclink (comandi minecraft_*).
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const el = (id) => document.getElementById(id);
  const tt = (s, v) => (typeof t === "function" ? t(s, v) : s);
  let mc = null; // copia di config.minecraft
  let saveTimer = null;

  const CHECKS = { "mc-follow": "follow", "mc-gather": "gather", "mc-build": "build", "mc-explore": "explore", "mc-fly": "fly", "mc-learn": "learn", "mc-csl": "csl", "mc-slim": "slim" };
  const TEXTS = { "mc-host": "host", "mc-username": "username", "mc-player": "player", "mc-version": "version", "mc-skin-cmd": "skin_cmd" };

  function fill(m) {
    if (!m) return;
    mc = structuredClone(m);
    for (const [id, k] of Object.entries(CHECKS)) el(id).checked = !!mc[k];
    for (const [id, k] of Object.entries(TEXTS)) if (document.activeElement !== el(id)) el(id).value = mc[k] ?? "";
    if (document.activeElement !== el("mc-port")) el("mc-port").value = mc.port ?? 0;
    el("mc-initiative").value = mc.initiative ?? 60;
    el("mc-initiative-val").textContent = el("mc-initiative").value;
    el("mc-skin").value = mc.skin === "file" ? "file" : "alterego";
  }

  function save() {
    if (!mc) return;
    for (const [id, k] of Object.entries(CHECKS)) mc[k] = el(id).checked;
    for (const [id, k] of Object.entries(TEXTS)) mc[k] = el(id).value.trim();
    mc.port = Math.max(0, Math.min(65535, Number(el("mc-port").value) || 0));
    mc.initiative = Number(el("mc-initiative").value);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => invoke("minecraft_settings", { settings: mc }).catch(() => {}), 300);
  }

  // ---------- anteprima della skin (vista di fronte) ----------
  function draw(src) {
    const img = new Image();
    img.onload = () => {
      const c = el("mc-skin-view");
      const g = c.getContext("2d");
      const s = c.width / 16;
      g.imageSmoothingEnabled = false;
      g.clearRect(0, 0, c.width, c.height);
      const tall = img.height >= 64;
      const part = (sx, sy, w, h, dx, dy, mirror) => {
        g.save();
        if (mirror) { g.translate((dx + w) * s, dy * s); g.scale(-1, 1); g.drawImage(img, sx, sy, w, h, 0, 0, w * s, h * s); }
        else g.drawImage(img, sx, sy, w, h, dx * s, dy * s, w * s, h * s);
        g.restore();
      };
      part(8, 8, 8, 8, 4, 0); part(40, 8, 8, 8, 4, 0); // testa + cappello
      part(20, 20, 8, 12, 4, 8); // corpo
      part(44, 20, 4, 12, 0, 8); // braccio destro
      part(4, 20, 4, 12, 4, 20); // gamba destra
      if (tall) {
        part(36, 52, 4, 12, 12, 8); part(20, 52, 4, 12, 8, 20);
        part(20, 36, 8, 12, 4, 8); part(44, 36, 4, 12, 0, 8); part(52, 52, 4, 12, 12, 8); part(4, 36, 4, 12, 4, 20); part(4, 52, 4, 12, 8, 20);
      } else {
        part(44, 20, 4, 12, 12, 8, true); part(4, 20, 4, 12, 8, 20, true); // vecchio formato: lato sinistro specchiato
      }
    };
    img.src = src;
  }
  async function preview() {
    let b64 = "";
    try { b64 = (await invoke("minecraft_skin_get")) || ""; } catch {}
    draw(typeof b64 === "string" && b64.length > 100 ? "data:image/png;base64," + b64 : "assets/minecraft/alterego_skin.png");
  }

  el("mc-skin").onchange = async (e) => {
    const msg = el("mc-skin-msg");
    msg.textContent = "";
    if (e.target.value === "file") { el("mc-skin-file").click(); return; }
    await invoke("minecraft_skin_set", { data: "" }).catch(() => {});
    if (mc) mc.skin = "alterego";
    preview();
  };
  el("mc-skin-file").onchange = (e) => {
    const f = e.target.files && e.target.files[0];
    const msg = el("mc-skin-msg");
    e.target.value = "";
    if (!f) { el("mc-skin").value = mc?.skin === "file" ? "file" : "alterego"; return; }
    const r = new FileReader();
    r.onload = async () => {
      try {
        const size = await invoke("minecraft_skin_set", { data: String(r.result) });
        msg.textContent = tt("Skin {s} ✓", { s: size }); msg.className = "result ok";
        if (mc) mc.skin = "file";
        el("mc-skin").value = "file";
      } catch (err) {
        msg.textContent = String(err); msg.className = "result bad";
        el("mc-skin").value = mc?.skin === "file" ? "file" : "alterego";
      }
      preview();
    };
    r.readAsDataURL(f);
  };

  // ---------- stato ----------
  const KICK = {
    online_mode: "Il mondo chiede un account Minecraft ufficiale (online-mode), e Alter Ego usa un account offline. Soluzioni: una mod per il LAN che spegne online-mode (per esempio «LAN World Plug-n-Play» o «Custom LAN»), oppure un server Paper sul PC con online-mode=false in server.properties.",
    whitelist: "Il server ha una whitelist: aggiungi l'account con /whitelist add {name}.",
    version: "Versione non compatibile: scrivi la versione del mondo nel campo «Versione» (per esempio 1.21.4).",
    banned: "L'account è bannato su questo server.",
    duplicate: "Nel mondo c'è già un giocatore con questo nome: cambia il nome dell'account.",
    full: "Il server è pieno.",
    refused: "Nessun mondo a questo indirizzo: apri il mondo in LAN oppure controlla indirizzo e porta.",
    no_lan: "Nessun mondo in LAN trovato: in Minecraft premi Esc → Apri in LAN, oppure scrivi la porta.",
    other: "Disconnessione dal server: {reason}",
  };
  async function status() {
    let s;
    try { s = await invoke("minecraft_status"); } catch { return; }
    if (!s || typeof s !== "object") return;
    const st = el("mc-state"), where = el("mc-where"), dot = el("mc-dot");
    const node = s.node;
    el("mc-node").textContent = s.installing ? tt("Installazione in corso…")
      : node ? tt("Node.js {v}{m}", { v: node.version, m: node.managed ? " (" + tt("portatile") + ")" : "" }) + (s.installed ? " · Mineflayer ✓" : " · " + tt("Mineflayer da installare")) : tt("Node.js non trovato");
    el("mc-node").className = s.installed ? "result ok" : "result";
    el("mc-install").disabled = !!s.installing;
    if (s.in_game) {
      st.textContent = tt("Nel mondo");
      const w = s.world || {};
      where.textContent = "· " + tt("Minecraft {v} su {h}:{p}", { v: w.version || "?", h: w.host || "?", p: w.port || "?" }) + (s.players?.length ? " · " + s.players.join(", ") : "");
      dot.style.background = "var(--accent)";
    } else if (s.running) {
      st.textContent = tt("Collegamento…"); where.textContent = ""; dot.style.background = "var(--warn)";
    } else {
      st.textContent = s.installed ? tt("Non collegato") : tt("Da installare");
      where.textContent = tt("· Java Edition, mondo aperto in LAN");
      dot.style.background = "";
    }
    el("mc-join").disabled = !s.installed || s.in_game || s.running;
    el("mc-leave").disabled = !s.running && !s.in_game;
    const k = el("mc-kick");
    if (s.kick && !s.in_game) {
      const [code, reason] = s.kick;
      k.textContent = tt(KICK[code] || KICK.other, { reason: reason || "", name: mc?.username || "AlterEgo" });
      k.hidden = false;
    } else if (s.error && !s.in_game && !s.running) {
      k.textContent = tt(KICK.other, { reason: s.error }); k.hidden = false;
    } else k.hidden = true;
    el("mc-learned").textContent = s.skills || s.lessons ? tt("{s} abilità e {l} lezioni imparate", { s: s.skills || 0, l: s.lessons || 0 }) : "";
    const sk = el("mc-csl-msg");
    if (s.skin) { sk.textContent = (s.skin[0] ? "SkinsRestorer ✓ " : "SkinsRestorer: ") + s.skin[1]; sk.className = s.skin[0] ? "result ok" : "result bad"; }
    else if (!sk.dataset.sticky) { sk.textContent = s.csl ? tt("CustomSkinLoader trovato ✓") : tt("CustomSkinLoader non trovato"); sk.className = s.csl ? "result ok" : "result"; }
  }

  // ---------- pulsanti ----------
  el("mc-join").onclick = async () => {
    const k = el("mc-kick");
    try { await invoke("minecraft_join", { join: true }); k.hidden = true; }
    catch (e) { k.textContent = String(e); k.hidden = false; }
    setTimeout(status, 1500);
  };
  el("mc-leave").onclick = async () => { await invoke("minecraft_join", { join: false }).catch(() => {}); setTimeout(status, 2000); };
  el("mc-install").onclick = async () => {
    const p = el("mc-progress");
    p.textContent = tt("Preparo l'installazione…"); p.className = "result";
    try { const v = await invoke("minecraft_install"); p.textContent = tt("Pronto: Node.js {v} e Mineflayer installati.", { v }); p.className = "result ok"; }
    catch (e) { p.textContent = String(e); p.className = "result bad"; }
    status();
  };
  el("mc-scan").onclick = async () => {
    const out = el("mc-lan");
    out.textContent = tt("Cerco mondi in LAN…");
    try {
      const list = (await invoke("minecraft_scan_lan")) || [];
      if (!Array.isArray(list) || !list.length) { out.textContent = tt("Nessun mondo in LAN trovato: in Minecraft premi Esc → Apri in LAN, oppure scrivi la porta."); return; }
      const w = list[0];
      out.textContent = tt("Trovato «{m}» su {h}:{p}", { m: w.motd, h: w.host, p: w.port });
      el("mc-port").value = 0; save();
    } catch (e) { out.textContent = String(e); }
  };
  el("mc-forget").onclick = async () => {
    if (!confirm(tt("Dimenticare abilità e lezioni imparate in Minecraft?"))) return;
    await invoke("minecraft_forget").catch(() => {});
    status();
  };
  el("mc-csl-now").onclick = async () => {
    const m = el("mc-csl-msg");
    m.dataset.sticky = "1";
    try { const p = await invoke("minecraft_skin_csl"); m.textContent = tt("Copiata in {p}", { p }); m.className = "result ok"; }
    catch (e) { m.textContent = String(e); m.className = "result bad"; }
  };
  el("mc-skin-export").onclick = async () => {
    const m = el("mc-csl-msg");
    m.dataset.sticky = "1";
    try { const p = await invoke("minecraft_skin_export"); m.textContent = tt("Salvata in {p}", { p }); m.className = "result ok"; }
    catch (e) { m.textContent = String(e); m.className = "result bad"; }
  };
  el("mc-sr-link").onclick = (e) => { e.preventDefault(); invoke("open_link_cmd", { which: "skinsrestorer_upload" }); };
  for (const id of [...Object.keys(CHECKS), ...Object.keys(TEXTS), "mc-port", "mc-initiative"]) el(id).onchange = save;
  el("mc-initiative").oninput = () => { el("mc-initiative-val").textContent = el("mc-initiative").value; };

  listen("minecraft-install", (e) => {
    const { phase, text } = e.payload || {};
    const p = el("mc-progress");
    p.textContent = text || "";
    p.className = phase === "error" ? "result bad" : phase === "done" ? "result ok" : "result";
    if (phase !== "install") status();
  });
  listen("minecraft-link", () => status());
  listen("config", (e) => { if (e.payload && e.payload.minecraft) fill(e.payload.minecraft); });

  invoke("get_config").then((c) => { fill(c && c.minecraft); preview(); }).catch(() => preview());
  status();
  setInterval(status, 4000);
})();
