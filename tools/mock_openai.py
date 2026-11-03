"""Server finto compatibile OpenAI per i test: /ok, /fail500, /flaky (429 poi ok), /badkey (401), /keycheck (solo 'Bearer good'), /tools (tool calling), /audio/transcriptions, /audio/speech (WAV 24 kHz), /health. Porta 18080."""
import json, os, struct, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
calls = {"flaky": 0}
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _json(self, code, obj):
        b = json.dumps(obj).encode(); self.send_response(code)
        self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        if self.path.endswith("/health"):
            self._json(200, {"status": "ok"})
        elif self.path.endswith("/models"):
            self._json(200, {"data": [{"id": "mock-b"}, {"id": "mock-a"}]})
        else: self._json(404, {})
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0)); raw = self.rfile.read(n)
        if self.path.endswith("/audio/transcriptions"):
            ok = b'name="file"' in raw and b"RIFF" in raw and b'name="language"' in raw
            return self._json(200 if ok else 400, {"text": " Ciao Alter Ego "} if ok else {"error": "multipart non valido"})
        body = json.loads(raw)
        with open(os.environ.get("MOCK_LAST", "/tmp/mock_last.json"), "w") as f: json.dump(body, f, ensure_ascii=False, indent=1)
        if self.path.endswith("/tools/search"):
            ok = self.headers.get("Authorization", "").startswith("Bearer ") and 1 <= len(body.get("query", "")) <= 128
            return self._json(200 if ok else 400, {"results": [{"title": "Risultato per " + body.get("query", ""), "url": "https://example.com/a", "snippet": "estratto di prova"}]})
        if self.path.endswith("/tools/fetch"):
            return self._json(200, {"title": "Pagina", "markdown": "# Pagina di prova\ncontenuto"})
        if self.path.endswith("/audio/speech"):
            with open("/tmp/mock_speech.json", "w") as f: json.dump(body, f)  # file a parte: i test girano in parallelo
            # 0,2 s di tono a 24 kHz, 16 bit mono
            import math
            pcm = b"".join(struct.pack("<h", int(8000 * math.sin(i / 10))) for i in range(4800))
            wav = b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, 24000, 48000, 2, 16) + b"data" + struct.pack("<I", len(pcm)) + pcm
            self.send_response(200); self.send_header("Content-Type", "audio/wav"); self.send_header("Content-Length", str(len(wav))); self.end_headers(); self.wfile.write(wav)
            return
        mode = self.path.split("/")[1]
        if mode == "keycheck" and self.headers.get("Authorization") != "Bearer good":
            return self._json(429 if "quota" in self.headers.get("Authorization", "") else 401, {"error": {"message": "bad key"}})
        if mode == "fail500": return self._json(500, {"error": {"message": "boom"}})
        if mode == "badkey": return self._json(401, {"error": {"message": "invalid key " + self.headers.get("Authorization", "")}})
        if mode == "flaky":
            calls["flaky"] += 1
            if calls["flaky"] == 1: return self._json(429, {"error": {"message": "slow down"}})
        if mode == "tools" and body.get("tools") and body["messages"][-1]["role"] != "tool":
            last = body["messages"][-1]["content"]
            if "promemoria" in last:
                call = {"name": "set_reminder", "arguments": json.dumps({"text": "pausa", "minutes": 1})}
            elif "metti" in last:
                call = {"name": "play_music", "arguments": json.dumps({"query": last.split("metti", 1)[1].strip()})}
            elif "cerca" in last:
                call = {"name": "web_search", "arguments": json.dumps({"query": last.split("cerca", 1)[1].strip(), "count": 3})}
            elif "comando" in last:
                call = {"name": "run_shell", "arguments": json.dumps({"shell": "cmd", "command": "echo ciao-dal-terminale"})}
            else:
                call = {"name": "write_file", "arguments": json.dumps({"path": os.environ.get("MOCK_FILE", "/tmp/ae_box/nota.txt"), "content": "ciao da Alter Ego"})}
            self.send_response(200); self.send_header("Content-Type", "text/event-stream"); self.end_headers()
            args = call["arguments"]
            for i, part in enumerate([args[:10], args[10:]]):
                d = {"index": 0, "function": {"arguments": part}}
                if i == 0: d.update({"id": "call_1", "type": "function", "function": {"name": call["name"], "arguments": part}})
                self.wfile.write(f"data: {json.dumps({'choices':[{'delta':{'tool_calls':[d]}}]})}\n\n".encode())
            self.wfile.write(b"data: [DONE]\n\n"); return
        if mode == "tools" and body["messages"][-1]["role"] == "tool":
            self.send_response(200); self.send_header("Content-Type", "text/event-stream"); self.end_headers()
            res = body["messages"][-1]["content"][:60]
            self.wfile.write(f"data: {json.dumps({'choices':[{'delta':{'content':'[happy]' + chr(10) + 'Fatto: ' + res}}]})}\n\n".encode())
            self.wfile.write(b"data: [DONE]\n\n"); return
        self.send_response(200); self.send_header("Content-Type", "text/event-stream"); self.end_headers()
        for piece in ["[happy]\n", "Ciao! ", "Sono il ", f"mock ({body['model']})."]:
            self.wfile.write(f"data: {json.dumps({'choices':[{'delta':{'content':piece}}]})}\n\n".encode()); self.wfile.flush(); time.sleep(0.02)
        if body.get("stream_options", {}).get("include_usage"):
            u = {"prompt_tokens": 1200, "prompt_cache_hit_tokens": 1000, "prompt_cache_miss_tokens": 200, "completion_tokens": 30}
            self.wfile.write(f"data: {json.dumps({'choices': [], 'usage': u})}\n\n".encode())
        self.wfile.write(b"data: [DONE]\n\n")
ThreadingHTTPServer(("127.0.0.1", 18080), H).serve_forever()
