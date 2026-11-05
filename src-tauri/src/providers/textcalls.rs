//! Pulizia delle risposte dei modelli:
//! - il "pensiero" (`<think>…</think>` e simili) non deve finire nel box di dialogo;
//! - alcuni modelli scrivono le chiamate a strumenti come testo invece di usare il campo
//!   `tool_calls` (DeepSeek "DSML", Hermes/Qwen `<tool_call>`, Llama `<function=…>`,
//!   Mistral `[TOOL_CALLS]`, il vecchio formato DeepSeek con i token speciali):
//!   qui le si riconosce, le si toglie dal testo e, se lo strumento è tra quelli offerti,
//!   diventano chiamate vere (che passano comunque dal sistema dei permessi).

use super::ToolCall;
use serde_json::{json, Map, Value};

/// Toglie i blocchi di ragionamento. Gestisce anche il tag di apertura mancante
/// (alcuni template lo mettono nel prompt: nel testo arriva solo `</think>`)
/// e quello di chiusura mancante (risposta troncata: resta solo il ragionamento → vuoto).
pub fn strip_thinking(text: &str) -> String {
    let mut s = text.to_string();
    for (open, close) in [
        ("<think>", "</think>"),
        ("<thinking>", "</thinking>"),
        ("<reasoning>", "</reasoning>"),
        ("<thought>", "</thought>"),
        ("◁think▷", "◁/think▷"),
    ] {
        loop {
            let lower = s.to_lowercase();
            let (Some(a), b) = (lower.find(open), lower.find(close)) else {
                // solo la chiusura: tutto quello che c'è prima era ragionamento
                if let Some(b) = lower.find(close) {
                    s = s[b + close.len()..].to_string();
                    continue;
                }
                break;
            };
            match b.filter(|b| *b > a) {
                Some(b) => s.replace_range(a..b + close.len(), ""),
                None => s.truncate(a), // aperto e mai chiuso
            }
        }
    }
    s.trim().to_string()
}

/// Normalizza i separatori dei token speciali (｜ a larghezza piena, ▁) per cercarli più facilmente.
fn norm(s: &str) -> String {
    s.replace('｜', "|").replace('▁', "_")
}

fn find_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    hay.get(from..)?.to_lowercase().find(&needle.to_lowercase()).map(|i| i + from)
}

/// Valore di un attributo `name="…"` dentro un tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let i = find_ci(tag, &format!("{name}="), 0)? + name.len() + 1;
    let rest = &tag[i..];
    let q = rest.chars().next()?;
    if q == '"' || q == '\'' {
        let end = rest[1..].find(q)?;
        Some(rest[1..1 + end].to_string())
    } else {
        Some(rest.split(|c: char| c.is_whitespace() || c == '>').next()?.to_string())
    }
}

/// Valore di un parametro: JSON se `string="false"` o se sembra JSON, altrimenti stringa.
fn param_value(raw: &str, is_string: Option<bool>) -> Value {
    let t = raw.trim();
    if is_string != Some(true) {
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            return v;
        }
    }
    Value::String(raw.trim_matches('\n').to_string())
}

/// DeepSeek V3.x "DSML":
/// `<｜DSML｜function_calls><｜DSML｜invoke name="x"><｜DSML｜parameter name="k" string="true">v</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜function_calls>`
fn parse_dsml(s: &str) -> (Vec<(String, Value)>, Vec<(usize, usize)>) {
    let mut calls = Vec::new();
    let mut cuts = Vec::new();
    let mut pos = 0;
    while let Some(start) = find_ci(s, "dsml", pos) {
        // inizio del tag: il '<' prima di "DSML"
        let block_start = s[..start].rfind('<').unwrap_or(start);
        // fine del blocco: dopo l'ultimo "function_calls>" o, se manca, alla fine del testo
        let end = find_ci(s, "function_calls>", start + 4)
            .and_then(|first| {
                // il primo "function_calls>" è l'apertura: cerco la chiusura dopo
                find_ci(s, "function_calls>", first + 15).map(|e| e + 15)
            })
            .unwrap_or(s.len());
        let block = &s[block_start..end];
        let mut ip = 0;
        while let Some(inv) = find_ci(block, "invoke ", ip) {
            let tag_end = block[inv..].find('>').map(|e| inv + e).unwrap_or(block.len());
            let Some(name) = attr(&block[inv..tag_end], "name") else { break };
            let inv_end = find_ci(block, "invoke>", tag_end).unwrap_or(block.len());
            let body = &block[tag_end.min(block.len())..inv_end];
            let mut args = Map::new();
            let mut pp = 0;
            while let Some(p) = find_ci(body, "parameter ", pp) {
                let pt_end = body[p..].find('>').map(|e| p + e).unwrap_or(body.len());
                let ptag = &body[p..pt_end];
                let Some(pname) = attr(ptag, "name") else { break };
                let is_string = attr(ptag, "string").map(|v| v.eq_ignore_ascii_case("true"));
                // il valore finisce al prossimo tag di chiusura del parametro
                let vend = find_ci(body, "parameter>", pt_end + 1)
                    .and_then(|e| body[..e].rfind("</"))
                    .unwrap_or(body.len());
                let raw = body.get(pt_end + 1..vend).unwrap_or("");
                args.insert(pname, param_value(raw, is_string));
                pp = vend.max(pt_end + 1);
            }
            calls.push((name, Value::Object(args)));
            ip = inv_end.max(tag_end + 1);
        }
        cuts.push((block_start, end));
        pos = end.max(start + 4);
        if pos >= s.len() {
            break;
        }
    }
    (calls, cuts)
}

