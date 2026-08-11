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

    match fetch_releases() {
        Ok(releases) => match find_upgrade(&current_version, releases) {
            Some(release) => AppUpdateInfo {
                latest: Some(release.version.to_string()),
                update_available: true,
                release_url: Some(release.url),
                current,
                check_error: None,
            },
            None => AppUpdateInfo {
                current,
                latest: None,
                update_available: false,
                release_url: None,
                check_error: None,
            },
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

fn fetch_releases() -> Result<Vec<RemoteRelease>, String> {
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

    let mut parsed = Vec::new();
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
        parsed.push(RemoteRelease {
            version,
            url: release_url,
        });
    }

    Ok(parsed)
}

/// Running 0.1.2-rcN should not be nudged to install 0.1.2 stable from the same line.
fn is_stable_same_base_upgrade(current: &Version, candidate: &Version) -> bool {
    !current.pre.is_empty()
        && candidate.pre.is_empty()
        && (current.major, current.minor, current.patch)
            == (candidate.major, candidate.minor, candidate.patch)
}

fn is_upgrade_candidate(current: &Version, candidate: &Version) -> bool {
    candidate > current && !is_stable_same_base_upgrade(current, candidate)
}

fn find_upgrade(current: &Version, releases: Vec<RemoteRelease>) -> Option<RemoteRelease> {
    releases
        .into_iter()
        .filter(|release| is_upgrade_candidate(current, &release.version))
        .max_by(|left, right| left.version.cmp(&right.version))
}

fn parse_version(raw: &str) -> Result<Version, String> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    Version::parse(trimmed).map_err(|error| format!("invalid version {raw:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> RemoteRelease {
        RemoteRelease {
            version: Version::parse(version).unwrap(),
            url: format!("https://example.test/{version}"),
        }
    }

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
        let newer = Version::parse("0.1.2-rc3").unwrap();
        assert!(newer > older);
    }

    #[test]
    fn stable_is_newer_than_rc_in_semver_but_not_an_upgrade_offer() {
        let rc = Version::parse("0.1.2-rc2").unwrap();
        let stable = Version::parse("0.1.2").unwrap();
        assert!(stable > rc);
        assert!(is_stable_same_base_upgrade(&rc, &stable));
        assert!(!is_upgrade_candidate(&rc, &stable));
    }

    #[test]
    fn rc_user_is_not_offered_older_stable_on_same_base() {
        let current = Version::parse("0.1.2-rc3").unwrap();
        let releases = vec![
            release("0.1.2"),
            release("0.1.2-rc1"),
            release("0.1.2-rc2"),
            release("0.1.2-rc3"),
        ];
        assert!(find_upgrade(&current, releases).is_none());
    }

    #[test]
    fn rc_user_is_offered_newer_rc3() {
        let current = Version::parse("0.1.2-rc2").unwrap();
        let releases = vec![release("0.1.2"), release("0.1.2-rc2"), release("0.1.2-rc3")];
        let upgrade = find_upgrade(&current, releases).unwrap();
        assert_eq!(upgrade.version, Version::parse("0.1.2-rc3").unwrap());
    }

    #[test]
    fn stable_user_is_not_offered_prerelease() {
        let current = Version::parse("0.1.2").unwrap();
        let releases = vec![release("0.1.2"), release("0.1.2-rc2")];
        assert!(find_upgrade(&current, releases).is_none());
    }
}
