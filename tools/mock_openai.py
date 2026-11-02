"""Server finto compatibile OpenAI per i test: /ok, /fail500, /flaky (429 poi ok), /badkey (401), /audio/transcriptions. Porta 18080."""
import json, os, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
calls = {"flaky": 0}
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _json(self, code, obj):
        b = json.dumps(obj).encode(); self.send_response(code)
        self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        if self.path.endswith("/models"):
            self._json(200, {"data": [{"id": "mock-b"}, {"id": "mock-a"}]})
        else: self._json(404, {})
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0)); raw = self.rfile.read(n)
        if self.path.endswith("/audio/transcriptions"):
            ok = b'name="file"' in raw and b"RIFF" in raw and b'name="language"' in raw
            return self._json(200 if ok else 400, {"text": " Ciao Alter Ego "} if ok else {"error": "multipart non valido"})
        body = json.loads(raw)
        with open(os.environ.get("MOCK_LAST", "/tmp/mock_last.json"), "w") as f: json.dump(body, f, ensure_ascii=False, indent=1)
        mode = self.path.split("/")[1]
        if mode == "fail500": return self._json(500, {"error": {"message": "boom"}})
        if mode == "badkey": return self._json(401, {"error": {"message": "invalid key " + self.headers.get("Authorization", "")}})
        if mode == "flaky":
            calls["flaky"] += 1
            if calls["flaky"] == 1: return self._json(429, {"error": {"message": "slow down"}})
        self.send_response(200); self.send_header("Content-Type", "text/event-stream"); self.end_headers()
        for piece in ["[happy]\n", "Ciao! ", "Sono il ", f"mock ({body['model']})."]:
            self.wfile.write(f"data: {json.dumps({'choices':[{'delta':{'content':piece}}]})}\n\n".encode()); self.wfile.flush(); time.sleep(0.02)
        self.wfile.write(b"data: [DONE]\n\n")
ThreadingHTTPServer(("127.0.0.1", 18080), H).serve_forever()
