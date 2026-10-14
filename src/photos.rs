//! Stock photo search across free providers.
//!
//! - Openverse: no key needed. Openly licensed images from Flickr, Wikimedia and others.
//! - Pexels, Unsplash, Pixabay: free, but each needs an API key from their site.

use std::thread;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::sources::{get_json_or_stale, get_json_with, send};

pub const PAGE_SIZE: u32 = 20;

#[derive(Clone, Debug)]
pub struct Photo {
    pub provider: &'static str,
    pub id: String,
    pub title: String,
    pub author: String,
    /// Photo page on the provider's site.
    pub page_url: String,
    pub thumb_url: String,
    /// Around 1–2k px wide: used for clipboard copies.
    pub large_url: String,
    /// Original/full resolution: used when saving.
    pub full_url: String,
    pub width: u32,
    pub height: u32,
    pub license: String,
    /// Ready-to-paste credit line.
    pub credit: String,
    /// Unsplash asks apps to call this endpoint whenever a photo is downloaded.
    pub download_ping: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Orientation {
    Any,
    Landscape,
    Portrait,
    Square,
}

impl Orientation {
    pub const ALL: [Orientation; 4] = [Self::Any, Self::Landscape, Self::Portrait, Self::Square];

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "Any shape",
            Self::Landscape => "Landscape",
            Self::Portrait => "Portrait",
            Self::Square => "Square",
        }
    }
}

/// API keys for the providers that need one. Empty means "not configured".
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Keys {
    pub pexels: String,
    pub unsplash: String,
    pub pixabay: String,
}

impl Keys {
    /// Keys typed into the app win; otherwise fall back to environment variables.
    pub fn resolved(&self) -> Keys {
        let pick = |typed: &str, env: &str| {
            let typed = typed.trim();
            if typed.is_empty() {
                std::env::var(env).unwrap_or_default()
            } else {
                typed.to_string()
            }
        };
        Keys {
            pexels: pick(&self.pexels, "PEXELS_API_KEY"),
            unsplash: pick(&self.unsplash, "UNSPLASH_ACCESS_KEY"),
            pixabay: pick(&self.pixabay, "PIXABAY_API_KEY"),
        }
    }
}

#[derive(Clone)]
pub struct Query {
    pub text: String,
    pub page: u32,
    pub orientation: Orientation,
    /// Only licenses that allow commercial use (affects Openverse; the others always do).
    pub commercial_only: bool,
}

