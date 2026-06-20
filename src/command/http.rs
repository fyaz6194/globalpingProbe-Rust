pub mod parse {
    use serde::Serialize;
    use std::collections::HashMap;

    const HEADERS_SIZE_LIMIT: usize = 10_000;
    const TRUNCATION_MARK: &str = "...[truncated]";

    #[derive(Debug, Clone, PartialEq, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum HttpStatus {
        Finished,
        Failed,
    }

    #[derive(Debug, Clone, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HttpTimings {
        pub total: Option<u64>,
        pub dns: Option<u64>,
        pub tcp: Option<u64>,
        pub tls: Option<u64>,
        pub first_byte: Option<u64>,
        pub download: Option<u64>,
    }

    #[derive(Debug, Clone, Default, Serialize)]
    pub struct TlsSubject {
        #[serde(rename = "CN", skip_serializing_if = "Option::is_none")]
        pub cn: Option<String>,
        #[serde(rename = "alt", skip_serializing_if = "Option::is_none")]
        pub alt: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize)]
    pub struct TlsIssuer {
        #[serde(rename = "CN", skip_serializing_if = "Option::is_none")]
        pub cn: Option<String>,
        #[serde(rename = "O", skip_serializing_if = "Option::is_none")]
        pub o: Option<String>,
        #[serde(rename = "C", skip_serializing_if = "Option::is_none")]
        pub c: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TlsInfo {
        pub authorized: bool,
        pub protocol: Option<String>,
        pub cipher_name: Option<String>,
        pub created_at: Option<String>,
        pub expires_at: Option<String>,
        pub subject: TlsSubject,
        pub issuer: TlsIssuer,
        pub key_type: Option<String>,
        pub key_bits: Option<u32>,
        pub serial_number: Option<String>,
        pub fingerprint256: Option<String>,
        pub public_key: Option<String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ParsedHttp {
        pub status: HttpStatus,
        pub status_code: Option<u16>,
        pub status_code_name: Option<String>,
        pub resolved_address: Option<String>,
        #[serde(skip)]
        pub http_version: Option<String>,
        /// Lowercased header name → string or vec<string> if duplicate
        pub headers: HashMap<String, serde_json::Value>,
        pub raw_headers: Option<String>,
        pub raw_body: Option<String>,
        pub truncated: bool,
        pub tls: Option<TlsInfo>,
        pub timings: HttpTimings,
        pub raw_output: Option<String>,
    }

    // ── Header parsing ────────────────────────────────────────────────────────

    /// Parse the `-D` dump-header file content into (name, value) pairs.
    /// Skips the first line (HTTP status line) and empty lines.
    pub fn parse_header_file(raw: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for (i, line) in raw.lines().enumerate() {
            let line = line.trim_end_matches('\r').trim();
            if i == 0 || line.is_empty() {
                continue; // skip status line and blank lines
            }
            if let Some(colon) = line.find(':') {
                let name = line[..colon].trim().to_string();
                let value = line[colon + 1..].trim().to_string();
                pairs.push((name, value));
            }
        }
        pairs
    }

    /// Extract HTTP status text from the dump-header first line.
    /// e.g. "HTTP/1.1 200 OK" → Some("OK")
    pub fn parse_status_text(raw: &str) -> Option<String> {
        let first = raw.lines().next()?.trim_end_matches('\r');
        let parts: Vec<&str> = first.splitn(3, ' ').collect();
        if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            None
        }
    }

    // ── Header truncation (port of handlers/http/truncate-headers.ts) ─────────

    pub struct TruncateResult {
        pub truncated: bool,
        pub headers: Vec<(String, String)>,
    }

    fn pair_size(k: &str, v: &str) -> usize {
        k.len() + v.len() + 3 // ": " + "\n"
    }
    fn pair_min_size(k: &str, v: &str) -> usize {
        k.len() + v.len().min(TRUNCATION_MARK.len()) + 3
    }

    pub fn truncate_headers(pairs: Vec<(String, String)>) -> TruncateResult {
        if pairs.is_empty() {
            return TruncateResult { truncated: false, headers: pairs };
        }
        let size: usize = pairs.iter().map(|(k, v)| pair_size(k, v)).sum::<usize>().saturating_sub(1);
        let min_size: usize = pairs.iter().map(|(k, v)| pair_min_size(k, v)).sum::<usize>().saturating_sub(1);

        if size <= HEADERS_SIZE_LIMIT {
            return TruncateResult { truncated: false, headers: pairs };
        }

        let mut kept = pairs;
        let mut current_size = size;
        let mut current_min = min_size;

        // Drop headers phase: remove largest (by min size) until we can fit with truncation
        if current_min > HEADERS_SIZE_LIMIT {
            let mut indexed: Vec<(usize, usize)> = kept.iter()
                .enumerate()
                .map(|(i, (k, v))| (i, pair_min_size(k, v)))
                .collect();
            indexed.sort_unstable_by(|a, b| b.1.cmp(&a.1));

            let mut dropped = std::collections::HashSet::new();
            for (i, min) in &indexed {
                if current_min <= HEADERS_SIZE_LIMIT { break; }
                let (k, v) = &kept[*i];
                current_size -= pair_size(k, v);
                current_min -= min;
                dropped.insert(*i);
            }
            kept = kept.into_iter().enumerate()
                .filter(|(i, _)| !dropped.contains(i))
                .map(|(_, p)| p)
                .collect();
        }

        if current_size <= HEADERS_SIZE_LIMIT {
            return TruncateResult { truncated: true, headers: kept };
        }

        // Shrink values phase: find uniform cap L
        let mut sorted_lengths: Vec<usize> = kept.iter().map(|(_, v)| v.len()).collect();
        sorted_lengths.sort_unstable_by(|a, b| b.cmp(a));
        let values_size: usize = sorted_lengths.iter().sum();
        let value_budget = HEADERS_SIZE_LIMIT + values_size - current_size;
        let mut total = values_size;
        let mut cap = 0usize;

        for n in 1..=sorted_lengths.len() {
            let len = sorted_lengths[n - 1];
            let next_len = if n < sorted_lengths.len() { sorted_lengths[n] } else { 0 };
            let reduction = n * (len - next_len);
            if total.saturating_sub(reduction) <= value_budget {
                let diff = total.saturating_sub(value_budget);
                cap = len.saturating_sub((diff + n - 1) / n); // ceiling division keeps total ≤ budget
                break;
            }
            total = total.saturating_sub(reduction);
        }

        cap = cap.max(TRUNCATION_MARK.len());

        let headers = kept.into_iter().map(|(k, v)| {
            if v.len() > cap {
                let truncated_v = format!("{}{}", &v[..cap - TRUNCATION_MARK.len()], TRUNCATION_MARK);
                (k, truncated_v)
            } else {
                (k, v)
            }
        }).collect();

        TruncateResult { truncated: true, headers }
    }

    /// Collapse header pairs into the deduplicated map (lowercased keys, single or Vec values)
    pub fn dedup_headers(pairs: &[(String, String)]) -> HashMap<String, serde_json::Value> {
        let mut map: HashMap<String, serde_json::Value> = HashMap::new();
        for (k, v) in pairs {
            let lk = k.to_lowercase();
            match map.get_mut(&lk) {
                Some(serde_json::Value::Array(arr)) => {
                    arr.push(serde_json::Value::String(v.clone()));
                }
                Some(existing @ serde_json::Value::String(_)) => {
                    let prev = existing.clone();
                    *existing = serde_json::Value::Array(vec![prev, serde_json::Value::String(v.clone())]);
                }
                _ => {
                    map.insert(lk, serde_json::Value::String(v.clone()));
                }
            }
        }
        map
    }

    // ── TLS verbose parsing ───────────────────────────────────────────────────

    /// Parse curl -v stderr output for TLS connection details.
    /// Returns None if no TLS info found (plain HTTP).
    pub fn parse_tls_verbose(verbose: &str, ssl_verify_result: u32) -> Option<TlsInfo> {
        let mut protocol: Option<String> = None;
        let mut cipher_name: Option<String> = None;
        let mut created_at: Option<String> = None;
        let mut expires_at: Option<String> = None;
        let mut subject = TlsSubject::default();
        let mut issuer = TlsIssuer::default();
        let mut found_tls = false;

        for line in verbose.lines() {
            let line = line.trim();
            if !line.starts_with("* ") && !line.starts_with("*  ") {
                continue;
            }
            let content = line.trim_start_matches('*').trim();

            // SSL connection using TLSv1.3 / TLS_AES_256_GCM_SHA384
            if let Some(rest) = content.strip_prefix("SSL connection using ") {
                found_tls = true;
                let parts: Vec<&str> = rest.splitn(3, " / ").collect();
                protocol = Some(parts[0].trim().to_string());
                if parts.len() >= 2 {
                    cipher_name = Some(parts[1].trim().to_string());
                }
            }
            // start date: Nov  5 00:00:00 2024 GMT
            else if let Some(rest) = content.strip_prefix("start date: ") {
                created_at = parse_curl_date(rest.trim());
            }
            // expire date: Nov  4 23:59:59 2025 GMT
            else if let Some(rest) = content.strip_prefix("expire date: ") {
                expires_at = parse_curl_date(rest.trim());
            }
            // subject: CN=cloudflare.com
            else if let Some(rest) = content.strip_prefix("subject: ") {
                for part in rest.split(';') {
                    let part = part.trim();
                    if let Some(cn) = part.strip_prefix("CN=") {
                        subject.cn = Some(cn.trim().to_string());
                    }
                }
            }
            // issuer: C=US; O=DigiCert Inc; CN=DigiCert TLS RSA SHA256 2020 CA1
            else if let Some(rest) = content.strip_prefix("issuer: ") {
                for part in rest.split(';') {
                    let part = part.trim();
                    if let Some(cn) = part.strip_prefix("CN=") {
                        issuer.cn = Some(cn.trim().to_string());
                    } else if let Some(o) = part.strip_prefix("O=") {
                        issuer.o = Some(o.trim().to_string());
                    } else if let Some(c) = part.strip_prefix("C=") {
                        issuer.c = Some(c.trim().to_string());
                    }
                }
            }
            // subjectAltName: host "example.com" matched cert's "example.com"
            // subjectAltName: IP address "1.1.1.1"
            else if let Some(rest) = content.strip_prefix("subjectAltName: ") {
                subject.alt = Some(rest.trim().to_string());
            }
        }

        if !found_tls {
            return None;
        }

        Some(TlsInfo {
            authorized: ssl_verify_result == 0,
            protocol,
            cipher_name,
            created_at,
            expires_at,
            subject,
            issuer,
            key_type: None,
            key_bits: None,
            serial_number: None,
            fingerprint256: None,
            public_key: None,
        })
    }

    /// Parse curl's date format: "Nov  5 00:00:00 2024 GMT" → ISO 8601
    fn parse_curl_date(s: &str) -> Option<String> {
        // Format: "Mon DD HH:MM:SS YYYY GMT"
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 4 { return None; }
        // Use chrono to parse
        let date_str = format!("{} {} {} {}", parts[0], parts[1], parts[2], parts[3]);
        let formats = ["%b %e %H:%M:%S %Y", "%b %d %H:%M:%S %Y"];
        for fmt in &formats {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&date_str, fmt) {
                return Some(dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
            }
        }
        None
    }

    // ── rawOutput builder ─────────────────────────────────────────────────────

    pub fn build_raw_output(
        http_version: Option<&str>,
        status_code: Option<u16>,
        raw_headers: Option<&str>,
        raw_body: Option<&str>,
        method: &str,
    ) -> Option<String> {
        let version = http_version?;
        let code = status_code?;
        let headers = raw_headers.unwrap_or("");

        let base = format!("HTTP/{version} {code}\n{headers}");

        if method == "HEAD" || raw_body.map_or(true, |b| b.is_empty()) {
            Some(base)
        } else {
            Some(format!("{base}\n\n{}", raw_body.unwrap_or("")))
        }
    }

    // ── Unit tests ────────────────────────────────────────────────────────────

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_header_file_basic() {
            let raw = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nX-Foo: bar\r\n\r\n";
            let pairs = parse_header_file(raw);
            assert_eq!(pairs.len(), 2);
            assert_eq!(pairs[0].0, "Content-Type");
            assert_eq!(pairs[0].1, "text/html");
            assert_eq!(pairs[1].0, "X-Foo");
            assert_eq!(pairs[1].1, "bar");
        }

        #[test]
        fn parse_header_file_http2_lowercase() {
            let raw = "HTTP/2 200 \r\ncontent-type: application/json\r\n";
            let pairs = parse_header_file(raw);
            assert_eq!(pairs[0].0, "content-type");
        }

        #[test]
        fn parse_status_text_ok() {
            assert_eq!(parse_status_text("HTTP/1.1 200 OK\r\n"), Some("OK".into()));
            assert_eq!(parse_status_text("HTTP/2 200 \r\n"), Some("".into()));
            assert_eq!(parse_status_text("HTTP/1.1 404 Not Found"), Some("Not Found".into()));
        }

        #[test]
        fn dedup_headers_singles() {
            let pairs = vec![
                ("Content-Type".into(), "text/html".into()),
                ("X-Foo".into(), "bar".into()),
            ];
            let map = dedup_headers(&pairs);
            assert_eq!(map["content-type"], serde_json::Value::String("text/html".into()));
        }

        #[test]
        fn dedup_headers_duplicates_become_array() {
            let pairs = vec![
                ("Set-Cookie".into(), "a=1".into()),
                ("Set-Cookie".into(), "b=2".into()),
            ];
            let map = dedup_headers(&pairs);
            assert!(map["set-cookie"].is_array());
            let arr = map["set-cookie"].as_array().unwrap();
            assert_eq!(arr.len(), 2);
        }

        #[test]
        fn truncate_headers_no_truncation_needed() {
            let pairs = vec![("X-Foo".into(), "bar".into())];
            let res = truncate_headers(pairs);
            assert!(!res.truncated);
            assert_eq!(res.headers.len(), 1);
        }

        #[test]
        fn truncate_headers_shrinks_large_value() {
            // One header with a very large value
            let big_value = "x".repeat(11_000);
            let pairs = vec![("X-Big".into(), big_value)];
            let res = truncate_headers(pairs);
            assert!(res.truncated);
            let val = &res.headers[0].1;
            assert!(val.ends_with(TRUNCATION_MARK), "value should end with truncation mark: {}", &val[val.len().saturating_sub(20)..]);
            let raw_size = res.headers.iter().map(|(k, v)| k.len() + v.len() + 2).sum::<usize>();
            assert!(raw_size <= HEADERS_SIZE_LIMIT + 10, "output size should be within limit");
        }

        #[test]
        fn truncate_headers_drops_headers_when_too_many() {
            // Many headers each with a large min-size
            let pairs: Vec<(String, String)> = (0..200)
                .map(|i| (format!("X-Header-{i:03}"), "x".repeat(100)))
                .collect();
            let res = truncate_headers(pairs);
            assert!(res.truncated);
            let total: usize = res.headers.iter()
                .map(|(k, v)| k.len() + v.len() + 3)
                .sum::<usize>()
                .saturating_sub(1);
            assert!(total <= HEADERS_SIZE_LIMIT, "total after truncation: {total}");
        }

        #[test]
        fn parse_tls_verbose_extracts_fields() {
            let verbose = "\
* SSL connection using TLSv1.3 / TLS_AES_256_GCM_SHA384
* Server certificate:
*  subject: CN=cloudflare.com
*  start date: Oct  1 00:00:00 2024 GMT
*  expire date: Oct  1 23:59:59 2025 GMT
*  subjectAltName: host \"cloudflare.com\" matched cert's \"cloudflare.com\"
*  issuer: C=US; O=DigiCert Inc; CN=DigiCert TLS RSA SHA256 2020 CA1
";
            let tls = parse_tls_verbose(verbose, 0).expect("should parse TLS");
            assert!(tls.authorized);
            assert_eq!(tls.protocol.as_deref(), Some("TLSv1.3"));
            assert_eq!(tls.cipher_name.as_deref(), Some("TLS_AES_256_GCM_SHA384"));
            assert_eq!(tls.subject.cn.as_deref(), Some("cloudflare.com"));
            assert_eq!(tls.issuer.cn.as_deref(), Some("DigiCert TLS RSA SHA256 2020 CA1"));
            assert_eq!(tls.issuer.o.as_deref(), Some("DigiCert Inc"));
            assert_eq!(tls.issuer.c.as_deref(), Some("US"));
            assert!(tls.created_at.is_some());
            assert!(tls.expires_at.is_some());
        }

        #[test]
        fn parse_tls_verbose_returns_none_for_http() {
            let verbose = "* Connected to example.com (1.2.3.4) port 80";
            assert!(parse_tls_verbose(verbose, 0).is_none());
        }

        #[test]
        fn build_raw_output_head_no_body() {
            let out = build_raw_output(Some("1.1"), Some(200), Some("Content-Type: text/html"), None, "HEAD");
            assert!(out.is_some());
            let s = out.unwrap();
            assert!(s.starts_with("HTTP/1.1 200"));
            assert!(!s.contains("\n\n"));
        }

        #[test]
        fn build_raw_output_get_with_body() {
            let out = build_raw_output(Some("1.1"), Some(200), Some("Content-Type: text/html"), Some("<html>"), "GET");
            let s = out.unwrap();
            assert!(s.contains("\n\n<html>"));
        }
    }
}

