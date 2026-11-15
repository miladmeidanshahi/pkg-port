use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;

const MAX_PARTS: usize = 512;

pub enum DiscoveryEvent {
    Found(String),
    Complete(usize),
    Incomplete(String),
}

fn scan_parts(
    prefix: &str,
    mut status_for: impl FnMut(&str) -> Result<u16, String>,
    mut send: impl FnMut(DiscoveryEvent) -> bool,
) {
    for index in 0..MAX_PARTS {
        let candidate = format!("{prefix}_{index}.pkg");
        let mut status = status_for(&candidate);
        // The CDN can transiently deny probes for parts that do exist.
        for _ in 0..2 {
            if !matches!(status, Ok(403)) {
                break;
            }
            status = status_for(&candidate);
        }
        match status {
            Ok(200 | 206) => {
                if !send(DiscoveryEvent::Found(candidate)) {
                    return;
                }
            }
            // shortcut: a persistent CDN 403 may also mean access denied; use a manifest if exact totals are required.
            Ok(403 | 404 | 410) if index > 0 => {
                send(DiscoveryEvent::Complete(index));
                return;
            }
            Ok(status) => {
                send(DiscoveryEvent::Incomplete(format!(
                    "Could not verify part {index} (HTTP {status})"
                )));
                return;
            }
            Err(error) => {
                send(DiscoveryEvent::Incomplete(error));
                return;
            }
        }
    }
    send(DiscoveryEvent::Incomplete(format!(
        "Verification stopped at {MAX_PARTS} parts"
    )));
}

fn head_status(candidate: &str) -> Result<u16, String> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--head",
            "--noproxy",
            "*",
            "--connect-timeout",
            "3",
            "--max-time",
            "8",
            "--output",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            "--write-out",
            "%{http_code}",
            "--",
            candidate,
        ])
        .output()
        .map_err(|_| "curl is unavailable".to_owned())?;
    if !output.status.success() {
        return Err("Package server could not be reached".into());
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| "Invalid package server response".into())
}

pub fn part_index(url: &str) -> Option<usize> {
    let filename = url.rsplit('/').next()?.strip_suffix(".pkg")?;
    filename.rsplit_once('_')?.1.parse().ok()
}

pub fn discover(first_url: &str) -> Option<Receiver<DiscoveryEvent>> {
    let parsed = url::Url::parse(first_url).ok()?;
    if parsed.scheme() != "http"
        || parsed.host_str()? != "gst.prod.dl.playstation.net"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || part_index(first_url) != Some(0)
    {
        return None;
    }
    let prefix = first_url.strip_suffix("_0.pkg")?.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        scan_parts(&prefix, head_status, |event| sender.send(event).is_ok());
    });
    Some(receiver)
}

#[cfg(test)]
mod tests {
    use super::{discover, part_index, scan_parts, DiscoveryEvent};

    #[test]
    fn numbered_package_urls_are_recognized_without_network_access() {
        let first = crate::TEST_GAME_URL;
        assert_eq!(part_index(first), Some(0));
        assert_eq!(part_index(&first.replace("_0.pkg", "_27.pkg")), Some(27));
        assert_eq!(part_index(&first.replace("_0.pkg", "-DP.pkg")), None);
        assert!(discover(&first.replace("gst.prod.dl.playstation.net", "example.com")).is_none());
        assert!(discover(&format!("{first}?token=secret")).is_none());
    }

    #[test]
    fn scan_stops_at_first_missing_part_and_reports_verified_count() {
        let mut events = Vec::new();
        scan_parts(
            "http://example.com/game",
            |url| Ok(if url.ends_with("_3.pkg") { 404 } else { 200 }),
            |event| {
                events.push(event);
                true
            },
        );
        assert_eq!(events.len(), 4);
        assert!(matches!(events.last(), Some(DiscoveryEvent::Complete(3))));
    }

    #[test]
    fn scan_does_not_claim_a_count_when_first_part_is_missing() {
        let mut events = Vec::new();
        scan_parts(
            "http://example.com/game",
            |_| Ok(404),
            |event| {
                events.push(event);
                true
            },
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DiscoveryEvent::Incomplete(_)));
    }

    #[test]
    fn scan_recovers_from_transient_access_denial() {
        let mut events = Vec::new();
        let mut denied = false;
        scan_parts(
            "http://example.com/game",
            |url| {
                let index = part_index(url).unwrap();
                Ok(if index == 12 && !denied {
                    denied = true;
                    403
                } else if index == 13 {
                    404
                } else {
                    200
                })
            },
            |event| {
                events.push(event);
                true
            },
        );
        assert!(denied);
        assert_eq!(events.len(), 14);
        assert!(matches!(events.last(), Some(DiscoveryEvent::Complete(13))));
    }

    #[test]
    fn scan_stops_on_unavailable_parts_but_not_server_failures() {
        for status in [403, 404, 410, 429, 500] {
            let mut events = Vec::new();
            let mut probes = 0;
            scan_parts(
                "http://example.com/game",
                |url| {
                    Ok(if part_index(url) == Some(13) {
                        probes += 1;
                        status
                    } else {
                        200
                    })
                },
                |event| {
                    events.push(event);
                    true
                },
            );
            assert_eq!(probes, if status == 403 { 3 } else { 1 });
            assert_eq!(events.len(), 14);
            assert!(events[..13]
                .iter()
                .all(|event| matches!(event, DiscoveryEvent::Found(_))));
            if matches!(status, 403 | 404 | 410) {
                assert!(matches!(events.last(), Some(DiscoveryEvent::Complete(13))));
            } else {
                assert!(matches!(
                    events.last(),
                    Some(DiscoveryEvent::Incomplete(message))
                        if message.contains(&format!("HTTP {status}"))
                ));
            }
        }
    }
}
