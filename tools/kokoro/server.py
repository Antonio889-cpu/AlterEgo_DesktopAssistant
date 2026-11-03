"""Server locale per Kokoro-82M (https://huggingface.co/hexgrad/Kokoro-82M) compatibile OpenAI.

Usato da Alter Ego per la voce "Kokoro (locale)". Solo libreria standard + `kokoro`:
  POST /v1/audio/speech   {"input": "...", "voice": "if_sara", "speed": 1.0, "response_format": "wav"|"pcm"}
  GET  /v1/audio/voices   elenco voci
  GET  /health            {"status": "ok"} quando il modello è caricato

Avvio:   python server.py [--host 127.0.0.1] [--port 8880] [--device auto|cpu|cuda]
Solo download di modello e voci (installazione):   python server.py --warmup
Ascolta solo su 127.0.0.1: non è raggiungibile da altri PC.
Licenze: modello Kokoro-82M e libreria kokoro sotto Apache-2.0 (hexgrad).
"""
import argparse
import io
import json
import os
import sys
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = "hexgrad/Kokoro-82M"
SAMPLE_RATE = 24000
# prima lettera della voce → lingua di Kokoro
LANGS = {"a": "a", "b": "b", "e": "e", "f": "f", "h": "h", "i": "i", "j": "j", "p": "p", "z": "z"}
VOICES = [
    "if_sara", "im_nicola",
    "af_heart", "af_alloy", "af_aoede", "af_bella", "af_jessica", "af_kore", "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
    "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael", "am_onyx", "am_puck", "am_santa",
    "bf_alice", "bf_emma", "bf_isabella", "bf_lily", "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
    "jf_alpha", "jf_gongitsune", "jf_nezumi", "jf_tebukuro", "jm_kumo",
    "zf_xiaobei", "zf_xiaoni", "zf_xiaoxiao", "zf_xiaoyi", "zm_yunjian", "zm_yunxi", "zm_yunxia", "zm_yunyang",
    "ef_dora", "em_alex", "em_santa", "ff_siwis", "hf_alpha", "hf_beta", "hm_omega", "hm_psi",
    "pf_dora", "pm_alex", "pm_santa",
]

# voci scaricate dall'installazione, così dopo funzionano anche senza internet
PRELOAD = ["if_sara", "im_nicola", "af_heart", "af_bella", "af_nicole", "af_sky", "am_michael", "am_fenrir", "bf_emma", "bm_george"]

state = {"model": None, "pipes": {}, "device": "cpu", "ready": False, "error": None}
lock = threading.Lock()


def log(msg: str, err: bool = False):
    import time

    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr if err else sys.stdout, flush=True)


def load(device: str):
    import time

    t0 = time.time()
    log("Carico PyTorch (al primo avvio può volerci più di un minuto)…")
    import torch

    log(f"PyTorch {torch.__version__} caricato in {time.time() - t0:.0f} s")

    log("Carico la libreria kokoro…")
    from kokoro import KModel

    if device == "auto":
        device = "cuda" if torch.cuda.is_available() else "cpu"
    state["device"] = device
    log(f"Carico il modello Kokoro-82M ({device})…")
    state["model"] = KModel(repo_id=REPO).to(device).eval()


def pipeline(lang: str):
    from kokoro import KPipeline

    if lang not in state["pipes"]:
        state["pipes"][lang] = KPipeline(lang_code=lang, repo_id=REPO, model=state["model"])
    return state["pipes"][lang]


def synth(text: str, voice: str, speed: float):
    import numpy as np

    lang = LANGS.get(voice[:1], "i")
    chunks = []
    with lock:  # un modello solo: una sintesi alla volta
        for _, _, audio in pipeline(lang)(text, voice=voice, speed=speed):
            if audio is not None:
                chunks.append(audio.detach().cpu().numpy() if hasattr(audio, "detach") else np.asarray(audio))
    if not chunks:
        return b""
    data = np.concatenate(chunks)
    return (np.clip(data, -1.0, 1.0) * 32767).astype("<i2").tobytes()


