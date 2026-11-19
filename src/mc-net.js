// Impostazioni → Giochi → Minecraft: ponte ViaProxy (ogni versione), accesso ai server (AuthMe),
// spostamenti tra lobby/warp/sottoserver e apertura della console. Backend: mclink/{via,servers,console}.rs.
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const el = (id) => document.getElementById(id);
  const tt = (s, v) => (typeof t === "function" ? t(s, v) : s);
  if (!el("mc-srv-addr")) return;

  // campi del profilo del server → chiavi di ServerProfile
  const TEXT = { "mc-srv-register": "register_cmd", "mc-srv-login": "login_cmd", "mc-srv-party": "party_cmd", "mc-srv-tpa": "tpa_accept_cmd", "mc-srv-tpato": "tpa_to_cmd", "mc-srv-server": "server_cmd", "mc-srv-warp": "warp_cmd", "mc-srv-hub": "hub_cmd", "mc-srv-ask": "ask_cmd" };
  const CHECK = { "mc-srv-auth": "auth", "mc-srv-registered": "registered", "mc-srv-follow": "follow", "mc-srv-invites": "accept_invites" };
  let profile = null;
  let lan = false;
  let timer = null;

  function msg(text, cls) {
    const m = el("mc-srv-pass-msg");
    m.textContent = text || "";
    m.className = "result" + (cls ? " " + cls : "");
  }

  async function load() {
    let info;
    try { info = await invoke("minecraft_server_get"); } catch { return; }
    if (!info || typeof info !== "object" || !info.profile) return;
    profile = info.profile;
    lan = info.addr === "lan";
    el("mc-srv-addr").textContent = lan ? tt("mondo in LAN (niente login)") : info.addr;
    for (const [id, k] of Object.entries(TEXT)) if (document.activeElement !== el(id)) el(id).value = profile[k] ?? "";
    for (const [id, k] of Object.entries(CHECK)) el(id).checked = !!profile[k];
    if (document.activeElement !== el("mc-srv-rate")) el("mc-srv-rate").value = profile.max_per_min ?? 6;
    for (const id of ["mc-srv-pass", "mc-srv-pass-save", "mc-srv-pass-gen", "mc-srv-pass-clear"]) el(id).disabled = lan;
    if (!el("mc-srv-pass-msg").dataset.sticky) msg(lan ? "" : info.has_password ? tt("Password salvata nel portachiavi ✓") : tt("Nessuna password salvata: la genero alla prima registrazione"), info.has_password ? "ok" : "");
  }

  function save() {
    if (!profile) return;
    for (const [id, k] of Object.entries(TEXT)) profile[k] = el(id).value.trim();
    for (const [id, k] of Object.entries(CHECK)) profile[k] = el(id).checked;
    profile.max_per_min = Math.max(1, Math.min(30, Number(el("mc-srv-rate").value) || 6));
    clearTimeout(timer);
    timer = setTimeout(() => invoke("minecraft_server_set", { profile }).catch(() => {}), 300);
  }
  for (const id of [...Object.keys(TEXT), ...Object.keys(CHECK), "mc-srv-rate"]) el(id).addEventListener("change", save);

  async function setPass(password) {
    el("mc-srv-pass-msg").dataset.sticky = "1";
    try {
      await invoke("minecraft_password_set", { password });
      el("mc-srv-pass").value = "";
      msg(password ? tt("Password salvata nel portachiavi ✓") : tt("Password nuova generata e salvata nel portachiavi ✓"), "ok");
    } catch (e) { msg(String(e), "bad"); }
  }
  el("mc-srv-pass-save").onclick = () => {
    const p = el("mc-srv-pass").value.trim();
    if (!p) { msg(tt("Scrivi una password, oppure premi «Genera»"), "bad"); return; }
    setPass(p);
  };
  el("mc-srv-pass-gen").onclick = () => setPass("");
  el("mc-srv-pass-clear").onclick = async () => {
    el("mc-srv-pass-msg").dataset.sticky = "1";
    try { await invoke("minecraft_password_clear"); msg(tt("Password dimenticata")); } catch (e) { msg(String(e), "bad"); }
  };
  el("mc-srv-pass").addEventListener("keydown", (e) => { if (e.key === "Enter") el("mc-srv-pass-save").click(); });

  // ---------- ponte ViaProxy ----------
  async function via() {
    let s;
    try { s = await invoke("minecraft_via_status"); } catch { return; }
    if (!s || typeof s !== "object") return;
    const st = el("mc-via-state");
    const java = s.java ? tt("Java {v}{m}", { v: s.java.major, m: s.java.managed ? " (" + tt("portatile") + ")" : "" }) : tt("Java non trovato");
    const jar = s.jar ? "ViaProxy " + (s.version || "") + " ✓" : tt("ViaProxy da installare");
    st.textContent = s.installing ? tt("Installazione in corso…") : java + " · " + jar + (s.active ? " · " + tt("ponte acceso verso {t}", { t: s.active.target }) : "");
    st.className = s.java && s.jar ? "result ok" : "result";
    el("mc-via-install").disabled = !!s.installing;
  }
  el("mc-via-install").onclick = async () => {
    const p = el("mc-via-progress");
    p.textContent = tt("Preparo l'installazione…"); p.className = "result";
    try { p.textContent = await invoke("minecraft_via_install"); p.className = "result ok"; }
    catch (e) { p.textContent = String(e); p.className = "result bad"; }
    via();
  };
  listen("minecraft-via", (e) => {
    const { phase, text } = e.payload || {};
    const p = el("mc-via-progress");
    p.textContent = text || "";
    p.className = phase === "error" ? "result bad" : phase === "done" ? "result ok" : "result";
    if (phase !== "install") via();
  });

  el("mc-console-open").onclick = () => invoke("minecraft_console_open").catch(() => {});
  // indirizzo o nome cambiati: altro profilo
  listen("config", () => load());
  load();
  via();
  setInterval(via, 6000);
})();
