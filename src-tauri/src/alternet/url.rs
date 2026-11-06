// Validazione dell'indirizzo del server Alter Net (dal client Alter Net, test inclusi).

/// Only plain http(s)://host[:port]/ addresses: the value ends up in the start page's script.
pub fn clean_url(s: &str) -> Option<String> {
    let s = s.trim();
    let rest = s.strip_prefix("http://").or_else(|| s.strip_prefix("https://"))?;
    let host = rest.trim_end_matches('/');
    let ok = !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    ok.then(|| format!("{}{}/", &s[..s.len() - rest.len()], host))
}

/// Is an Alter Net server listening there? Plain TCP connect: works from Rust where a page's fetch() to a
/// private address may be blocked by the WebView.
pub fn reachable(url: &str) -> bool {
    use std::{net::{TcpStream, ToSocketAddrs}, time::Duration};
    let rest = url.split("://").nth(1).unwrap_or("").trim_end_matches('/');
    let has_port = rest.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) && !rest.ends_with(']');
    let addr = if has_port { rest.to_string() } else { format!("{rest}:{}", if url.starts_with("https") { 443 } else { 80 }) };
    addr.to_socket_addrs().ok().and_then(|mut a| a.next())
        .is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{clean_url, reachable};
    #[test]
    fn reach() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(reachable(&format!("http://127.0.0.1:{port}/")));
        drop(l);
        assert!(!reachable(&format!("http://127.0.0.1:{port}/")));
        assert!(!reachable("http://nonexistent.invalid:8080/"));
    }
    #[test]
    fn urls() {
        assert_eq!(clean_url("http://192.168.1.18:8080").as_deref(), Some("http://192.168.1.18:8080/"));
        assert_eq!(clean_url(" https://alter.example/ \n").as_deref(), Some("https://alter.example/"));
        assert_eq!(clean_url("http://[::1]:8080/").as_deref(), Some("http://[::1]:8080/"));
        for bad in ["ftp://x", "http://", "http://a/b", "http://a\"';alert(1)//", "javascript:alert(1)", "http://a b"] {
            assert_eq!(clean_url(bad), None, "{bad}");
        }
    }
}
