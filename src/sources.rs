//! Logo sources. All of these are free and need no API key:
//! - Wikidata "logo image" (P154) for matching companies/products
//! - Simple Icons brand SVGs
//! - Wikimedia Commons file search
//! - Favicon of the official website (from Wikidata P856), as a last resort

use std::collections::HashSet;
use std::time::Duration;

use crate::cache;
use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::RETRY_AFTER;
use serde_json::Value;

const USER_AGENT: &str = concat!(
    "magpie/",
    env!("CARGO_PKG_VERSION"),
    " (desktop logo, icon and photo search tool; https://github.com/mgwoolf/magpie)"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Svg,
    Png,
}

impl Kind {
    pub fn ext(self) -> &'static str {
        match self {
            Kind::Svg => "svg",
            Kind::Png => "png",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub source: &'static str,
    pub title: String,
    pub url: String,
    pub kind: Kind,
    /// Iconify icon-set prefix (e.g. `lucide`), for icons only.
    pub set: Option<String>,
}

/// An Iconify icon set ("family"), e.g. Lucide or Material Symbols.
#[derive(Clone, Debug)]
pub struct IconSet {
    pub prefix: String,
    pub name: String,
    pub total: u64,
    pub license: String,
    pub category: String,
}

pub const ICONIFY: &str = "Iconify";

impl Candidate {
    /// Single-colour icons drawn with `currentColor`, which can be recoloured.
    pub fn tintable(&self) -> bool {
        self.source == ICONIFY
    }
}

/// Replace `currentColor` in an SVG with a hex colour such as `#1a73e8`.
pub fn tint(svg: &[u8], hex: &str) -> Vec<u8> {
    String::from_utf8_lossy(svg).replace("currentColor", hex).into_bytes()
}

pub fn client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        // Fail fast when a host is unreachable, but give slow APIs (Openverse) time to answer.
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

/// Send a request, retrying when the server throttles us (Wikimedia does this
/// readily) and honouring its `Retry-After` header.
pub(crate) fn send(build: impl Fn() -> RequestBuilder) -> Result<Response, String> {
    const ATTEMPTS: u32 = 4;
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        let last_try = attempt + 1 == ATTEMPTS;
        match build().send() {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                let status = resp.status();
                last = format!("HTTP {status}");
                if last_try || (status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error()) {
                    break;
                }
                let wait = resp
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(10);
                std::thread::sleep(Duration::from_secs(wait));
            }
            Err(e) => {
                let timed_out = e.is_timeout();
                last = describe(e);
                // A server that timed out is likely still overloaded: one more try, not three.
                if last_try || (timed_out && attempt >= 1) {
                    break;
                }
                // Dropped connections and DNS hiccups are usually gone a moment later.
                std::thread::sleep(Duration::from_millis(500 << attempt));
            }
        }
    }
    Err(last)
}

/// Say *why* a request failed. reqwest's own message is just "error sending request for url
/// (…)"; the useful part (timeout, DNS, connection reset, TLS) is in its source chain.
fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        return "the server took too long to respond (timed out)".into();
    }
    let mut causes: Vec<String> = Vec::new();
    let mut source = std::error::Error::source(&e);
    while let Some(err) = source {
        let msg = err.to_string();
        if !msg.is_empty() && !causes.iter().any(|c| c.contains(&msg) || msg.contains(c.as_str())) {
            causes.push(msg);
        }
        source = err.source();
    }
    let prefix = if e.is_connect() {
        "could not connect"
    } else {
        "request failed"
    };
    match causes.last() {
        Some(cause) => format!("{prefix}: {cause}"),
        None => format!("{prefix}: {}", e.without_url()),
    }
}

/// Download without caching, for large files such as full-size photos.
pub fn download_uncached(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    send(|| client.get(url))?
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| e.to_string())
}

pub fn download(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    if let Some(hit) = cache::get(url, cache::FILE_TTL) {
        return Ok(hit);
    }
    let bytes = send(|| client.get(url))?
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| e.to_string())?;
    cache::put(url, &bytes);
    Ok(bytes)
}