// ── Imports ───────────────────────────────────────────────────────────────────

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tokio::fs;
use tokio::process::Command;
use tokio::time::{timeout, Duration, Instant};

use super::MeasurementCommand;
use crate::util::private_ip::is_ip_private;
use parse::{
    build_raw_output, dedup_headers, parse_header_file, parse_status_text,
    parse_tls_verbose, truncate_headers, HttpStatus, HttpTimings, ParsedHttp, TlsInfo,
};

// ── Options ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestOptions {
    #[serde(default = "default_method")]
    pub method: String,
    pub host: Option<String>,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpOptions {
    pub target: String,
    pub resolver: Option<String>,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub port: Option<u16>,
    #[serde(default = "default_ip_version")]
    pub ip_version: u8,
    #[serde(default)]
    pub in_progress_updates: bool,
    pub request: HttpRequestOptions,
}

fn default_method() -> String { "HEAD".into() }
fn default_path() -> String { "/".into() }
fn default_protocol() -> String { "HTTPS".into() }
fn default_ip_version() -> u8 { 4 }

// ── Validation ────────────────────────────────────────────────────────────────

fn validate(opts: &HttpOptions) -> Result<()> {
    if opts.ip_version != 4 && opts.ip_version != 6 {
        bail!("ipVersion must be 4 or 6");
    }
    let proto = opts.protocol.to_uppercase();
    if proto != "HTTP" && proto != "HTTPS" && proto != "HTTP2" {
        bail!("protocol must be HTTP, HTTPS, or HTTP2");
    }
    let method = opts.request.method.to_uppercase();
    if method != "GET" && method != "HEAD" && method != "OPTIONS" {
        bail!("method must be GET, HEAD, or OPTIONS");
    }
    // Private IP check if target is already an IP
    if let Ok(ip) = opts.target.parse() {
        if is_ip_private(ip) {
            bail!("Private IP ranges are not allowed");
        }
    }
    Ok(())
}

