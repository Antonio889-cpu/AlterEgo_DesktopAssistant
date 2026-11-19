//! Rizzo Flow per le decisioni rapide in Minecraft (come `saving::rizzo`: `POST /v1/systemone`, domande
//! tipizzate noul/choice/score in una sola chiamata, ~50 ms, zero token generati). Si usa solo se Rizzo Flow
//! è il router dell'app (Impostazioni → Risparmio token) e l'opzione di Minecraft è accesa; altrimenti, o se il
//! server non risponde, decide l'euristica di chi chiama.
//! Domande in batch: tattica del round dopo + "copio il suo stile?" + "gli chiedo come preferisce?";
//! una risposta in chat a una domanda di Alter Ego: sì/no + scelta.

use serde_json::{json, Map, Value};
use std::time::Duration;

/// Una domanda per Rizzo Flow.
#[derive(Debug, Clone, PartialEq)]
pub enum Q {
    /// Sì o no (probabilità del sì).
    Noul(&'static str, String),
    /// Una tra più opzioni (nome → descrizione).
    Choice(&'static str, String, Vec<(String, String)>),
}

/// Corpo della richiesta (stesso formato di `saving::rizzo`).
pub fn body(state: Value, qs: &[Q]) -> Value {
    let mut questions = Map::new();
    for q in qs {
        match q {
            Q::Noul(k, ins) => {
                questions.insert((*k).into(), json!({ "type": "noul", "instructions": ins }));
            }
            Q::Choice(k, ins, opts) => {
                let o: Map<String, Value> = opts.iter().map(|(n, d)| (n.clone(), Value::String(d.clone()))).collect();
                questions.insert((*k).into(), json!({ "type": "choice", "instructions": ins, "options": o }));
            }
        }
    }
    json!({ "model": "rizzo-latest", "state": state, "questions": questions })
}

/// Fa le domande. None se Rizzo Flow non risponde (si usa l'euristica).
pub async fn ask(http: &reqwest::Client, url: &str, state: Value, qs: &[Q]) -> Option<Value> {
    let endpoint = format!("{}/v1/systemone", url.trim().trim_end_matches('/'));
    let resp = http.post(endpoint).timeout(Duration::from_millis(1500)).json(&body(state, qs)).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    v.get("answers").is_some().then_some(v)
}

/// Probabilità del sì per la domanda `key`.
pub fn noul(v: &Value, key: &str) -> Option<f64> {
    v.pointer(&format!("/answers/{key}/noul")).and_then(Value::as_f64)
}

/// Scelta e sua probabilità (probabilità come oggetto o come elenco, come in `saving::parse_rizzo`).
pub fn choice(v: &Value, key: &str) -> Option<(String, f64)> {
    let a = v.pointer(&format!("/answers/{key}"))?;
    let pick = a.get("choice").and_then(Value::as_str).map(str::to_string);
    let probs: Vec<(String, f64)> = match a.get("probabilities") {
        Some(Value::Object(m)) => m.iter().filter_map(|(k, p)| p.as_f64().map(|p| (k.clone(), p))).collect(),
        Some(Value::Array(l)) => l
            .iter()
            .filter_map(|x| Some((x.get("option")?.as_str()?.to_string(), x.get("p").or(x.get("probability"))?.as_f64()?)))
            .collect(),
        _ => Vec::new(),
    };
    let best = probs.iter().cloned().max_by(|a, b| a.1.total_cmp(&b.1));
    match (pick, best) {
        (Some(c), _) => {
            let p = probs.iter().find(|(k, _)| *k == c).map(|x| x.1).unwrap_or(1.0);
            Some((c, p))
        }
        (None, Some(b)) => Some(b),
        _ => None,
    }
}

/// Domande tra un round e l'altro: tattica (tra quelle possibili), copiare lo stile, chiedere al giocatore.
pub fn tactic_questions(options: &[(String, String)]) -> Vec<Q> {
    vec![
        Q::Choice(
            "tactic",
            "Minecraft PvP duel between friends, best of a few rounds. Pick the tactic for Alter Ego's next round that most likely wins against this opponent's style, using the per-tactic results so far (reward 0-1) and classic counters (shield against jump crits, spacing against close spammers, rush against bow users, axe against shields).".into(),
            options.to_vec(),
        ),
        Q::Noul("mimic", "Did the opponent win the last round with a style worth copying (their distance and strafing)?".into()),
        Q::Noul("ask_style", "Is the opponent's style still unclear after this round, so that asking them a short question in chat would help?".into()),
    ]
}

/// Domande per capire una risposta del giocatore a una domanda di Alter Ego.
pub fn answer_questions(question: &str, options: &[(String, String)]) -> Vec<Q> {
    let mut v = vec![Q::Noul("yes", format!("Alter Ego asked the player in Minecraft chat: \"{question}\". Does the player's reply mean YES / agree / go ahead?"))];
    if !options.is_empty() {
        v.push(Q::Choice("pick", format!("Which option does the player's reply to \"{question}\" choose?"), options.to_vec()));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_body_and_answers() {
        let opts = vec![("rush".to_string(), "stay close".to_string()), ("spacing".to_string(), "keep reach".to_string())];
        let b = body(json!({ "opponent": "Makoto" }), &tactic_questions(&opts));
        assert_eq!(b["model"], "rizzo-latest");
        assert_eq!(b["questions"]["tactic"]["type"], "choice");
        assert_eq!(b["questions"]["tactic"]["options"]["spacing"], "keep reach");
        assert_eq!(b["questions"]["mimic"]["type"], "noul");
        assert_eq!(b["questions"].as_object().unwrap().len(), 3, "tre domande in una chiamata");
        let v = json!({ "answers": { "tactic": { "choice": "spacing", "probabilities": { "spacing": 0.7, "rush": 0.3 } }, "mimic": { "noul": 0.8 } } });
        assert_eq!(choice(&v, "tactic"), Some(("spacing".into(), 0.7)));
        assert_eq!(noul(&v, "mimic"), Some(0.8));
        assert_eq!(noul(&v, "ask_style"), None);
        // solo probabilità, in elenco
        let v = json!({ "answers": { "pick": { "probabilities": [{ "option": "here", "p": 0.2 }, { "option": "base", "p": 0.8 }] } } });
        assert_eq!(choice(&v, "pick"), Some(("base".into(), 0.8)));
        let q = answer_questions("posso rompere?", &[]);
        assert!(matches!(&q[0], Q::Noul("yes", t) if t.contains("posso rompere?")));
        assert_eq!(q.len(), 1);
    }
}
