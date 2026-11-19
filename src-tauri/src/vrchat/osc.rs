//! OSC 1.0 minimo (quello che usa VRChat): messaggi con argomenti int, float, stringa, bool
//! e bundle in arrivo. Niente crate esterni: sono poche righe e si provano facilmente.

/// Argomento di un messaggio OSC.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
    Bool(bool),
}

impl Arg {
    /// Valore numerico (bool → 0/1): i parametri dell'avatar possono essere di tutti e tre i tipi.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Arg::Int(i) => Some(*i as f32),
            Arg::Float(f) => Some(*f),
            Arg::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Arg::Str(_) => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Arg::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Msg {
    pub addr: String,
    pub args: Vec<Arg>,
}

impl Msg {
    pub fn new(addr: &str, args: Vec<Arg>) -> Self {
        Self { addr: addr.to_string(), args }
    }
}

fn pad(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn put_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
    pad(buf);
}

/// Codifica un messaggio.
pub fn encode(m: &Msg) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    put_str(&mut buf, &m.addr);
    let mut tags = String::from(",");
    for a in &m.args {
        tags.push(match a {
            Arg::Int(_) => 'i',
            Arg::Float(_) => 'f',
            Arg::Str(_) => 's',
            Arg::Bool(true) => 'T',
            Arg::Bool(false) => 'F',
        });
    }
    put_str(&mut buf, &tags);
    for a in &m.args {
        match a {
            Arg::Int(i) => buf.extend_from_slice(&i.to_be_bytes()),
            Arg::Float(f) => buf.extend_from_slice(&f.to_be_bytes()),
            Arg::Str(s) => put_str(&mut buf, s),
            Arg::Bool(_) => {}
        }
    }
    buf
}

fn read_str(b: &[u8], p: &mut usize) -> Option<String> {
    let rest = b.get(*p..)?;
    let end = rest.iter().position(|&c| c == 0)?;
    let s = String::from_utf8_lossy(&rest[..end]).into_owned();
    *p += (end + 4) & !3; // stringa + zero, allineata a 4
    (*p <= b.len()).then_some(s)
}

fn read4(b: &[u8], p: &mut usize) -> Option<[u8; 4]> {
    let v: [u8; 4] = b.get(*p..*p + 4)?.try_into().ok()?;
    *p += 4;
    Some(v)
}

fn read8(b: &[u8], p: &mut usize) -> Option<[u8; 8]> {
    let v: [u8; 8] = b.get(*p..*p + 8)?.try_into().ok()?;
    *p += 8;
    Some(v)
}

fn decode_msg(b: &[u8]) -> Option<Msg> {
    let mut p = 0;
    let addr = read_str(b, &mut p)?;
    if !addr.starts_with('/') {
        return None;
    }
    if p >= b.len() {
        return Some(Msg { addr, args: Vec::new() }); // senza type tag (vecchio OSC)
    }
    let tags = read_str(b, &mut p)?;
    let mut args = Vec::new();
    for t in tags.strip_prefix(',')?.chars() {
        args.push(match t {
            'i' => Arg::Int(i32::from_be_bytes(read4(b, &mut p)?)),
            'f' => Arg::Float(f32::from_be_bytes(read4(b, &mut p)?)),
            's' => Arg::Str(read_str(b, &mut p)?),
            'T' => Arg::Bool(true),
            'F' => Arg::Bool(false),
            'N' | 'I' => continue,
            'd' => Arg::Float(f64::from_be_bytes(read8(b, &mut p)?) as f32),
            'h' | 't' => Arg::Int(i64::from_be_bytes(read8(b, &mut p)?) as i32),
            _ => return None, // tipo sconosciuto: il resto non si può leggere
        });
    }
    Some(Msg { addr, args })
}

/// Decodifica un pacchetto (messaggio o bundle, anche annidati). Pacchetti rotti: elenco vuoto.
pub fn decode(b: &[u8]) -> Vec<Msg> {
    let mut out = Vec::new();
    decode_into(b, &mut out, 0);
    out
}

fn decode_into(b: &[u8], out: &mut Vec<Msg>, depth: u8) {
    if depth > 4 || b.is_empty() {
        return;
    }
    if b.starts_with(b"#bundle\0") {
        let mut p = 16; // "#bundle\0" + time tag
        while p + 4 <= b.len() {
            let n = u32::from_be_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]) as usize;
            p += 4;
            let Some(part) = b.get(p..p + n) else { return };
            decode_into(part, out, depth + 1);
            p += n;
        }
        return;
    }
    if let Some(m) = decode_msg(b) {
        out.push(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_matches_spec() {
        // esempio della specifica OSC 1.0: "/oscillator/4/frequency" ,f 440.0
        let m = Msg::new("/oscillator/4/frequency", vec![Arg::Float(440.0)]);
        let b = encode(&m);
        assert_eq!(b.len(), 32);
        assert_eq!(&b[..24], b"/oscillator/4/frequency\0");
        assert_eq!(&b[24..28], b",f\0\0");
        assert_eq!(&b[28..], &[0x43, 0xdc, 0x00, 0x00]);
        // chatbox: stringa + due bool (nessun byte per T/F)
        let c = encode(&Msg::new("/chatbox/input", vec![Arg::Str("ciao".into()), Arg::Bool(true), Arg::Bool(false)]));
        assert_eq!(&c[..16], b"/chatbox/input\0\0");
        assert_eq!(&c[16..24], b",sTF\0\0\0\0");
        assert_eq!(&c[24..], b"ciao\0\0\0\0");
        assert_eq!(c.len() % 4, 0);
    }

    #[test]
    fn roundtrip_and_bundles() {
        let msgs = [
            Msg::new("/input/MoveForward", vec![Arg::Int(1)]),
            Msg::new("/avatar/parameters/VelocityZ", vec![Arg::Float(-1.5)]),
            Msg::new("/avatar/change", vec![Arg::Str("avtr_0123-abcd".into())]),
            Msg::new("/avatar/parameters/HeadPat", vec![Arg::Bool(true)]),
            Msg::new("/x", vec![]),
        ];
        for m in &msgs {
            assert_eq!(decode(&encode(m)), vec![m.clone()], "{m:?}");
        }
        // bundle con due messaggi (come li manda VRChat a volte)
        let mut b = b"#bundle\0".to_vec();
        b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        for m in &msgs[..2] {
            let e = encode(m);
            b.extend_from_slice(&(e.len() as u32).to_be_bytes());
            b.extend_from_slice(&e);
        }
        assert_eq!(decode(&b), msgs[..2].to_vec());
        // pacchetti rotti: niente panico, niente messaggi
        assert!(decode(b"").is_empty());
        assert!(decode(b"garbage").is_empty());
        assert!(decode(&encode(&msgs[1])[..30]).is_empty());
        let mut trunc = b.clone();
        trunc.truncate(30);
        assert!(decode(&trunc).len() <= 1);
        assert_eq!(Arg::Bool(true).as_f32(), Some(1.0));
        assert_eq!(Arg::Int(3).as_f32(), Some(3.0));
    }
}