// ── DNS pre-resolution ────────────────────────────────────────────────────────

/// If `target` is a hostname, resolve it using dig and return (ip, dns_ms).
/// If it's already an IP address, return (target, 0) with dns_ms = None.
async fn resolve_target(
    target: &str,
    resolver: Option<&str>,
    ip_version: u8,
) -> Result<(String, Option<u64>)> {
    if target.parse::<std::net::IpAddr>().is_ok() {
        return Ok((target.to_string(), None)); // already an IP, no DNS needed
    }

    let query_type = if ip_version == 6 { "AAAA" } else { "A" };
    let mut args: Vec<String> = Vec::new();
    if let Some(r) = resolver {
        args.push(format!("@{r}"));
    }
    args.push(target.to_string());
    args.push(query_type.to_string());
    args.push("+short".into());
    args.push("+time=2".into());
    args.push("+tries=1".into());

    let start = Instant::now();
    let output = timeout(
        Duration::from_secs(5),
        Command::new("dig").args(&args).output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("DNS resolution timed out for {target}"))??;
    let dns_ms = start.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let ip = stdout
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with(';') && l.parse::<std::net::IpAddr>().is_ok())
        .map(|l| l.to_string())
        .ok_or_else(|| anyhow::anyhow!("DNS resolution returned no results for {target}"))?;

    // Check if resolved IP is private
    if let Ok(parsed_ip) = ip.parse() {
        if is_ip_private(parsed_ip) {
            bail!("Private IP ranges are not allowed");
        }
    }

    Ok((ip, Some(dns_ms)))
}