def to_wav(pcm: bytes) -> bytes:
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SAMPLE_RATE)
        w.writeframes(pcm)
    return buf.getvalue()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body: bytes, ctype="application/json"):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _json(self, code, obj):
        self._send(code, json.dumps(obj).encode())

    def do_GET(self):
        path = self.path.rstrip("/")
        if path in ("/health", "/v1/health"):
            if state["ready"]:
                return self._json(200, {"status": "ok", "device": state["device"]})
            return self._json(503, {"status": "loading" if not state["error"] else "error", "error": state["error"]})
        if path in ("/v1/audio/voices", "/audio/voices"):
            return self._json(200, {"voices": VOICES})
        if path in ("/v1/models", "/models"):
            return self._json(200, {"data": [{"id": "kokoro"}]})
        self._json(404, {"error": {"message": "non trovato"}})

    def do_POST(self):
        if self.path.rstrip("/") not in ("/v1/audio/speech", "/audio/speech"):
            return self._json(404, {"error": {"message": "non trovato"}})
        if not state["ready"]:
            return self._json(503, {"error": {"message": state["error"] or "modello in caricamento"}})
        try:
            n = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(n) or b"{}")
            text = str(body.get("input", "")).strip()[:4000]
            voice = str(body.get("voice") or "if_sara")
            speed = min(max(float(body.get("speed") or 1.0), 0.5), 2.0)
            fmt = body.get("response_format") or "wav"
            if not text:
                return self._json(400, {"error": {"message": "testo vuoto"}})
            if voice not in VOICES and "+" not in voice:
                return self._json(400, {"error": {"message": f"voce sconosciuta: {voice}"}})
            try:
                pcm = synth(text, voice, speed)
            except Exception as e:  # noqa: BLE001
                if os.environ.get("HF_HUB_OFFLINE") == "1" and ("offline" in str(e).lower() or "LocalEntryNotFound" in type(e).__name__):
                    return self._json(404, {"error": {"message": f"voce {voice} non scaricata: Impostazioni → Voce e audio → Reinstalla / aggiorna"}})
                raise
            if fmt == "pcm":
                return self._send(200, pcm, "audio/pcm")
            if fmt != "wav":
                return self._json(400, {"error": {"message": "formati supportati: wav, pcm"}})
            self._send(200, to_wav(pcm), "audio/wav")
        except Exception as e:  # noqa: BLE001 - l'errore va al client
            self._json(500, {"error": {"message": f"{type(e).__name__}: {e}"}})


def watch_parent(pid: int):
    """Esce quando termina il processo `pid`, senza toccare stdin/stdout."""
    import time

    if os.name == "nt":
        import ctypes

        kernel32 = ctypes.windll.kernel32
        SYNCHRONIZE = 0x00100000
        handle = kernel32.OpenProcess(SYNCHRONIZE, False, pid)
        if not handle:
            return  # processo non accessibile: meglio restare attivi che uscire a caso
        kernel32.WaitForSingleObject(handle, 0xFFFFFFFF)
    else:
        while os.getppid() == pid:
            time.sleep(1)
    os._exit(0)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8880)
    ap.add_argument("--device", default="auto", choices=["auto", "cpu", "cuda"])
    ap.add_argument("--warmup", action="store_true", help="scarica modello e voci italiane/inglesi, poi esce")
    ap.add_argument("--parent-pid", type=int, default=0,
                    help="esce quando termina questo processo (Alter Ego), anche se va in crash")
    args = ap.parse_args()

    if args.warmup:
        load("cpu")
        for voice in PRELOAD:
            log(f"Scarico e provo la voce {voice}…")
            synth("Ciao, sono Alter Ego." if voice[0] == "i" else "Hi, I'm Alter Ego.", voice, 1.0)
        log("Kokoro pronto.")
        return

    class Server(ThreadingHTTPServer):
        # coda di connessioni ampia: mentre Python carica PyTorch il thread che accetta le
        # connessioni può restare fermo a lungo, e su Windows una coda piena = "connessione rifiutata"
        request_queue_size = 128
        daemon_threads = True

    srv = Server((args.host, args.port), Handler)

    def loader():
        try:
            load(args.device)
            log("Preparo la voce italiana…")
            pipeline("i")
            state["ready"] = True
            log(f"Kokoro pronto su http://{args.host}:{args.port} ({state['device']})")
        except Exception as e:  # noqa: BLE001
            import traceback

            state["error"] = f"{type(e).__name__}: {e}"
            traceback.print_exc()
            log("Errore nel caricamento: " + state["error"], err=True)

    threading.Thread(target=loader, daemon=True).start()
    if args.parent_pid:
        # NON leggere stdin in un thread: su Windows una lettura bloccata su una pipe blocca anche
        # gli altri thread che toccano gli handle standard (es. import torch → subprocess) = stallo.
        threading.Thread(target=watch_parent, args=(args.parent_pid,), daemon=True).start()
    log(f"In ascolto su http://{args.host}:{args.port} (caricamento del modello…)")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
