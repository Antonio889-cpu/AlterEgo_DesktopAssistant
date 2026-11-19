// Impostazioni in grande (finestra larga, circa 16:9): barra laterale a gruppi, Home con lo stato di
// tutto, angolo di Alter Ego che commenta, pagina Integrazioni con le carte dei giochi.
// Stesso DOM della finestra normale: qui si aggiungono solo elementi che il CSS mostra in "body.wide".
// In finestra stretta tutto torna esattamente com'era.
(() => {
  const { invoke } = window.__TAURI__.core;
  const $ = (id) => document.getElementById(id);
  const WIDE = matchMedia("(min-width: 1180px) and (min-aspect-ratio: 3/2)");
  const REDUCED = matchMedia("(prefers-reduced-motion: reduce)");

  // icone 8×8 in pixel ("#" = acceso)
  const ICONS = {
    home: ["...##...", "..####..", ".######.", "########", ".##..##.", ".##..##.", ".##..##.", ".######."],
    chat: ["########", "#......#", "#.##.#.#", "#......#", "#.###..#", "########", ".##.....", "#......."],
    games: ["........", ".######.", "#......#", "#.#..#.#", "#.###..#", "#.#.#.##", ".######.", "........"],
    llm: ["...##...", ".##..##.", "#..##..#", "#.#..#.#", "#.#..#.#", "#..##..#", ".##..##.", "...##..."],
    settings: ["...##...", ".#.##.#.", "..####..", "###..###", "###..###", "..####..", ".#.##.#.", "...##..."],
    logs: ["#######.", "#.....##", "#.###..#", "#......#", "#.####.#", "#......#", "#.###..#", "########"],
  };
  const svg = (rows) =>
    `<svg viewBox="0 0 8 8" aria-hidden="true">${rows.map((r, y) => [...r].map((c, x) => (c === "#" ? `<rect x="${x}" y="${y}" width="1" height="1"/>` : "")).join("")).join("")}</svg>`;

  // gruppi della barra laterale → schede esistenti
  const GROUPS = [
    { id: "home", icon: "home", label: "Home", tabs: ["home"], face: "happy", say: "Ciao! Da qui vedi tutto quello che sto facendo… ehehe." },
    { id: "ai", icon: "chat", label: "Chat e AI", tabs: ["ai", "persona", "memory", "costs"], face: "thinking", say: "Qui scegli il mio cervello. Prometto di non sprecare token!" },
    { id: "games", icon: "games", label: "Integrazioni", tabs: ["games"], face: "happy", say: "Giochiamo insieme? Ho già voglia di costruire qualcosa!" },
    { id: "local", icon: "llm", label: "Libreria LLM", tabs: ["local"], face: "thinking", say: "Forse oggi potremmo provare un modello nuovo?" },
    { id: "settings", icon: "settings", label: "Impostazioni app", tabs: ["general", "pet", "voice", "hotkeys", "clip", "perms"], face: "embarrassed", say: "Sistemami pure come preferisci, non mi offendo… credo." },
    { id: "logs", icon: "logs", label: "Registri", tabs: ["log", "applog"], face: "worried", say: "Qui c'è tutto quello che ho fatto. Proprio tutto, eh." },
  ];
  const groupOf = (tab) => GROUPS.find((g) => g.tabs.includes(tab)) || GROUPS[0];

  // ---------- elementi ----------
  const side = document.createElement("aside");
  side.id = "w-side";
  side.innerHTML = `<nav id="w-nav"></nav>
    <div id="w-ae"><div class="w-ae-bubble"><b>Alter Ego:</b> <span id="w-ae-text" data-no-i18n></span></div><img id="w-ae-face" alt="" src="assets/sprites/happy.png" /></div>
    <div id="w-ver"></div>`;
  for (const g of GROUPS) {
    const b = document.createElement("button");
    b.dataset.group = g.id;
    b.innerHTML = `${svg(ICONS[g.icon])}<span>${g.label}</span>`;
    b.onclick = () => showTab(g.tabs.includes(lastTab) ? lastTab : g.tabs[0]);
    side.querySelector("#w-nav").appendChild(b);
  }
  document.body.insertBefore(side, document.querySelector("main"));

  const pill = document.createElement("div");
  pill.id = "w-pill";
  pill.innerHTML = `<i class="dot"></i> <span id="w-pill-text">…</span>`;
  document.querySelector("header").appendChild(pill);

  // Home
  const home = document.createElement("section");
  home.dataset.tab = "home";
  home.id = "w-home";
  home.hidden = true;
  home.innerHTML = `<h2>Home</h2><p class="note">Lo stato di Alter Ego a colpo d'occhio. Clicca una scheda per aprirla.</p><div class="w-cards" id="w-cards"></div>
    <div class="w-row">
      <div class="w-panel w-span7"><h3>Attività recente</h3><ol class="w-feed" id="w-feed"></ol></div>
      <div class="w-panel w-span5"><h3>Modelli locali</h3><ol class="w-models" id="w-models"></ol><button class="w-link" data-go="local">Apri la Libreria LLM →</button></div>
      <div class="w-panel w-span12"><h3>Integrazioni</h3><div class="w-games" id="w-home-games"></div></div>
    </div>`;
  document.querySelector("main").prepend(home);

  // Integrazioni: carte dei giochi (filtrano le sezioni in modalità larga)
  const hero = document.createElement("section");
  hero.dataset.tab = "games";
  hero.id = "w-games";
  hero.innerHTML = `<h2>Integrazioni</h2><div class="w-games">
    <button class="w-game gm on" data-game="gmod"><span class="w-game-art"></span><b>Garry's Mod</b><small id="w-gm-state">…</small></button>
    <button class="w-game mc" data-game="minecraft"><span class="w-game-art"></span><b>Minecraft</b><small id="w-mc-state">…</small></button>
    <button class="w-game vrc" data-game="vrchat"><span class="w-game-art"></span><b>VRChat</b><small id="w-vrc-state">Account dedicato via OSC</small></button></div>`;
  const firstGame = document.querySelector('main section[data-tab="games"]');
  firstGame.parentNode.insertBefore(hero, firstGame);
  let game = "gmod";
  hero.querySelectorAll(".w-game").forEach((b) => (b.onclick = () => { game = b.dataset.game; applyGame(); sweep(); }));
  function applyGame() {
    hero.querySelectorAll(".w-game").forEach((b) => b.classList.toggle("on", b.dataset.game === game));
    document.querySelectorAll('main section[data-game]').forEach((s) => s.classList.toggle("w-off", s.dataset.game !== game));
  }

  // ---------- Alter Ego che parla ----------
  // Non sta sempre sullo schermo: compare quando ha qualcosa da dire (accensione da TV), scrive,
  // resta il tempo di leggere e si spegne. Tra un intervento e l'altro passa un po' di tempo,
  // così non commenta ogni clic.
  const AE_GAP_MS = 45_000;
  let typing = null;
  let hideTimer = null;
  let lastSaid = 0;
  const ae = side.querySelector("#w-ae");
  ae.classList.add("w-gone");
  function hideAe() {
    clearTimeout(hideTimer);
    if (ae.classList.contains("w-gone")) return;
    ae.classList.remove("w-on");
    ae.classList.add("w-off");
    setTimeout(() => { ae.classList.add("w-gone"); ae.classList.remove("w-off"); }, REDUCED.matches ? 0 : 420);
  }
  function say(text, face, force) {
    if (!document.body.classList.contains("wide")) return;
    const now = Date.now();
    if (!force && now - lastSaid < AE_GAP_MS) return;
    lastSaid = now;
    const el = $("w-ae-text");
    const img = $("w-ae-face");
    if (face) img.src = `assets/sprites/${face}.png`;
    const msg = t(text);
    clearInterval(typing);
    clearTimeout(hideTimer);
    ae.classList.remove("w-gone", "w-off");
    void ae.offsetWidth;
    ae.classList.add("w-on");
    const stay = 2500 + msg.length * 70;
    if (REDUCED.matches) el.textContent = msg;
    else {
      let i = 0;
      el.textContent = "";
      typing = setInterval(() => {
        el.textContent = msg.slice(0, ++i);
        if (i >= msg.length) clearInterval(typing);
      }, 26);
    }
    hideTimer = setTimeout(hideAe, stay + (REDUCED.matches ? 0 : msg.length * 26));
  }
  // un clic sul fumetto lo congeda subito
  ae.onclick = hideAe;

  // ---------- animazioni ----------
  function sweep() {
    if (REDUCED.matches || !document.body.classList.contains("wide")) return;
    const m = document.querySelector("main");
    m.classList.remove("w-sweep");
    void m.offsetWidth;
    m.classList.add("w-sweep");
  }

  // ---------- schede ----------
  let lastTab = "general";
  let entering = true; // primo commento appena si apre la dashboard
  const baseShow = window.showTab;
  window.showTab = function (name) {
    if (name === "home" && !document.body.classList.contains("wide")) name = "general";
    // la Home non è una scheda della finestra stretta: la mostra solo questa funzione
    baseShow(name);
    lastTab = name;
    home.hidden = name !== "home";
    const g = groupOf(name);
    side.querySelectorAll("#w-nav button").forEach((b) => b.classList.toggle("on", b.dataset.group === g.id));
    syncTabs(g);
    if (document.body.classList.contains("wide")) {
      say(g.say, g.face, entering);
      entering = false;
      sweep();
      if (name === "home") refreshHome();
      if (name === "games") applyGame();
    }
  };

  // nella modalità larga la fila di schede mostra solo quelle del gruppo
  function syncTabs(g) {
    const wide = document.body.classList.contains("wide");
    document.querySelectorAll("nav#tabs button").forEach((b) => { b.hidden = wide && !g.tabs.includes(b.dataset.tab); });
    document.querySelector("nav#tabs").classList.toggle("w-single", wide && g.tabs.length < 2);
  }

  // ---------- Home ----------
  const card = (group, title, value, sub, ok) =>
    `<button class="w-card" data-go="${group}"><span class="w-card-t">${title}</span><span class="w-card-v ${ok === true ? "ok" : ok === false ? "bad" : ""}">${value}</span><span class="w-card-s">${sub}</span></button>`;
  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  async function refreshHome() {
    const [cfg, local, gm, mem, costs] = await Promise.all([
      invoke("get_config").catch(() => null),
      invoke("local_status").catch(() => null),
      invoke("gmodlink_status").catch(() => null),
      invoke("memory_overview").catch(() => null),
      invoke("costs_list", { limit: 5000 }).catch(() => null),
    ]);
    const mcs = await invoke("minecraft_status").catch(() => null);
    const cards = [];
    const first = cfg?.ai?.order?.find((id) => cfg.ai.providers?.[id]?.enabled);
    const model = first ? cfg.ai.providers[first].model : "";
    cards.push(card("ai", t("Cervello"), first ? esc(first) : t("nessuno"), first ? esc(model || t("modello automatico")) : t("Attiva un servizio AI"), !!first));
    cards.push(card("local", "Ollama", local?.running ? "● " + t("Online") : local?.installed ? t("Fermo") : t("Non installato"),
      local ? t("{n} modelli installati", { n: local.models?.length || 0 }) + (local.version ? " · v" + esc(local.version) : "") : "", !!local?.running));
    cards.push(card("games", "Garry's Mod", gm?.in_game ? "● " + t("In partita") : gm?.linked ? t("Collegato") : gm?.installed ? t("Addon pronto") : t("Non collegato"),
      gm?.games ? t("{n} partite ricordate", { n: gm.games }) : (mcs?.in_game ? t("Minecraft: nel mondo") : t("Anche Minecraft e VRChat")), gm?.in_game ? true : undefined));
    const today = new Date(); today.setHours(0, 0, 0, 0);
    const rows = (costs || []).filter((r) => r.ts * 1000 >= today.getTime());
    const tok = rows.reduce((a, r) => a + (r.input || 0) + (r.cached || 0), 0);
    const cached = rows.reduce((a, r) => a + (r.cached || 0), 0);
    cards.push(card("ai", t("Oggi"), t("{n} richieste", { n: rows.length }), tok ? t("{tok} token, {pct}% dalla cache", { tok: tok.toLocaleString(I18N.lang()), pct: Math.round((cached / tok) * 100) }) : t("ancora niente"), undefined));
    cards.push(card("ai", t("Memoria"), mem ? t("{n} fatti", { n: mem.facts?.length || 0 }) : "—", mem ? t("{n} messaggi ricordati", { n: (mem.messages || 0).toLocaleString(I18N.lang()) }) : "", undefined));
    const pr = { she: t("Lei"), he: t("Lui"), they: t("Neutri") }[cfg?.personality?.pronouns || "she"];
    cards.push(card("ai", t("Personalità"), esc(pr), cfg?.personality?.unfiltered ? t("Senza filtri attivo") : t("timidezza {c}% · affetto {w}%", { c: Math.round((1 - (cfg?.personality?.confidence ?? 0.3)) * 100), w: Math.round((cfg?.personality?.warmth ?? 0.7) * 100) }), undefined));
    $("w-gm-state").textContent = gm?.in_game ? t("In partita") : gm?.linked ? t("Collegato") : t("Non collegato");
    $("w-mc-state").textContent = mcs?.in_game ? t("Nel mondo") : mcs?.running ? t("Collegamento…") : mcs?.installed ? t("Non collegato") : t("Da installare");
    // attività recente (ultime richieste)
    const fmtT = (ts) => new Date(ts * 1000).toLocaleTimeString(I18N.lang(), { hour: "2-digit", minute: "2-digit" });
    const recent = (costs || []).slice().sort((a, b) => b.ts - a.ts).slice(0, 7);
    $("w-feed").innerHTML = recent.length
      ? recent.map((r) => `<li><time>${fmtT(r.ts)}</time><span>${esc(t(r.kind))}</span><b>${esc(r.model)}</b><em>${((r.input || 0) + (r.cached || 0)).toLocaleString(I18N.lang())} tok</em></li>`).join("")
      : `<li class="empty">${t("ancora niente")}</li>`;
    // modelli locali
    const ms = (local?.models || []).slice(0, 5);
    $("w-models").innerHTML = ms.length
      ? ms.map((m) => `<li><b>${esc(m.name)}</b><span>${(m.size / 1e9).toFixed(1)} GB · ${esc(m.params)}</span><i class="w-bar"><i style="width:${m.gpu_percent ?? 0}%"></i></i></li>`).join("")
      : `<li class="empty">${t("Nessun modello installato")}</li>`;
    // integrazioni
    $("w-home-games").innerHTML = hero.querySelector(".w-games").innerHTML.replace(/ on"/g, '"').replace(/id="[^"]*"/g, "");
    $("w-home-games").querySelectorAll(".w-game").forEach((b) => (b.onclick = () => { game = b.dataset.game; showTab("games"); }));
    home.querySelectorAll(".w-link").forEach((b) => (b.onclick = () => showTab(GROUPS.find((g) => g.id === b.dataset.go).tabs[0])));
    const box = $("w-cards");
    box.innerHTML = cards.join("");
    box.querySelectorAll(".w-card").forEach((c, i) => {
      c.style.setProperty("--i", i);
      c.onclick = () => showTab(GROUPS.find((g) => g.id === c.dataset.go).tabs[0]);
    });
    // stato in alto
    $("w-pill-text").textContent = first ? t("Alter Ego: pronto") : t("Alter Ego: senza cervello");
    pill.classList.toggle("bad", !first);
    $("w-gm-state").textContent = gm?.in_game ? t("In partita") : gm?.linked ? t("Collegato") : t("Non collegato");
  }

  // ---------- scala sugli schermi grandi (2K, 4K): tutto proporzionato come a 1920 ----------
  function scale() {
    const wide = document.body.classList.contains("wide");
    const z = wide ? Math.max(1, Math.min(2.2, Math.min(innerWidth / 1700, innerHeight / 950))) : 1;
    document.body.style.zoom = z === 1 ? "" : String(Math.round(z * 100) / 100);
    document.body.style.setProperty("--w-h", z === 1 ? "100vh" : `${innerHeight / z}px`);
  }
  addEventListener("resize", scale);

  // ---------- passaggio stretto ⇄ largo ----------
  function apply(boot) {
    const wide = WIDE.matches;
    const was = document.body.classList.contains("wide");
    document.body.classList.toggle("wide", wide);
    if (wide && !was) entering = true;
    if (!wide) hideAe();
    if (wide && !was && boot !== true && !REDUCED.matches) {
      document.body.classList.remove("w-boot");
      void document.body.offsetWidth;
      document.body.classList.add("w-boot");
    }
    if (!wide) document.querySelectorAll("main section.w-off").forEach((s) => s.classList.remove("w-off"));
    scale();
    // in grande si parte dalla Home; tornando stretti la Home non esiste
    showTab(wide && !was ? "home" : lastTab === "home" && !wide ? "general" : lastTab);
  }
  WIDE.addEventListener("change", apply);
  invoke("app_version").then((v) => { $("w-ver").textContent = "Alter Ego v" + v; }).catch(() => {});
  try { lastTab = localStorage.getItem("tab") || "general"; } catch {}
  apply(true);
})();