// ── URL builder ───────────────────────────────────────────────────────────────

fn build_url(opts: &HttpOptions, port: u16) -> String {
    let proto = opts.protocol.to_uppercase();
    let scheme = if proto == "HTTP" { "http" } else { "https" };
    let host = if opts.target.contains(':') {
        // IPv6 address needs brackets
        format!("[{}]", opts.target)
    } else {
        opts.target.clone()
    };
    let path = format!("/{}", opts.request.path.trim_start_matches('/'));
    let query = if opts.request.query.is_empty() {
        String::new()
    } else {
        format!("?{}", opts.request.query.trim_start_matches('?'))
    };
    format!("{}://{}:{}{}{}", scheme, host, port, path, query)
}

// ── curl arg builder ──────────────────────────────────────────────────────────

const CURL_WRITE_FMT: &str = concat!(
    r#"{"remote_ip":"%{remote_ip}","time_namelookup":%{time_namelookup},"#,
    r#""time_connect":%{time_connect},"time_appconnect":%{time_appconnect},"#,
    r#""time_starttransfer":%{time_starttransfer},"time_total":%{time_total},"#,
    r#""http_version":"%{http_version}","response_code":%{response_code},"#,
    r#""ssl_verify_result":%{ssl_verify_result}}"#,
);

#[derive(Debug, serde::Deserialize)]
struct CurlStats {
    remote_ip: String,
    time_namelookup: f64,
    time_connect: f64,
    time_appconnect: f64,
    time_starttransfer: f64,
    time_total: f64,
    http_version: String,
    response_code: u16,
    ssl_verify_result: u32,
}