/// Search every configured provider in parallel. Returns photos interleaved
/// across providers, plus one message per provider that failed.
pub fn search(client: &Client, q: &Query, keys: &Keys) -> (Vec<Photo>, Vec<String>) {
    /// Photos plus an optional note to show the user (e.g. "showing saved results").
    type Found = Result<(Vec<Photo>, Option<String>), String>;
    type Provider = fn(&Client, &Query, &str) -> Found;
    let mut providers: Vec<(&str, Provider, String)> = vec![("Openverse", openverse as Provider, String::new())];
    if !keys.pexels.is_empty() {
        providers.push((
            "Pexels",
            |c, q, k| pexels(c, q, k).map(|p| (p, None)),
            keys.pexels.clone(),
        ));
    }
    if !keys.unsplash.is_empty() {
        providers.push((
            "Unsplash",
            |c, q, k| unsplash(c, q, k).map(|p| (p, None)),
            keys.unsplash.clone(),
        ));
    }
    if !keys.pixabay.is_empty() {
        providers.push((
            "Pixabay",
            |c, q, k| pixabay(c, q, k).map(|p| (p, None)),
            keys.pixabay.clone(),
        ));
    }

    let results: Vec<(&str, Found)> = thread::scope(|scope| {
        let handles: Vec<_> = providers
            .iter()
            .map(|(name, f, key)| (*name, scope.spawn(move || f(client, q, key))))
            .collect();
        handles
            .into_iter()
            .map(|(name, h)| (name, h.join().unwrap_or_else(|_| Err("crashed".into()))))
            .collect()
    });

    let mut lists = Vec::new();
    let mut errors = Vec::new();
    for (name, res) in results {
        match res {
            Ok((list, note)) => {
                lists.push(list);
                errors.extend(note.map(|n| format!("{name}: {n}")));
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    let mut out = Vec::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..longest {
        for list in &lists {
            if let Some(p) = list.get(i) {
                out.push(p.clone());
            }
        }
    }
    (out, errors)
}

/// Check one provider's key with a tiny real search. Returns the result count.
pub fn test_key(client: &Client, provider: &str, key: &str) -> Result<usize, String> {
    let q = Query {
        text: "mountain".into(),
        page: 1,
        orientation: Orientation::Any,
        commercial_only: false,
    };
    let key = key.trim();
    if key.is_empty() {
        return Err("no key entered".into());
    }
    let res = match provider {
        "Pexels" => pexels(client, &q, key),
        "Unsplash" => unsplash(client, &q, key),
        "Pixabay" => pixabay(client, &q, key),
        _ => return Err("unknown provider".into()),
    };
    res.map(|photos| photos.len()).map_err(|e| {
        if e.contains("401") || e.contains("403") || e.contains("400") {
            format!("key rejected ({e})")
        } else {
            e
        }
    })
}

/// Tell Unsplash a photo was downloaded (required by their API guidelines).
pub fn ping_download(client: &Client, photo: &Photo, keys: &Keys) {
    if let Some(url) = &photo.download_ping {
        let auth = format!("Client-ID {}", keys.unsplash);
        let _ = send(|| client.get(url).header("Authorization", &auth));
    }
}

fn str_of(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn u32_of(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}

fn openverse(client: &Client, q: &Query, _key: &str) -> Result<(Vec<Photo>, Option<String>), String> {
    let page = q.page.to_string();
    let size = PAGE_SIZE.to_string();
    let mut params = vec![
        ("q", q.text.as_str()),
        ("page", &page),
        ("page_size", &size),
        ("mature", "false"),
    ];
    if q.commercial_only {
        params.push(("license_type", "commercial"));
    }
    match q.orientation {
        Orientation::Any => {}
        Orientation::Landscape => params.push(("aspect_ratio", "wide")),
        Orientation::Portrait => params.push(("aspect_ratio", "tall")),
        Orientation::Square => params.push(("aspect_ratio", "square")),
    }
    // Openverse's free API is often slow or briefly down; reuse an earlier answer if we have one.
    let (v, note) = match get_json_or_stale(client, "https://api.openverse.org/v1/images/", &params, None) {
        Ok((v, false)) => (v, None),
        Ok((v, true)) => (v, Some("not responding, showing saved results".to_string())),
        // Anonymous Openverse access stops after a few pages.
        Err(e) if q.page > 1 && (e.contains("401") || e.contains("400")) => return Ok((Vec::new(), None)),
        Err(e) => return Err(e),
    };
    let photos = v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let url = r["url"].as_str()?.to_string();
            let license = format!(
                "CC {} {}",
                str_of(&r["license"]).to_uppercase(),
                str_of(&r["license_version"])
            )
            .replace("CC PDM", "Public domain")
            .replace("CC CC0", "CC0")
            .trim()
            .to_string();
            let author = str_of(&r["creator"]);
            Some(Photo {
                provider: "Openverse",
                id: str_of(&r["id"]),
                title: str_of(&r["title"]),
                credit: r["attribution"]
                    .as_str()
                    .map(|a| a.split(" To view").next().unwrap_or(a).trim().to_string())
                    .unwrap_or_else(|| format!("Photo by {author}, {license}")),
                author,
                page_url: str_of(&r["foreign_landing_url"]),
                thumb_url: r["thumbnail"].as_str().unwrap_or(&url).to_string(),
                large_url: url.clone(),
                full_url: url,
                width: u32_of(&r["width"]),
                height: u32_of(&r["height"]),
                license,
                download_ping: None,
            })
        })
        .collect();
    Ok((photos, note))
}

fn pexels(client: &Client, q: &Query, key: &str) -> Result<Vec<Photo>, String> {
    let page = q.page.to_string();
    let size = PAGE_SIZE.to_string();
    let mut params = vec![("query", q.text.as_str()), ("page", &page), ("per_page", &size)];
    match q.orientation {
        Orientation::Any => {}
        Orientation::Landscape => params.push(("orientation", "landscape")),
        Orientation::Portrait => params.push(("orientation", "portrait")),
        Orientation::Square => params.push(("orientation", "square")),
    }
    let v = get_json_with(
        client,
        "https://api.pexels.com/v1/search",
        &params,
        Some(("Authorization", key)),
    )?;
    Ok(v["photos"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let author = str_of(&p["photographer"]);
            Photo {
                provider: "Pexels",
                id: p["id"].to_string(),
                title: str_of(&p["alt"]),
                credit: format!("Photo by {author} on Pexels"),
                author,
                page_url: str_of(&p["url"]),
                thumb_url: str_of(&p["src"]["medium"]),
                large_url: str_of(&p["src"]["large2x"]),
                full_url: str_of(&p["src"]["original"]),
                width: u32_of(&p["width"]),
                height: u32_of(&p["height"]),
                license: "Pexels License".into(),
                download_ping: None,
            }
        })
        .collect())
}

fn unsplash(client: &Client, q: &Query, key: &str) -> Result<Vec<Photo>, String> {
    let page = q.page.to_string();
    let size = PAGE_SIZE.to_string();
    let mut params = vec![
        ("query", q.text.as_str()),
        ("page", &page),
        ("per_page", &size),
        ("content_filter", "high"),
    ];
    match q.orientation {
        Orientation::Any => {}
        Orientation::Landscape => params.push(("orientation", "landscape")),
        Orientation::Portrait => params.push(("orientation", "portrait")),
        Orientation::Square => params.push(("orientation", "squarish")),
    }
    let auth = format!("Client-ID {key}");
    let v = get_json_with(
        client,
        "https://api.unsplash.com/search/photos",
        &params,
        Some(("Authorization", &auth)),
    )?;
    Ok(v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let author = str_of(&p["user"]["name"]);
            let title = p["alt_description"]
                .as_str()
                .or(p["description"].as_str())
                .unwrap_or("")
                .to_string();
            Photo {
                provider: "Unsplash",
                id: str_of(&p["id"]),
                title,
                credit: format!("Photo by {author} on Unsplash"),
                author,
                page_url: str_of(&p["links"]["html"]),
                thumb_url: str_of(&p["urls"]["small"]),
                large_url: str_of(&p["urls"]["regular"]),
                full_url: str_of(&p["urls"]["full"]),
                width: u32_of(&p["width"]),
                height: u32_of(&p["height"]),
                license: "Unsplash License".into(),
                download_ping: p["links"]["download_location"].as_str().map(str::to_string),
            }
        })
        .collect())
}