/// Query every source and return candidates, best guesses first.
pub fn search(client: &Client, query: &str) -> Result<Vec<Candidate>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut errors = Vec::new();

    let mut websites = Vec::new();
    match wikidata(client, query) {
        Ok((logos, sites)) => {
            out.extend(logos);
            websites = sites;
        }
        Err(e) => errors.push(format!("Wikidata: {e}")),
    }

    if let Some(c) = simple_icons(client, query) {
        out.push(c);
    }

    match commons(client, query) {
        Ok(found) => out.extend(found),
        Err(e) => errors.push(format!("Commons: {e}")),
    }

    for host in websites.iter().take(2) {
        out.push(Candidate {
            source: "Favicon",
            title: host.clone(),
            url: format!("https://www.google.com/s2/favicons?domain={host}&sz=256"),
            kind: Kind::Png,
            set: None,
        });
    }

    let mut seen = HashSet::new();
    out.retain(|c| seen.insert(dedupe_key(&c.url)));

    if out.is_empty() && !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(out)
}

/// Wikimedia files are reachable via several URLs; key them by file name.
fn dedupe_key(url: &str) -> String {
    if url.contains("wikimedia.org")
        && let Some(name) = url.rsplit('/').next()
    {
        return name.to_lowercase();
    }
    url.to_string()
}

/// Search the Iconify index of open-source icon sets (Material, Lucide, Tabler,
/// Phosphor, Font Awesome, …). Results are interleaved across sets for variety.
/// `set` restricts results to one icon family (an Iconify prefix such as `lucide`).
pub fn search_icons(client: &Client, query: &str, set: Option<&str>) -> Result<Vec<Candidate>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut params = vec![("query", query), ("limit", "96")];
    if let Some(prefix) = set {
        params.push(("prefixes", prefix));
    }
    let v = get_json(client, "https://api.iconify.design/search", &params)?;
    let collections = &v["collections"];

    // Group by icon set, keeping Iconify's ranking within each set.
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for full in v["icons"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let Some((prefix, name)) = full.split_once(':') else {
            continue;
        };
        match groups.iter_mut().find(|(p, _)| p == prefix) {
            Some((_, names)) => names.push(name.to_string()),
            None => groups.push((prefix.to_string(), vec![name.to_string()])),
        }
    }

    let mut out = Vec::new();
    let longest = groups.iter().map(|(_, n)| n.len()).max().unwrap_or(0);
    for round in 0..longest {
        for (prefix, names) in &groups {
            let Some(name) = names.get(round) else { continue };
            let set = &collections[prefix];
            let set_name = set["name"].as_str().unwrap_or(prefix);
            let title = match set["license"]["title"].as_str() {
                Some(lic) => format!("{name} — {set_name} ({lic})"),
                None => format!("{name} — {set_name}"),
            };
            out.push(Candidate {
                source: ICONIFY,
                title,
                // height=auto gives the icon its natural viewBox size instead of 1em.
                url: format!("https://api.iconify.design/{prefix}/{name}.svg?height=auto"),
                kind: Kind::Svg,
                set: Some(prefix.clone()),
            });
        }
    }
    Ok(out)
}

/// All icon families Iconify offers, sorted by name.
pub fn icon_sets(client: &Client) -> Result<Vec<IconSet>, String> {
    let v = get_json(client, "https://api.iconify.design/collections", &[])?;
    let mut sets: Vec<IconSet> = v
        .as_object()
        .ok_or("unexpected response")?
        .iter()
        .filter(|(_, c)| !c["hidden"].as_bool().unwrap_or(false))
        .map(|(prefix, c)| IconSet {
            prefix: prefix.clone(),
            name: c["name"].as_str().unwrap_or(prefix).to_string(),
            total: c["total"].as_u64().unwrap_or(0),
            license: c["license"]["title"].as_str().unwrap_or("").to_string(),
            category: c["category"].as_str().unwrap_or("").to_string(),
        })
        .collect();
    sets.sort_by_key(|s| s.name.to_lowercase());
    Ok(sets)
}

fn get_json(client: &Client, url: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    get_json_with(client, url, params, None)
}