fn build_curl_args(
    opts: &HttpOptions,
    url: &str,
    port: u16,
    resolved_ip: &str,
    headers_path: &str,
    body_path: &str,
) -> Vec<String> {
    let proto = opts.protocol.to_uppercase();
    let method = opts.request.method.to_uppercase();
    let host_header = opts
        .request
        .host
        .as_deref()
        .unwrap_or(&opts.target)
        .to_string();

    let mut args: Vec<String> = vec![
        "-sS".into(),          // silent, show errors
        "-k".into(),           // don't abort on TLS errors (but still report ssl_verify_result)
        "-v".into(),           // verbose → TLS info on stderr
        "--compressed".into(), // accept gzip/brotli
        "--max-time".into(), "10".into(),
        "-X".into(), method,
        format!("-{}", opts.ip_version),
        "-D".into(), headers_path.into(), // dump response headers to file
        "-o".into(), body_path.into(),    // write body to file
        "--write-out".into(), CURL_WRITE_FMT.into(),
        "-H".into(), format!("Host: {host_header}"),
        "-H".into(), "User-Agent: globalping probe (https://github.com/jsdelivr/globalping)".into(),
        "-H".into(), "Connection: close".into(),
        "-H".into(), "Accept-Encoding: gzip, deflate, br".into(),
    ];

    // Extra request headers
    for (k, v) in &opts.request.headers {
        args.push("-H".into());
        args.push(format!("{k}: {v}"));
    }

    // Protocol flag
    match proto.as_str() {
        "HTTP2" => { args.push("--http2".into()); }
        "HTTP" => { args.push("--http1.1".into()); }
        _ => {} // HTTPS uses curl default (HTTP/1.1 or HTTP/2 via ALPN)
    }

    // Force connection to pre-resolved IP (bypasses curl's DNS)
    if !opts.target.parse::<std::net::IpAddr>().is_ok() {
        args.push("--resolve".into());
        let resolve_ip = if resolved_ip.contains(':') {
            // IPv6: curl needs brackets in --resolve
            format!("[{resolved_ip}]")
        } else {
            resolved_ip.to_string()
        };
        args.push(format!("{}:{}:{}", opts.target, port, resolve_ip));
    }

    args.push(url.to_string());
    args
}

