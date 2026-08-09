use serde::Serialize;
use semver::Version;

use crate::version::APP_VERSION;

const GITHUB_REPO: &str = "wh7019025/Harbor";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub check_error: Option<String>,
}

pub fn check_app_update() -> AppUpdateInfo {
    let current = APP_VERSION.to_string();
    let current_version = match parse_version(current.as_str()) {
        Ok(version) => version,
        Err(error) => {
            return AppUpdateInfo {
                current,
                latest: None,
                update_available: false,
                release_url: None,
                check_error: Some(error),
            };
        }
    };

    match fetch_latest_release() {
        Ok(Some(release)) => {
            let update_available = release.version > current_version;
            AppUpdateInfo {
                latest: Some(release.version.to_string()),
                update_available,
                release_url: if update_available {
                    Some(release.url)
                } else {
                    None
                },
                current,
                check_error: None,
            }
        }
        Ok(None) => AppUpdateInfo {
            current,
            latest: None,
            update_available: false,
            release_url: None,
            check_error: None,
        },
        Err(error) => AppUpdateInfo {
            current,
            latest: None,
            update_available: false,
            release_url: None,
            check_error: Some(error),
        },
    }
}

struct RemoteRelease {
    version: Version,
    url: String,
}

fn fetch_latest_release() -> Result<Option<RemoteRelease>, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page=30");
    let response = ureq::get(&url)
        .set("User-Agent", "Harbor")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| format!("request failed: {error}"))?;
    if !(200..300).contains(&response.status()) {
        return Err(format!("GitHub API returned {}", response.status()));
    }

    let releases: Vec<serde_json::Value> = response
        .into_json()
        .map_err(|error| format!("parse releases failed: {error}"))?;

    let mut best: Option<RemoteRelease> = None;
    for release in releases {
        let Some(tag_name) = release.get("tag_name").and_then(|value| value.as_str()) else {
            continue;
        };
        let version = match parse_version(tag_name) {
            Ok(version) => version,
            Err(_) => continue,
        };
        let release_url = release
            .get("html_url")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        if release_url.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|current| version > current.version) {
            best = Some(RemoteRelease {
                version,
                url: release_url,
            });
        }
    }

    Ok(best)
}

fn parse_version(raw: &str) -> Result<Version, String> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    Version::parse(trimmed).map_err(|error| format!("invalid version {raw:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_accepts_tag_prefix() {
        assert_eq!(
            parse_version("v0.1.2-rc1").unwrap(),
            Version::parse("0.1.2-rc1").unwrap()
        );
    }

    #[test]
    fn rc_is_newer_than_previous_rc() {
        let older = Version::parse("0.1.2-rc1").unwrap();
        let newer = Version::parse("0.1.2-rc2").unwrap();
        assert!(newer > older);
    }

    #[test]
    fn stable_is_newer_than_rc() {
        let rc = Version::parse("0.1.2-rc1").unwrap();
        let stable = Version::parse("0.1.2").unwrap();
        assert!(stable > rc);
    }
}
