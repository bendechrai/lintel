//! Sharing a project as a link: the whole project rides in the link itself, compressed, after
//! the `#`, so nothing is uploaded anywhere. The page reads it back when it opens.
//!
//! The page also takes `?project=<url>` and fetches a project file from that address (which
//! must allow cross-origin reads, as a raw GitHub or gist link does).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::model::Project;

/// Where the browser build is published.
pub const PAGE: &str = "https://bendechrai.github.io/lintel/app/";

/// The project packed into text that is safe in a link.
pub fn encode(project: &Project) -> Result<String, String> {
    let json = serde_json::to_vec(project).map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(miniz_oxide::deflate::compress_to_vec(&json, 9)))
}

pub fn decode(text: &str) -> Result<Project, String> {
    let packed = URL_SAFE_NO_PAD.decode(text.trim()).map_err(|_| "the link is damaged or cut short".to_string())?;
    let json = miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, 64 << 20).map_err(|_| "the link is damaged or cut short".to_string())?;
    serde_json::from_slice(&json).map_err(|e| format!("the link doesn't hold a project this version reads: {e}"))
}

/// A link that opens the project in the browser.
pub fn link(project: &Project) -> Result<String, String> {
    Ok(format!("{PAGE}#p={}", encode(project)?))
}

/// The project a page address carries after its `#`, if it carries one.
pub fn from_fragment(fragment: &str) -> Option<Result<Project, String>> {
    fragment.trim_start_matches('#').strip_prefix("p=").map(decode)
}

/// The address of a project file named in a page's query string (`?project=...`).
pub fn from_query(query: &str) -> Option<String> {
    query.trim_start_matches('?').split('&').find_map(|pair| pair.strip_prefix("project=")).map(|v| percent_decode(v)).filter(|v| !v.is_empty())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_house_survives_the_trip_through_a_link_and_the_link_is_short_enough_to_send() {
        let house = crate::sample::house();
        let link = link(&house).unwrap();
        assert!(link.starts_with("https://") && link.len() < 30_000, "{} characters", link.len());
        let fragment = &link[link.find('#').unwrap()..];
        assert_eq!(from_fragment(fragment).unwrap().unwrap(), house);
        assert!(from_fragment("#p=not-a-project").unwrap().is_err());
        assert!(from_fragment("#about").is_none());
    }

    #[test]
    fn the_query_names_a_project_file() {
        assert_eq!(from_query("?project=https%3A%2F%2Fexample.com%2Fmy%20house.json&x=1").as_deref(), Some("https://example.com/my house.json"));
        assert_eq!(from_query("?x=1"), None);
    }
}