// ── TLS cert enrichment via openssl ──────────────────────────────────────────

/// After a successful HTTPS request, run `openssl s_client | openssl x509` to
/// extract the fields curl -v doesn't expose: fingerprint256, serialNumber,
/// keyType, keyBits, subject.alt, and the real authorized status.
async fn enrich_tls(tls: &mut TlsInfo, ip: &str, port: u16, servername: &str) {
    let connect_addr = if ip.contains(':') {
        format!("[{}]:{}", ip, port)
    } else {
        format!("{}:{}", ip, port)
    };
    // No SNI when target is already an IP address
    let sni = if servername.parse::<std::net::IpAddr>().is_ok() {
        "-noservername".to_string()
    } else {
        format!("-servername {}", servername)
    };

    // Write s_client output (PEM cert) to a temp file so we can run x509 on it
    // and also parse the verify-return-code line from stderr.
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let full_path = format!("/tmp/gp_sc_{id}.txt");
    let pem_path = format!("/tmp/gp_cert_{id}.pem");

    // openssl s_client writes EVERYTHING (connection info + PEM cert + "Verify return code")
    // to stdout. Capture it all, then extract the PEM block separately for x509.
    let sc_cmd = format!(
        "echo | openssl s_client -connect {connect_addr} {sni} 2>/dev/null >{full_path}"
    );
    let _ = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("sh").arg("-c").arg(&sc_cmd).output(),
    ).await;

    // Extract just the PEM block so openssl x509 can parse it cleanly
    let extract_cmd = format!(
        "sed -n '/-----BEGIN CERTIFICATE-----/,/-----END CERTIFICATE-----/p' {full_path} >{pem_path}"
    );
    let _ = Command::new("sh").arg("-c").arg(&extract_cmd).output().await;

    // Parse verify result from the full s_client output
    if let Ok(full_text) = fs::read_to_string(&full_path).await {
        for line in full_text.lines() {
            if line.contains("Verify return code:") {
                tls.authorized = line.contains("Verify return code: 0 (ok)");
                break;
            }
        }
    }
    let _ = fs::remove_file(&full_path).await;

    // Parse x509 cert details from the extracted PEM
    let x509_cmd = format!(
        "openssl x509 -noout -fingerprint -sha256 -serial -text -in {pem_path} 2>/dev/null"
    );
    let x509_out = tokio::time::timeout(
        Duration::from_secs(4),
        Command::new("sh").arg("-c").arg(&x509_cmd).output(),
    ).await.ok().and_then(|r| r.ok());

    let _ = fs::remove_file(&pem_path).await;

    let Some(x509) = x509_out else { return };
    let text = String::from_utf8_lossy(&x509.stdout);

    let mut next_is_san = false;
    for line in text.lines() {
        let t = line.trim();

        // OpenSSL 3.x outputs "sha256 Fingerprint=", 1.x outputs "SHA256 Fingerprint="
        if let Some(fp) = t.strip_prefix("sha256 Fingerprint=")
            .or_else(|| t.strip_prefix("SHA256 Fingerprint="))
        {
            tls.fingerprint256 = Some(fp.trim().to_string());
        }
        // "serial=AABB..." → "AA:BB:..."
        else if let Some(hex) = t.strip_prefix("serial=") {
            let hex = hex.trim().to_uppercase();
            let fmt: Vec<String> = hex.chars()
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|c| c.iter().collect())
                .collect();
            if !fmt.is_empty() {
                tls.serial_number = Some(fmt.join(":"));
            }
        }
        // "Public Key Algorithm: id-ecPublicKey" / "rsaEncryption"
        else if t.starts_with("Public Key Algorithm:") {
            if t.contains("ecPublicKey") || t.contains("id-ec") {
                tls.key_type = Some("EC".to_string());
            } else if t.contains("rsaEncryption") {
                tls.key_type = Some("RSA".to_string());
            }
        }
        // "Public-Key: (256 bit)"
        else if let Some(rest) = t.strip_prefix("Public-Key: (") {
            if let Some(bits_str) = rest.strip_suffix(" bit)") {
                if let Ok(bits) = bits_str.parse::<u32>() {
                    tls.key_bits = Some(bits);
                }
            }
        }
        // "X509v3 Subject Alternative Name:" → next non-empty line has the SANs
        else if t.starts_with("X509v3 Subject Alternative Name") {
            next_is_san = true;
        }
        else if next_is_san && !t.is_empty() {
            tls.subject.alt = Some(t.to_string());
            next_is_san = false;
        }
    }
}