/// Cached JSON GET, optionally with one extra header (e.g. an API key).
pub(crate) fn get_json_with(
    client: &Client,
    url: &str,
    params: &[(&str, &str)],
    header: Option<(&str, &str)>,
) -> Result<Value, String> {
    let key = format!("json:{url}?{params:?}");
    if let Some(v) = cache::get(&key, cache::API_TTL).and_then(|b| serde_json::from_slice(&b).ok()) {
        return Ok(v);
    }
    let body = send(|| {
        let req = client.get(url).query(params);
        match header {
            Some((name, value)) => req.header(name, value),
            None => req,
        }
    })?
    .bytes()
    .map_err(|e| e.to_string())?;
    let v = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    cache::put(&key, &body);
    Ok(v)
}

/// Like [`get_json_with`], but if the request fails, fall back to a cached response of any age.
/// The flag is true when the result came from that stale cache.
pub(crate) fn get_json_or_stale(
    client: &Client,
    url: &str,
    params: &[(&str, &str)],
    header: Option<(&str, &str)>,
) -> Result<(Value, bool), String> {
    match get_json_with(client, url, params, header) {
        Ok(v) => Ok((v, false)),
        Err(e) => {
            let key = format!("json:{url}?{params:?}");
            cache::get(&key, Duration::MAX)
                .and_then(|b| serde_json::from_slice(&b).ok())
                .map(|v| (v, true))
                .ok_or(e)
        }
    }
}

fn kind_from_name(name: &str) -> Option<Kind> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".svg") {
        Some(Kind::Svg)
    } else if lower.ends_with(".png") {
        Some(Kind::Png)
    } else {
        None
    }
}

fn commons_file_url(filename: &str) -> String {
    let name = filename.replace(' ', "_");
    let mut url = reqwest::Url::parse("https://commons.wikimedia.org/wiki/Special:FilePath/").expect("static url");
    url.path_segments_mut().expect("base url").pop().push(&name);
    url.to_string()
}

/// Returns (logo candidates, official website hosts).
fn wikidata(client: &Client, query: &str) -> Result<(Vec<Candidate>, Vec<String>), String> {
    let api = "https://www.wikidata.org/w/api.php";
    let found = get_json(
        client,
        api,
        &[
            ("action", "wbsearchentities"),
            ("search", query),
            ("language", "en"),
            ("type", "item"),
            ("limit", "7"),
            ("format", "json"),
        ],
    )?;

    let hits: Vec<(String, String)> = found["search"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| {
                    let id = h["id"].as_str()?.to_string();
                    let label = h["label"].as_str().unwrap_or(&id).to_string();
                    let desc = h["description"].as_str().unwrap_or("");
                    let title = if desc.is_empty() {
                        label
                    } else {
                        format!("{label} — {desc}")
                    };
                    Some((id, title))
                })
                .collect()
        })
        .unwrap_or_default();

    if hits.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let ids = hits.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join("|");
    let entities = get_json(
        client,
        api,
        &[
            ("action", "wbgetentities"),
            ("ids", &ids),
            ("props", "claims"),
            ("format", "json"),
        ],
    )?;

    let mut logos = Vec::new();
    let mut sites = Vec::new();
    for (id, title) in &hits {
        let claims = &entities["entities"][id]["claims"];
        // Current logo is usually marked "preferred"; otherwise later entries are newer.
        let mut logo_claims: Vec<&Value> = claims["P154"].as_array().into_iter().flatten().collect();
        logo_claims.reverse();
        logo_claims.sort_by_key(|c| match c["rank"].as_str() {
            Some("preferred") => 0,
            Some("deprecated") => 2,
            _ => 1,
        });
        for claim in logo_claims {
            let Some(file) = claim["mainsnak"]["datavalue"]["value"].as_str() else {
                continue;
            };
            let Some(kind) = kind_from_name(file) else {
                continue;
            };
            logos.push(Candidate {
                source: "Wikidata",
                title: title.clone(),
                url: commons_file_url(file),
                kind,
                set: None,
            });
        }
        if let Some(site) = claims["P856"][0]["mainsnak"]["datavalue"]["value"].as_str()
            && let Some(host) = reqwest::Url::parse(site)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
            && !sites.contains(&host)
        {
            sites.push(host);
        }
    }
    Ok((logos, sites))
}