/// `{"name": "...", "arguments": {...}}` (anche `parameters`) → (nome, argomenti).
fn json_call(v: &Value) -> Option<(String, Value)> {
    let f = v.get("function").unwrap_or(v);
    let name = f.get("name")?.as_str()?.to_string();
    let args = f.get("arguments").or_else(|| f.get("parameters")).cloned().unwrap_or(json!({}));
    let args = match args {
        Value::String(s) => serde_json::from_str(&s).unwrap_or(json!({})),
        a => a,
    };
    Some((name, args))
}

/// Primo valore JSON completo a partire da `from` (oggetto o array), con la sua fine.
fn json_at(s: &str, from: usize) -> Option<(Value, usize)> {
    let rel = s.get(from..)?.find(['{', '['])?;
    let start = from + rel;
    let mut de = serde_json::Deserializer::from_str(&s[start..]).into_iter::<Value>();
    let v = de.next()?.ok()?;
    Some((v, start + de.byte_offset()))
}

/// Altri formati testuali: `<tool_call>{json}</tool_call>`, `<function=nome>{json}</function>`,
/// `[TOOL_CALLS][{...}]`, `<|tool_calls_begin|>…<|tool_sep|>nome ```json {…}``` …`.
fn parse_others(s: &str) -> (Vec<(String, Value)>, Vec<(usize, usize)>) {
    let mut calls = Vec::new();
    let mut cuts = Vec::new();
    // Hermes / Qwen
    let mut pos = 0;
    while let Some(a) = find_ci(s, "<tool_call>", pos) {
        let end = find_ci(s, "</tool_call>", a).map(|e| e + 12).unwrap_or(s.len());
        if let Some((v, _)) = json_at(&s[..end], a + 11) {
            if let Some(c) = json_call(&v) {
                calls.push(c);
            }
        }
        cuts.push((a, end));
        pos = end;
    }
    // Llama 3.x
    let mut pos = 0;
    while let Some(a) = find_ci(s, "<function=", pos) {
        let name_end = s[a + 10..].find('>').map(|e| a + 10 + e).unwrap_or(s.len());
        let name = s[a + 10..name_end].trim().trim_matches('"').to_string();
        let end = find_ci(s, "</function>", name_end).map(|e| e + 11).unwrap_or(s.len());
        let args = json_at(&s[..end], name_end).map(|(v, _)| v).unwrap_or(json!({}));
        if !name.is_empty() {
            calls.push((name, args));
        }
        cuts.push((a, end));
        pos = end;
    }
    // Mistral
    if let Some(a) = find_ci(s, "[TOOL_CALLS]", 0) {
        if let Some((v, end)) = json_at(s, a + 12) {
            for item in v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]) {
                if let Some(c) = json_call(&item) {
                    calls.push(c);
                }
            }
            cuts.push((a, end));
        }
    }
    // DeepSeek V2/V3 con token speciali
    if let Some(a) = find_ci(s, "<|tool_calls_begin|>", 0) {
        let end = find_ci(s, "<|tool_calls_end|>", a).map(|e| e + 18).unwrap_or(s.len());
        let block = &s[a..end];
        let mut p = 0;
        while let Some(sep) = find_ci(block, "<|tool_sep|>", p) {
            let after = sep + 12;
            let name: String = block[after..].chars().take_while(|c| !c.is_whitespace() && *c != '`' && *c != '<').collect();
            let args = json_at(block, after + name.len()).map(|(v, _)| v).unwrap_or(json!({}));
            if !name.is_empty() {
                calls.push((name, args));
            }
            p = after + 1;
        }
        cuts.push((a, end));
    }
    (calls, cuts)
}