// ── Runner ────────────────────────────────────────────────────────────────────

const BODY_LIMIT: usize = 10_000;

async fn run_http(opts: &HttpOptions) -> Result<ParsedHttp> {
    validate(opts)?;

    let proto = opts.protocol.to_uppercase();
    let is_https = proto != "HTTP";
    let port = opts.port.unwrap_or(if proto == "HTTP" { 80 } else { 443 });

    // Pre-resolve target hostname; check for private IPs
    let (resolved_ip, dns_ms) = resolve_target(&opts.target, opts.resolver.as_deref(), opts.ip_version).await?;

    let url = build_url(opts, port);

    // Temp files for headers and body
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let headers_path = format!("/tmp/gp_hdr_{id}.txt");
    let body_path = format!("/tmp/gp_body_{id}.txt");

    let args = build_curl_args(opts, &url, port, &resolved_ip, &headers_path, &body_path);

    let curl_result = timeout(
        Duration::from_secs(15),
        Command::new("curl").args(&args).output(),
    )
    .await;

    let curl_out = match curl_result {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            let _ = fs::remove_file(&headers_path).await;
            let _ = fs::remove_file(&body_path).await;
            bail!("curl failed to spawn: {e}");
        }
        Err(_) => {
            let _ = fs::remove_file(&headers_path).await;
            let _ = fs::remove_file(&body_path).await;
            bail!("curl timed out");
        }
    };

    let stats_str = String::from_utf8_lossy(&curl_out.stdout).trim().to_string();
    let verbose = String::from_utf8_lossy(&curl_out.stderr).to_string();

    // Read temp files then clean up
    let raw_headers_file = fs::read_to_string(&headers_path).await.unwrap_or_default();
    let raw_body_bytes = fs::read(&body_path).await.unwrap_or_default();
    let _ = fs::remove_file(&headers_path).await;
    let _ = fs::remove_file(&body_path).await;

    // If curl stats are missing/malformed, it's a hard failure
    let stats: CurlStats = match serde_json::from_str(&stats_str) {
        Ok(s) => s,
        Err(_) => {
            // Extract error message from curl stderr
            let err_msg = verbose
                .lines()
                .find(|l| l.contains("curl:") || l.contains("error") || l.starts_with("* "))
                .map(|l| l.trim_start_matches("* ").trim().to_string())
                .unwrap_or_else(|| "HTTP request failed".to_string());
            return Ok(failed_result(err_msg));
        }
    };

    if stats.response_code == 0 {
        let err = verbose.lines()
            .filter(|l| l.starts_with("* ") || l.contains("error"))
            .last()
            .map(|l| l.trim_start_matches("* ").trim().to_string())
            .unwrap_or_else(|| "HTTP request failed".to_string());
        return Ok(failed_result(err));
    }

    // Resolved address (curl may override if --resolve was used)
    let final_resolved_ip = if stats.remote_ip.is_empty() {
        resolved_ip.clone()
    } else {
        stats.remote_ip.clone()
    };

    // Post-resolution private IP check
    if let Ok(ip) = final_resolved_ip.parse() {
        if is_ip_private(ip) {
            return Ok(failed_result("Private IP ranges are not allowed.".into()));
        }
    }

    // Parse headers file
    let header_pairs_raw = parse_header_file(&raw_headers_file);
    let status_text = parse_status_text(&raw_headers_file);
    let truncate_res = truncate_headers(header_pairs_raw);
    let headers_map = dedup_headers(&truncate_res.headers);
    let raw_headers_str = truncate_res.headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    // Body: limit to BODY_LIMIT bytes
    let (raw_body, body_truncated) = if raw_body_bytes.len() > BODY_LIMIT {
        let truncated = String::from_utf8_lossy(&raw_body_bytes[..BODY_LIMIT]).to_string();
        (truncated, true)
    } else {
        (String::from_utf8_lossy(&raw_body_bytes).to_string(), false)
    };
    let truncated = truncate_res.truncated || body_truncated;

    // HTTP version from curl stats
    let http_version = match stats.http_version.as_str() {
        "2" | "2.0" => Some("2".to_string()),
        "1.0" => Some("1.0".to_string()),
        "1.1" => Some("1.1".to_string()),
        v if !v.is_empty() => Some(v.to_string()),
        _ => None,
    };

    // TLS info
    let mut tls: Option<TlsInfo> = if is_https {
        parse_tls_verbose(&verbose, stats.ssl_verify_result)
    } else {
        None
    };

    // Enrich TLS with fields curl -v doesn't expose (fingerprint256, serial, etc.)
    if let Some(ref mut t) = tls {
        let sni = opts.request.host.as_deref()
            .unwrap_or(&opts.target);
        enrich_tls(t, &final_resolved_ip, port, sni).await;
    }

    // Timings: curl reports cumulative seconds from start; convert to incremental ms
    // When --resolve is used, time_namelookup is near 0 (DNS was pre-done by us)
    let t_dns = dns_ms; // our measured DNS time (None if target was an IP)
    let t_lookup = (stats.time_namelookup * 1000.0).round() as u64;
    let t_tcp = ((stats.time_connect - stats.time_namelookup) * 1000.0).round() as u64;
    let t_tls = if is_https {
        let v = ((stats.time_appconnect - stats.time_connect) * 1000.0).round() as u64;
        Some(v)
    } else {
        None
    };
    let app_connect = if is_https { stats.time_appconnect } else { stats.time_connect };
    let t_first = ((stats.time_starttransfer - app_connect) * 1000.0).round() as u64;
    let t_download = ((stats.time_total - stats.time_starttransfer) * 1000.0).round() as u64;
    // Total = our DNS time + curl total
    let t_total = t_dns.unwrap_or(0) + (stats.time_total * 1000.0).round() as u64;

    let timings = HttpTimings {
        total: Some(t_total),
        dns: t_dns,
        tcp: Some(t_tcp + t_lookup), // include curl's namelookup if target was an IP
        tls: t_tls,
        first_byte: Some(t_first),
        download: Some(t_download),
    };

    let raw_body_opt = if raw_body.is_empty() { None } else { Some(raw_body.clone()) };
    let raw_output = build_raw_output(
        http_version.as_deref(),
        Some(stats.response_code),
        Some(&raw_headers_str),
        raw_body_opt.as_deref(),
        &opts.request.method,
    );

    Ok(ParsedHttp {
        status: HttpStatus::Finished,
        status_code: Some(stats.response_code),
        status_code_name: status_text,
        resolved_address: Some(final_resolved_ip),
        http_version,
        headers: headers_map,
        raw_headers: if raw_headers_str.is_empty() { None } else { Some(raw_headers_str) },
        raw_body: raw_body_opt,
        truncated,
        tls,
        timings,
        raw_output,
    })
}

