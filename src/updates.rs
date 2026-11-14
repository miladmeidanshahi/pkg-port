use futures::AsyncReadExt as _;
use gpui_kit::http_client::{AsyncBody, HttpClient, HttpRequestExt, RedirectPolicy, Request};
use semver::Version;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

const RELEASES_URL: &str =
    "https://api.github.com/repos/miladmeidanshahi/pkg-port/releases?per_page=1";
const MAX_RELEASE_RESPONSE: u64 = 64 * 1024;

pub struct AvailableUpdate {
    pub version: Version,
    pub download_url: String,
}

#[derive(Deserialize)]
struct Release {
    name: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub async fn check_latest(client: Arc<dyn HttpClient>) -> Result<Option<AvailableUpdate>, String> {
    let request = Request::builder()
        .uri(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "PKGPort")
        .timeout(Duration::from_secs(8))
        .follow_redirects(RedirectPolicy::FollowAll)
        .body(AsyncBody::empty())
        .map_err(|error| error.to_string())?;
    let mut response = client
        .send(request)
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub returned {}", response.status()));
    }

    let mut body = Vec::new();
    std::mem::replace(response.body_mut(), AsyncBody::empty())
        .take(MAX_RELEASE_RESPONSE)
        .read_to_end(&mut body)
        .await
        .map_err(|error| error.to_string())?;
    let mut releases: Vec<Release> =
        serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    let Some(release) = releases.pop() else {
        return Ok(None);
    };
    let Some(version) = release
        .name
        .as_deref()
        .and_then(|name| name.strip_prefix("PKGPort "))
        .and_then(|version| Version::parse(version).ok())
    else {
        return Ok(None);
    };
    if version <= Version::parse(env!("CARGO_PKG_VERSION")).expect("valid package version") {
        return Ok(None);
    }

    let download_url = platform_asset_suffix()
        .and_then(|suffix| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(suffix))
        })
        .map(|asset| asset.browser_download_url.as_str())
        .filter(|url| is_github_https_url(url))
        .or_else(|| is_github_https_url(&release.html_url).then_some(&release.html_url));
    let Some(download_url) = download_url else {
        return Ok(None);
    };

    Ok(Some(AvailableUpdate {
        version,
        download_url: download_url.to_owned(),
    }))
}

fn is_github_https_url(value: &str) -> bool {
    url::Url::parse(value)
        .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("github.com"))
}

const fn platform_asset_suffix() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("windows-x86_64-setup.exe")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some("linux-x86_64.deb")
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some("macos-aarch64.dmg")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some("macos-x86_64.dmg")
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
    )))]
    {
        None
    }
}
