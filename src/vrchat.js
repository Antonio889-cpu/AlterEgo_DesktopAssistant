// Impostazioni → Integrazioni → VRChat: avviso (una volta), avatar dai file OSC, accesso facoltativo,
// sensi, comportamento, voce e chatbox, gesti, porte OSC, stato dal vivo.
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const $ = (id) => document.getElementById(id);
  if (!$("vrc-join")) return;
  const DEF = {
    accepted_risk: false, host: "127.0.0.1", send_port: 9000, recv_port: 9001, osc_dir: "", avatar: "", vision: "off", vision_model: "", look_secs: 8,
    hearing: true, hearing_device: "", chatbox: true, mic_mode: "open", follow: true, answer: true, gestures: true, explore: false, talk_others: false,
    chatter: "low", contacts: [], emote_param: "VRCEmote", emotes: "wave=1, clap=2, point=3, cheer=4, dance=5, backflip=6, sad=7, die=8",
    expr_param: "", expressions: "happy=1, sad=2, surprised=3, embarrassed=4, worried=5, thinking=6", fly: true, reflexes: true, login: false, account: "",
  };
  const SENSES = [["touch", "Tatto (carezze, boop)"], ["pain", "Dolore (colpi, strattoni)"], ["loom_l", "Vicinanza a sinistra"], ["loom_r", "Vicinanza a destra"], ["sweet", "Dolce (cibo)"]];
  let v = { ...DEF };
  let avatars = [];
  let methods = [];
  let timer = null;

  const checks = { "vrc-hearing": "hearing", "vrc-fly": "fly", "vrc-reflex": "reflexes", "vrc-follow": "follow", "vrc-answer": "answer", "vrc-gestures": "gestures",
    "vrc-explore": "explore", "vrc-others": "talk_others", "vrc-chatbox": "chatbox", "vrc-login": "login" };
  const fields = { "vrc-osc-dir": "osc_dir", "vrc-vision": "vision", "vrc-vmodel": "vision_model", "vrc-hear-dev": "hearing_device", "vrc-chatter": "chatter",
    "vrc-mic": "mic_mode", "vrc-emote-param": "emote_param", "vrc-emotes": "emotes", "vrc-expr-param": "expr_param", "vrc-exprs": "expressions" };

  function fill(c) {
    v = { ...DEF, ...(c || {}) };
    for (const [id, k] of Object.entries(checks)) $(id).checked = !!v[k];
    for (const [id, k] of Object.entries(fields)) {
      const el = $(id);
      if (el.tagName === "SELECT" && v[k] && ![...el.options].some((o) => o.value === v[k])) el.add(new Option(v[k], v[k]));
      el.value = v[k] ?? "";
    }
    $("vrc-look").value = String(v.look_secs);
    if (!$("vrc-look").value) $("vrc-look").value = "8";
    $("vrc-send").value = v.send_port;
    $("vrc-recv").value = v.recv_port;
    renderContacts();
    gate();
  }

  // avviso sui Termini: si mostra finché non viene accettato, poi mai più
  function gate() {
    $("vrc-warn").hidden = !!v.accepted_risk;
    $("vrc-join").disabled = !v.accepted_risk;
    $("vrc-login-box").hidden = !v.login;
    $("vrc-vmodel-row").hidden = v.vision !== "local";
    $("vrc-logout").disabled = !v.account;
    $("vrc-login-result").textContent = v.account ? t("Collegato come {n}", { n: v.account }) : "";
    for (const id of ["vrc-reflex"]) $(id).disabled = !v.fly;
    $("vrc-hear-dev").disabled = !v.hearing;
  }

  function collect() {
    for (const [id, k] of Object.entries(checks)) v[k] = $(id).checked;
    for (const [id, k] of Object.entries(fields)) v[k] = $(id).value.trim ? $(id).value.trim() : $(id).value;
    v.look_secs = Number($("vrc-look").value) || 8;
    v.send_port = Number($("vrc-send").value) || 9000;
    v.recv_port = Number($("vrc-recv").value) || 9001;
    v.contacts = [...$("vrc-contacts").querySelectorAll(".vrc-contact")]
      .map((r) => ({ param: r.querySelector("input").value.trim(), sense: r.querySelector("select").value }))
      .filter((c) => c.param);
    return v;
  }

  function save() {
    collect();
    gate();
    clearTimeout(timer);
    timer = setTimeout(() => invoke("vrchat_settings", { settings: v }), 300);
  }

  function renderContacts() {
    const box = $("vrc-contacts");
    box.innerHTML = "";
    if (!v.contacts.length) {
      const p = document.createElement("p");
      p.className = "note sub";
      p.textContent = t("Automatico: collegamenti scelti dai nomi dei parametri dell'avatar.");
      box.append(p);
    }
    v.contacts.forEach((c) => addRow(c));
  }

  function addRow(c) {
    const row = document.createElement("div");
    row.className = "vrc-contact";
    const inp = document.createElement("input");
    inp.setAttribute("list", "vrc-param-list");
    inp.maxLength = 64;
    inp.placeholder = t("parametro (es. HeadPat)");
    inp.value = c.param || "";
    const sel = document.createElement("select");
    for (const [k, label] of SENSES) sel.add(new Option(t(label), k));
    sel.value = c.sense || "touch";
    const del = document.createElement("button");
    del.textContent = t("Rimuovi");
    del.onclick = () => { row.remove(); save(); };
    inp.onchange = save;
    sel.onchange = save;
    row.append(inp, sel, del);
    $("vrc-contacts").querySelector("p")?.remove();
    $("vrc-contacts").append(row);
  }

  function showParams() {
    const a = avatars.find((x) => x.id === $("vrc-avatar").value);
    const list = $("vrc-param-list");
    list.innerHTML = "";
    if (!a) { $("vrc-params").textContent = ""; return; }
    for (const p of a.params) list.append(new Option(p.name));
    const names = a.params.map((p) => p.name + (p.input ? "" : " ⇠"));
    $("vrc-params").textContent = a.params.length
      ? t("{n} parametri: {list}", { n: a.params.length, list: names.slice(0, 40).join(", ") + (names.length > 40 ? " …" : "") })
      : t("Nessun parametro nel file di questo avatar.");
  }

  async function loadAvatars() {
    avatars = (await invoke("vrchat_avatars").catch(() => [])) || [];
    const sel = $("vrc-avatar");
    sel.innerHTML = "";
    sel.add(new Option(avatars.length ? t("Quello che indossa in VRChat") : t("Nessun avatar trovato: entra in VRChat con l'OSC attivo"), ""));
    for (const a of avatars) sel.add(new Option(a.name, a.id));
    sel.value = avatars.some((a) => a.id === v.avatar) ? v.avatar : "";
    showParams();
  }

  async function loadVision() {
    const r = (await invoke("vrchat_vision_models").catch(() => null)) || {};
    const sel = $("vrc-vmodel");
    sel.innerHTML = "";
    const inst = r.installed || [];
    for (const m of inst) sel.add(new Option(m, m));
    for (const m of r.recommended || []) if (!inst.includes(m)) sel.add(new Option(t("{m} (da scaricare nella Libreria LLM)", { m }), m));
    if (v.vision_model && ![...sel.options].some((o) => o.value === v.vision_model)) sel.add(new Option(v.vision_model, v.vision_model));
    sel.value = v.vision_model || (inst[0] || sel.options[0]?.value || "");
  }

  async function loadDevices() {
    const d = (await invoke("audio_devices").catch(() => null)) || {};
    const sel = $("vrc-hear-dev");
    sel.innerHTML = "";
    sel.add(new Option(t("Uscita predefinita del sistema"), ""));
    for (const o of d.outputs || []) sel.add(new Option(o, o));
    if (v.hearing_device && ![...sel.options].some((o) => o.value === v.hearing_device)) sel.add(new Option(v.hearing_device, v.hearing_device));
    sel.value = v.hearing_device || "";
  }

  async function status() {
    const s = await invoke("vrchat_status").catch(() => null);
    if (!s) return;
    const dot = $("vrc-dot");
    const live = s.running && s.connected;
    dot.style.background = live ? "var(--accent)" : s.running ? "var(--warn)" : "";
    $("vrc-state").textContent = !s.running ? t("Offline") : s.connected ? t("In VRChat") : t("In attesa di VRChat");
    $("vrc-osc").textContent = t("OSC {o}", { o: s.osc || "" });
    $("vrc-leave").disabled = !s.running;
    const parts = [];
    if (s.running && !s.connected) parts.push(t("Nessun pacchetto OSC: in VRChat attiva l'OSC (menu azioni → Opzioni → OSC)."));
    if (s.avatar) parts.push(t("Avatar: {a}", { a: s.avatar.name }));
    if (s.running && s.mode) parts.push(t("Modo: {m}", { m: s.mode }));
    if (s.hearing?.error) parts.push(t("Udito: {e}", { e: s.hearing.error }));
    else if (s.hearing?.on) parts.push(s.hearing.talking ? t("Udito: qualcuno parla") : t("Udito: {n} voci nell'ultimo minuto", { n: s.hearing.voices || 0 }));
    if (s.vision?.error && v.vision !== "off") parts.push(t("Vista: {e}", { e: s.vision.error }));
    else if (s.vision?.last) parts.push(t("Vede: {d}", { d: s.vision.last.text }));
    if (s.error) parts.push(s.error);
    $("vrc-detail").textContent = parts.join(" · ");
    $("vrc-miclink").textContent = s.mic_link ? t("Mic-Link è attivo.") : t("Mic-Link è spento: attivalo nella scheda Voce.");
  }

  for (const id of [...Object.keys(checks), ...Object.keys(fields), "vrc-look", "vrc-send", "vrc-recv"]) $(id).addEventListener("change", save);
  $("vrc-vision").addEventListener("change", () => { if (v.vision === "local") loadVision(); });
  $("vrc-accept").onclick = async () => { await invoke("vrchat_accept"); v.accepted_risk = true; gate(); };
  $("vrc-join").onclick = async () => {
    const r = $("vrc-result");
    r.textContent = "…"; r.className = "result";
    try { await invoke("vrchat_join", { join: true }); r.textContent = t("collegamento avviato"); r.className = "result ok"; }
    catch (e) { r.textContent = String(e); r.className = "result bad"; }
    setTimeout(status, 800);
  };
  $("vrc-leave").onclick = async () => { await invoke("vrchat_join", { join: false }).catch(() => {}); $("vrc-result").textContent = ""; setTimeout(status, 800); };
  $("vrc-avatars-refresh").onclick = loadAvatars;
  $("vrc-avatar").onchange = showParams;
  $("vrc-avatar-use").onclick = async () => {
    const r = $("vrc-avatar-result");
    try { r.textContent = await invoke("vrchat_select_avatar", { id: $("vrc-avatar").value }); r.className = "result ok"; v.avatar = $("vrc-avatar").value; }
    catch (e) { r.textContent = String(e); r.className = "result bad"; }
  };
  $("vrc-contact-add").onclick = () => addRow({ param: "", sense: "touch" });
  $("vrc-contact-auto").onclick = () => { v.contacts = []; renderContacts(); save(); };
  $("vrc-login-go").onclick = async () => {
    const r = $("vrc-login-result");
    r.textContent = "…"; r.className = "result";
    const code = $("vrc-code-row").hidden ? null : $("vrc-code").value.trim();
    const method = methods.find((m) => /totp/i.test(m)) || methods[0] || "totp";
    try {
      const res = await invoke("vrchat_login", { username: $("vrc-user").value, password: $("vrc-pass").value, code, method });
      if (res.step === "2fa") {
        methods = res.methods || [];
        $("vrc-code-row").hidden = false;
        r.textContent = methods.some((m) => /email/i.test(m)) ? t("Scrivi il codice arrivato per email") : t("Scrivi il codice dell'app di autenticazione");
      } else {
        $("vrc-pass").value = ""; $("vrc-code").value = ""; $("vrc-code-row").hidden = true;
        v.account = res.account; gate(); r.className = "result ok";
      }
    } catch (e) { r.textContent = String(e); r.className = "result bad"; }
  };
  $("vrc-logout").onclick = async () => { await invoke("vrchat_logout").catch(() => {}); v.account = ""; gate(); };

  (async () => {
    const cfg = await invoke("get_config").catch(() => null);
    fill(cfg?.vrchat);
    await Promise.all([loadAvatars(), loadDevices(), v.vision === "local" ? loadVision() : null]);
    status();
    setInterval(() => { if (!document.hidden) status(); }, 2500);
  })();
  // da altre finestre cambiano solo consenso e account: il resto non si tocca mentre scrivi
  listen("config", (e) => {
    const c = e.payload?.vrchat;
    if (!c) return;
    v.accepted_risk = c.accepted_risk;
    v.account = c.account;
    gate();
  });
})();