fn pixabay(client: &Client, q: &Query, key: &str) -> Result<Vec<Photo>, String> {
    let page = q.page.to_string();
    let size = PAGE_SIZE.to_string();
    let mut params = vec![
        ("key", key),
        ("q", q.text.as_str()),
        ("page", &page),
        ("per_page", &size),
        ("image_type", "photo"),
        ("safesearch", "true"),
    ];
    match q.orientation {
        Orientation::Any | Orientation::Square => {} // Pixabay has no square filter
        Orientation::Landscape => params.push(("orientation", "horizontal")),
        Orientation::Portrait => params.push(("orientation", "vertical")),
    }
    let v = match get_json_with(client, "https://pixabay.com/api/", &params, None) {
        Ok(v) => v,
        // Pixabay answers 400 when you page past the last result.
        Err(e) if q.page > 1 && e.contains("400") => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(v["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            let (w, h) = (u32_of(&p["imageWidth"]), u32_of(&p["imageHeight"]));
            q.orientation != Orientation::Square || (w as f32 / h.max(1) as f32 - 1.0).abs() < 0.15
        })
        .map(|p| {
            let author = str_of(&p["user"]);
            Photo {
                provider: "Pixabay",
                id: p["id"].to_string(),
                title: str_of(&p["tags"]),
                credit: format!("Image by {author} from Pixabay"),
                author,
                page_url: str_of(&p["pageURL"]),
                thumb_url: str_of(&p["webformatURL"]),
                large_url: str_of(&p["largeImageURL"]),
                full_url: str_of(&p["largeImageURL"]),
                width: u32_of(&p["imageWidth"]),
                height: u32_of(&p["imageHeight"]),
                license: "Pixabay License".into(),
                download_ping: None,
            }
        })
        .collect())
}

/// File extension for a photo URL, defaulting to jpg.
pub fn extension(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions() {
        assert_eq!(extension("https://x/y/photo.PNG?w=10"), "png");
        assert_eq!(extension("https://images.unsplash.com/photo-123?ixid=abc"), "jpg");
        assert_eq!(extension("https://x/a.webp"), "webp");
    }
}