/// Estrae le chiamate scritte come testo. Restituisce il testo ripulito e le chiamate
/// (solo quelle verso strumenti in `allowed`; le altre vengono comunque tolte dal testo).
pub fn extract(text: &str, allowed: &[String]) -> (String, Vec<ToolCall>) {
    let s = norm(text);
    let lower = s.to_lowercase();
    if !["dsml", "<tool_call>", "<function=", "[tool_calls]", "<|tool_calls_begin|>"].iter().any(|m| lower.contains(m)) {
        return (text.to_string(), Vec::new());
    }
    let (mut calls, mut cuts) = parse_dsml(&s);
    let (c2, k2) = parse_others(&s);
    calls.extend(c2);
    cuts.extend(k2);
    // testo senza i blocchi (dal fondo, per non spostare gli indici)
    cuts.sort();
    let mut clean = String::new();
    let mut last = 0;
    for (a, b) in cuts {
        if a >= last {
            clean.push_str(&s[last..a]);
            last = b.max(a);
        } else {
            last = last.max(b);
        }
    }
    clean.push_str(s.get(last..).unwrap_or(""));
    let calls = calls
        .into_iter()
        .filter(|(n, _)| allowed.iter().any(|a| a == n))
        .enumerate()
        .map(|(i, (name, args))| ToolCall { id: format!("text_call_{i}"), name, arguments: args.to_string() })
        .collect();
    (clean.trim().to_string(), calls)
}

/// Nomi degli strumenti in una lista di definizioni (formato OpenAI).
pub fn tool_names(tools: &Value) -> Vec<String> {
    tools
        .as_array()
        .map(|a| a.iter().filter_map(|t| t.pointer("/function/name").and_then(Value::as_str).map(str::to_string)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        ["take_screenshot", "play_music", "open_target"].map(String::from).to_vec()
    }

    #[test]
    fn thinking_removed() {
        assert_eq!(strip_thinking("<think>uhm, l'utente…</think>\n[happy]\nCiao!"), "[happy]\nCiao!");
        assert_eq!(strip_thinking("ragiono…</think>[idle] Ok"), "[idle] Ok");
        assert_eq!(strip_thinking("[happy] Ciao <THINKING>x</THINKING>amico"), "[happy] Ciao amico");
        assert_eq!(strip_thinking("<think>solo pensiero troncato"), "");
        assert_eq!(strip_thinking("[happy] Nessun pensiero"), "[happy] Nessun pensiero");
    }

    #[test]
    fn dsml_from_screenshot() {
        // come nel bug segnalato: la chiamata finisce nel box di dialogo
        let t = "<｜DSML｜function_calls>\n<｜DSML｜invoke name=\"take_screenshot\">\n</｜DSML｜invoke>\n</｜DSML｜function_calls>";
        let (clean, calls) = extract(t, &allowed());
        assert_eq!(clean, "");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "take_screenshot");
        let t = "[happy]\nOk!<｜DSML｜function_calls><｜DSML｜invoke name=\"play_music\"><｜DSML｜parameter name=\"query\" string=\"true\">Megalovania</｜DSML｜parameter><｜DSML｜parameter name=\"enqueue\" string=\"false\">true</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜function_calls>";
        let (clean, calls) = extract(t, &allowed());
        assert_eq!(clean, "[happy]\nOk!");
        let a: Value = serde_json::from_str(&calls[0].arguments).unwrap();
        assert_eq!(a, json!({"query": "Megalovania", "enqueue": true}));
        // spazi e barre ASCII, chiamata troncata a metà
        let (clean, calls) = extract("< |DSML| function_calls>\n< |DSML| invoke name=\"take_screenshot\">", &allowed());
        assert_eq!((clean.as_str(), calls.len()), ("", 1));
    }

    #[test]
    fn other_formats() {
        let (c, calls) = extract("Subito! <tool_call>{\"name\": \"open_target\", \"arguments\": {\"target\": \"Spotify\"}}</tool_call>", &allowed());
        assert_eq!((c.as_str(), calls[0].name.as_str()), ("Subito!", "open_target"));
        let (_, calls) = extract("<function=play_music>{\"query\": \"Eva\"}</function>", &allowed());
        assert_eq!(calls[0].arguments, "{\"query\":\"Eva\"}");
        let (_, calls) = extract("[TOOL_CALLS][{\"name\": \"take_screenshot\", \"arguments\": {}}]", &allowed());
        assert_eq!(calls.len(), 1);
        let t = "<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>function<｜tool▁sep｜>play_music\n```json\n{\"query\": \"x\"}\n```<｜tool▁call▁end｜><｜tool▁calls▁end｜>";
        let (c, calls) = extract(t, &allowed());
        assert_eq!((c.as_str(), calls[0].name.as_str()), ("", "play_music"));
        // strumento non offerto: tolto dal testo ma non eseguito
        let (c, calls) = extract("Ecco <tool_call>{\"name\": \"run_shell\", \"arguments\": {}}</tool_call>", &allowed());
        assert_eq!((c.as_str(), calls.len()), ("Ecco", 0));
        // testo normale invariato
        assert_eq!(extract("[happy] ciao", &allowed()).0, "[happy] ciao");
    }
}
