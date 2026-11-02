"""Server finto compatibile OpenAI per i test: /ok, /fail500, /flaky (429 poi ok), /badkey (401), /keycheck (solo 'Bearer good'), /tools (tool calling), /audio/transcriptions. Porta 18080."""
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
        self.wfile.write(b"data: [DONE]\n\n")
ThreadingHTTPServer(("127.0.0.1", 18080), H).serve_forever()
