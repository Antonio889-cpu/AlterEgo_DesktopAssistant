// Console Minecraft: log in diretta del bot (chat, server, eventi, comandi, login mascherati, ponte
// ViaProxy, errori), filtri, ricerca e una riga per scrivere in chat o mandare comandi dall'account di
// Alter Ego. Backend: src-tauri/src/mclink/console.rs (comandi minecraft_console_*).
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const $ = (id) => document.getElementById(id);
  const tt = (s, v) => (typeof t === "function" ? t(s, v) : s);
  const MAX = 2000; // righe nel DOM (come il buffer in memoria)
  const LABEL = { chat: "chat", sys: "server", event: "evento", cmd: "inviato", auth: "login", via: "ponte", error: "errore" };
  const LABEL_EN = { chat: "chat", sys: "server", event: "event", cmd: "sent", auth: "login", via: "bridge", error: "error" };

  let rows = []; // { id, ts, kind, text }
  let lastId = 0;
  let kind = "all";
  let query = "";
  const log = $("log");

  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const fmt = (ms) => new Date(ms).toLocaleTimeString(typeof I18N !== "undefined" ? I18N.lang() : "it", { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" });
  const visible = (r) => (kind === "all" || r.kind === kind) && (!query || r.text.toLowerCase().includes(query));

  function line(r) {
    let text = esc(r.text);
    if (query) {
      const q = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      text = text.replace(new RegExp(esc(q), "gi"), (m) => `<mark>${m}</mark>`);
    }
    const en = typeof I18N !== "undefined" && I18N.lang() === "en";
    return `<div class="ln ${esc(r.kind)}"><time>${fmt(r.ts)}</time><b>${esc((en ? LABEL_EN : LABEL)[r.kind] || r.kind)}</b><span>${text}</span></div>`;
  }

  function atBottom() {
    return log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  }
  function scroll() {
    if ($("follow").checked) log.scrollTop = log.scrollHeight;
  }

  function render() {
    log.innerHTML = rows.filter(visible).map(line).join("");
    $("empty").hidden = rows.length > 0;
    scroll();
  }

  function add(list) {
    const fresh = list.filter((r) => r && r.id > lastId);
    if (!fresh.length) return;
    lastId = fresh[fresh.length - 1].id;
    rows.push(...fresh);
    const cut = rows.length - MAX;
    if (cut > 0) {
      rows = rows.slice(cut);
      render();
      return;
    }
    const html = fresh.filter(visible).map(line).join("");
    if (html) log.insertAdjacentHTML("beforeend", html);
    while (log.childElementCount > MAX) log.firstElementChild.remove();
    $("empty").hidden = rows.length > 0;
    scroll();
  }

  // ---------- filtri e ricerca ----------
  $("chips").querySelectorAll("button").forEach((b) => {
    b.onclick = () => {
      kind = b.dataset.kind;
      $("chips").querySelectorAll("button").forEach((x) => x.classList.toggle("on", x === b));
      render();
    };
  });
  let qTimer = null;
  $("q").addEventListener("input", () => {
    clearTimeout(qTimer);
    qTimer = setTimeout(() => { query = $("q").value.trim().toLowerCase(); render(); }, 120);
  });
  log.addEventListener("scroll", () => { $("follow").checked = atBottom(); });
  $("follow").onchange = scroll;
  $("clear").onclick = async () => {
    await invoke("minecraft_console_clear").catch(() => {});
    rows = [];
    render();
  };

  // ---------- riga di input con cronologia ----------
  const HKEY = "mc-console-history";
  let history = [];
  try { history = JSON.parse(localStorage.getItem(HKEY) || "[]").filter((x) => typeof x === "string").slice(-50); } catch {}
  let hi = history.length;
  // i comandi con la password non finiscono nella cronologia
  const secret = (s) => /^\/(login|l|log|register|reg|changepassword|changepass|cp|unregister|2fa)\b/i.test(s);
  const status = (text, bad) => { $("status").textContent = text || ""; $("status").className = "status" + (bad ? " bad" : ""); };

  $("input").addEventListener("submit", async (e) => {
    e.preventDefault();
    const text = $("line").value.trim();
    if (!text) return;
    $("send").disabled = true;
    try {
      await invoke("minecraft_console_send", { text });
      if (!secret(text)) {
        history = history.filter((h) => h !== text);
        history.push(text);
        history = history.slice(-50);
        try { localStorage.setItem(HKEY, JSON.stringify(history)); } catch {}
      }
      hi = history.length;
      $("line").value = "";
      status("");
    } catch (err) {
      status(String(err), true);
    } finally {
      $("send").disabled = false;
      $("line").focus();
    }
  });
  $("line").addEventListener("keydown", (e) => {
    if (e.key === "ArrowUp" && history.length) {
      e.preventDefault();
      hi = Math.max(0, hi - 1);
      $("line").value = history[hi] || "";
    } else if (e.key === "ArrowDown" && history.length) {
      e.preventDefault();
      hi = Math.min(history.length, hi + 1);
      $("line").value = history[hi] || "";
    }
  });

  // ---------- stato in alto ----------
  async function state() {
    const [s, v] = await Promise.all([invoke("minecraft_status").catch(() => null), invoke("minecraft_via_status").catch(() => null)]);
    const pill = $("pill");
    if (s && s.in_game) {
      const w = s.world || {};
      $("pill-text").textContent = tt("Nel mondo");
      pill.className = "pill ok";
      $("where").textContent = tt("Minecraft {v} su {h}:{p}", { v: w.version || "?", h: w.host || "?", p: w.port || "?" }) + (v && v.active ? " · " + tt("ponte ViaProxy") : "") + (s.players?.length ? " · " + s.players.join(", ") : "");
    } else if (s && s.running) {
      $("pill-text").textContent = tt("Collegamento…");
      pill.className = "pill wait";
      $("where").textContent = v && v.active ? tt("ponte acceso verso {t}", { t: v.active.target }) : "";
    } else {
      $("pill-text").textContent = tt("Non collegato");
      pill.className = "pill";
      $("where").textContent = tt("Entra nel mondo dalle impostazioni (Giochi → Minecraft)");
    }
    $("send").disabled = !(s && s.running);
  }

  listen("minecraft-console", (e) => add([e.payload]));
  listen("minecraft-link", () => setTimeout(state, 300));
  (async () => {
    try { I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language)); } catch {}
    try { add((await invoke("minecraft_console_list", { since: 0 })) || []); } catch {}
    render();
    state();
    setInterval(state, 3000);
    $("line").focus();
  })();
})();