fn failed_result(message: String) -> ParsedHttp {
    ParsedHttp {
        status: HttpStatus::Failed,
        status_code: None,
        status_code_name: None,
        resolved_address: None,
        http_version: None,
        headers: HashMap::new(),
        raw_headers: None,
        raw_body: None,
        truncated: false,
        tls: None,
        timings: HttpTimings::default(),
        raw_output: Some(message),
    }
}

// ── Command ───────────────────────────────────────────────────────────────────

pub struct HttpCommand;

#[async_trait::async_trait]
impl MeasurementCommand for HttpCommand {
    async fn run(&self, options: Value) -> Result<Value> {
        let opts: HttpOptions = serde_json::from_value(options)?;
        let result = run_http(&opts).await?;
        Ok(serde_json::to_value(result)?)
    }
}

// ── Public helper for integration tests ───────────────────────────────────────

pub async fn run_measurement(
    target: &str,
    protocol: &str,
    method: &str,
    path: &str,
    resolver: Option<&str>,
    ip_version: u8,
) -> Result<ParsedHttp> {
    let opts = HttpOptions {
        target: target.to_string(),
        resolver: resolver.map(String::from),
        protocol: protocol.to_string(),
        port: None,
        ip_version,
        in_progress_updates: false,
        request: HttpRequestOptions {
            method: method.to_string(),
            host: None,
            path: path.to_string(),
            query: String::new(),
            headers: HashMap::new(),
        },
    };
    run_http(&opts).await
}
