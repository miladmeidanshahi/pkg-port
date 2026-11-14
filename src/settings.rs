use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub bind_ip: String,
    pub port: String,
    pub selected_interface: Option<Ipv4Addr>,
    pub package_dir: Option<PathBuf>,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind_ip: "0.0.0.0".into(),
            port: "8080".into(),
            selected_interface: None,
            package_dir: None,
            language: "en".into(),
        }
    }
}

fn path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)?
        .join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("ps-rusty-downloader/settings.json"))
}

pub fn load() -> Settings {
    let Some(path) = path() else {
        return Settings::default();
    };
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            eprintln!("Invalid settings at {}: {error}", path.display());
            Settings::default()
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
        Err(error) => {
            eprintln!("Cannot read settings at {}: {error}", path.display());
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> io::Result<()> {
    let path = path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Config directory unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(io::Error::other)?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn default_settings_round_trip() {
        let encoded = serde_json::to_vec(&Settings::default()).unwrap();
        let decoded: Settings = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.bind_ip, "0.0.0.0");
        assert_eq!(decoded.port, "8080");
        assert_eq!(decoded.language, "en");
    }
}