/// Simple Icons slug rules (approximation of their `titleToSlug`).
fn simple_icons_slug(name: &str) -> String {
    let mut s = String::new();
    for ch in name.to_lowercase().chars() {
        match ch {
            '+' => s.push_str("plus"),
            '.' => s.push_str("dot"),
            '&' => s.push_str("and"),
            'à'..='å' => s.push('a'),
            'è'..='ë' => s.push('e'),
            'ì'..='ï' => s.push('i'),
            'ò'..='ö' | 'ø' => s.push('o'),
            'ù'..='ü' => s.push('u'),
            'ç' => s.push('c'),
            'ñ' => s.push('n'),
            'ß' => s.push_str("ss"),
            c if c.is_ascii_alphanumeric() => s.push(c),
            _ => {}
        }
    }
    s
}

fn simple_icons(client: &Client, query: &str) -> Option<Candidate> {
    let slug = simple_icons_slug(query);
    if slug.is_empty() {
        return None;
    }
    let url = format!("https://cdn.simpleicons.org/{slug}");
    let key = format!("exists:{url}");
    let ok = match cache::get(&key, cache::API_TTL) {
        Some(hit) => hit == b"1",
        None => match send(|| client.head(&url)) {
            Ok(_) => {
                cache::put(&key, b"1");
                true
            }
            // Only remember a definite "no such icon", not a network failure.
            Err(e) if e.starts_with("HTTP 404") => {
                cache::put(&key, b"0");
                false
            }
            Err(_) => false,
        },
    };
    ok.then(|| Candidate {
        source: "Simple Icons",
        title: format!("{query} (brand icon)"),
        url,
        kind: Kind::Svg,
        set: None,
    })
}

fn commons(client: &Client, query: &str) -> Result<Vec<Candidate>, String> {
    let search = format!("{query} logo");
    let v = get_json(
        client,
        "https://commons.wikimedia.org/w/api.php",
        &[
            ("action", "query"),
            ("generator", "search"),
            ("gsrsearch", &search),
            ("gsrnamespace", "6"),
            ("gsrlimit", "15"),
            ("prop", "imageinfo"),
            ("iiprop", "url|mime"),
            ("format", "json"),
        ],
    )?;

    let mut pages: Vec<&Value> = v["query"]["pages"]
        .as_object()
        .map(|m| m.values().collect())
        .unwrap_or_default();
    // Generator results come back keyed by page id; restore search ranking.
    pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(i64::MAX));

    let mut out = Vec::new();
    for p in pages {
        let info = &p["imageinfo"][0];
        let kind = match info["mime"].as_str() {
            Some("image/svg+xml") => Kind::Svg,
            Some("image/png") => Kind::Png,
            _ => continue,
        };
        let (Some(title), Some(url)) = (p["title"].as_str(), info["url"].as_str()) else {
            continue;
        };
        out.push(Candidate {
            source: "Commons",
            title: title.trim_start_matches("File:").to_string(),
            url: url.split('?').next().unwrap_or(url).to_string(),
            kind,
            set: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(simple_icons_slug("Node.js"), "nodedotjs");
        assert_eq!(simple_icons_slug("Google Chrome"), "googlechrome");
        assert_eq!(simple_icons_slug("C++"), "cplusplus");
        assert_eq!(simple_icons_slug("AT&T"), "atandt");
        assert_eq!(simple_icons_slug("Citroën"), "citroen");
    }

    #[test]
    fn tints_current_color() {
        let svg = br#"<svg><path fill="currentColor"/></svg>"#;
        assert_eq!(tint(svg, "#ff0000"), br##"<svg><path fill="#ff0000"/></svg>"##.to_vec());
    }

    #[test]
    fn file_urls() {
        assert_eq!(
            commons_file_url("Apple logo black.svg"),
            "https://commons.wikimedia.org/wiki/Special:FilePath/Apple_logo_black.svg"
        );
    }
}
