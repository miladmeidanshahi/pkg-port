use std::io::Cursor;
use std::process::Command;

const TITLES: &str =
    "https://raw.githubusercontent.com/andshrew/PlayStation-Titles/main/PS5_Titles.tsv";

pub struct GameMetadata {
    pub name: String,
    pub cover: Result<Vec<u8>, String>,
}

pub fn lookup(title_id: &str) -> Result<GameMetadata, String> {
    if title_id.len() != 9
        || !title_id.starts_with("PPSA")
        || !title_id[4..].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Invalid PS5 title ID".into());
    }
    let titles = String::from_utf8(fetch(TITLES)?).map_err(|_| "Invalid title list encoding")?;
    let (name, concept_id) = title_record(&titles, title_id)
        .ok_or_else(|| format!("{title_id} was not found in the title list"))?;
    let cover = (|| {
        let page = String::from_utf8(fetch(&format!(
            "https://store.playstation.com/en-us/concept/{concept_id}"
        ))?)
        .map_err(|_| "Invalid Store page encoding")?;
        let url = cover_url(&page).ok_or("Store page has no cover image")?;
        let bytes = fetch(&url)?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| format!("Cover image cannot be decoded: {error}"))?;
        let mut png = Cursor::new(Vec::new());
        image
            .thumbnail(256, 256)
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|error| format!("Cover image cannot be encoded: {error}"))?;
        Ok(png.into_inner())
    })();
    Ok(GameMetadata { name, cover })
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    // shortcut: curl must be installed, use an in-process HTTPS client if that is unavailable.
    let output = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            "--max-filesize",
            "8000000",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            url,
        ])
        .output()
        .map_err(|error| format!("Cannot run curl: {error}"))?;
    if !output.status.success() || output.stdout.len() > 8_000_000 {
        let reason = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Metadata request failed for {} (curl exit {}): {}",
            url::Url::parse(url)
                .map(|url| url.host_str().unwrap_or("").to_owned())
                .unwrap_or_default(),
            output.status.code().unwrap_or(-1),
            reason.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(output.stdout)
}

fn title_record(titles: &str, title_id: &str) -> Option<(String, String)> {
    let wanted = format!("{title_id}_00");
    for line in titles.lines().skip(1) {
        let mut columns = line.split('\t');
        let Some(id) = columns.next() else { continue };
        if id != wanted {
            continue;
        }
        let Some(concept_id) = columns.next() else {
            continue;
        };
        let Some(name) = columns.next() else { continue };
        if !concept_id.is_empty()
            && concept_id.bytes().all(|byte| byte.is_ascii_digit())
            && !name.is_empty()
        {
            return Some((name.into(), concept_id.into()));
        }
    }
    None
}

fn cover_url(page: &str) -> Option<String> {
    if let Some(script) = page
        .split("id=\"mfe-jsonld-tags\"")
        .nth(1)
        .and_then(|tail| tail.split('>').nth(1))
        .and_then(|tail| tail.split("</script>").next())
    {
        for key in ["\"image\":\"", "\"image\": \""] {
            if let Some(value) = script
                .split(key)
                .nth(1)
                .and_then(|tail| tail.split('"').next())
            {
                if let Some(url) = valid_cover_url(value) {
                    return Some(url);
                }
            }
        }
    }
    for chunk in page.split("<meta").skip(1) {
        let Some(tag) = chunk.split('>').next() else {
            continue;
        };
        if !tag.contains("property=\"og:image\"") {
            continue;
        }
        let Some(content) = tag
            .split("content=\"")
            .nth(1)
            .and_then(|value| value.split('"').next())
        else {
            continue;
        };
        if let Some(url) = valid_cover_url(content) {
            return Some(url);
        }
    }
    None
}

fn valid_cover_url(value: &str) -> Option<String> {
    let candidate = value.replace("\\/", "/").replace("&amp;", "&");
    let url = url::Url::parse(&candidate).ok()?;
    (url.scheme() == "https" && url.host_str() == Some("image.api.playstation.com"))
        .then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_title_and_cover_from_source_shapes() {
        let titles =
            "titleId\tconceptId\tname\tcontentId\nPPSA01547_00\t10000730\tGrand Theft Auto VI\t\n";
        assert_eq!(
            title_record(titles, "PPSA01547"),
            Some(("Grand Theft Auto VI".into(), "10000730".into()))
        );
        assert_eq!(
            cover_url("<meta property=\"og:image\" content=\"https://image.api.playstation.com/vulcan/cover.jpg?w=1&amp;h=2\">"),
            Some("https://image.api.playstation.com/vulcan/cover.jpg?w=1&h=2".into())
        );
        assert!(cover_url(
            "<meta property=\"og:image\" content=\"https://example.com/cover.jpg\">"
        )
        .is_none());
        assert_eq!(
            cover_url("<meta property=\"og:image:width\" content=\"256\"><meta property=\"og:image\" content=\"https://image.api.playstation.com/cover.png\">"),
            Some("https://image.api.playstation.com/cover.png".into())
        );
        assert_eq!(
            cover_url("<script id=\"mfe-jsonld-tags\" type=\"application/ld+json\">{\"image\":\"https:\\/\\/image.api.playstation.com\\/vulcan\\/cover.png\"}</script>"),
            Some("https://image.api.playstation.com/vulcan/cover.png".into())
        );
    }
}
