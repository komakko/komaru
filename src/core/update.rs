use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use minisign_verify::{PublicKey, Signature};
use toml::Table;

use crate::core::ingest::fetch_up_to;
use crate::error::UpdateError;
use crate::identity;

const MANIFEST_LIMIT: u64 = 1024 * 1024;
const BUILD_LIMIT: u64 = 256 * 1024 * 1024;
const STABLE_URL: &str = "https://github.com/komakko/komaru/releases/latest/download/latest.toml";
const BETA_URL: &str = "https://github.com/komakko/komaru/releases/download/beta/latest.toml";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ARTIFACT: Option<&str> = Some("komaru-x86_64-linux");
#[cfg(all(windows, target_arch = "x86_64"))]
const ARTIFACT: Option<&str> = Some("komaru-x86_64-windows.exe");
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(windows, target_arch = "x86_64")
)))]
const ARTIFACT: Option<&str> = None;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    fn manifest_url(self) -> &'static str {
        match self {
            Self::Stable => STABLE_URL,
            Self::Beta => BETA_URL,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Build {
    url: String,
    signature: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Release {
    pub version: String,
    pub build: Option<Build>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Identifier {
    Number(u64),
    Text(String),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Version {
    numbers: [u64; 3],
    stable: bool,
    pre: Vec<Identifier>,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
        let numbers = [parts.next()??, parts.next()??, parts.next()??];
        if parts.next().is_some() {
            return None;
        }
        let pre = pre
            .map(|pre| {
                pre.split('.')
                    .map(|part| match part.parse() {
                        Ok(number) => Identifier::Number(number),
                        Err(_) => Identifier::Text(part.to_owned()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            numbers,
            stable: text == core,
            pre,
        })
    }
}

fn newer_than(current: &str, candidate: &str) -> bool {
    match (Version::parse(candidate), Version::parse(current)) {
        (Some(candidate), Some(current)) => candidate.cmp(&current) == Ordering::Greater,
        _ => false,
    }
}

fn parse_manifest(text: &str, artifact: Option<&str>) -> Result<Release, UpdateError> {
    let table: Table = text.parse()?;
    let version = table
        .get("version")
        .and_then(toml::Value::as_str)
        .filter(|version| Version::parse(version).is_some())
        .ok_or(UpdateError::NoVersion)?
        .to_owned();
    let build = artifact
        .and_then(|name| table.get("artifacts")?.as_table()?.get(name)?.as_table())
        .and_then(|entry| {
            Some(Build {
                url: entry.get("url")?.as_str()?.to_owned(),
                signature: entry.get("signature")?.as_str()?.trim().to_owned(),
            })
        });
    Ok(Release { version, build })
}

pub fn check(channel: Channel) -> Result<Option<Release>, UpdateError> {
    let bytes = fetch_up_to(channel.manifest_url(), MANIFEST_LIMIT, || false, |_, _| {})?;
    let text = String::from_utf8_lossy(&bytes);
    let release = parse_manifest(&text, ARTIFACT)?;
    Ok(newer_than(identity::VERSION, &release.version).then_some(release))
}

fn verified(bytes: Vec<u8>, signature: &str, public_key: &str) -> Result<Vec<u8>, UpdateError> {
    let key = PublicKey::from_base64(public_key).map_err(|_| UpdateError::NoKey)?;
    let signature = Signature::decode(signature).map_err(|_| UpdateError::BadSignature)?;
    key.verify(&bytes, &signature, false)
        .map_err(|_| UpdateError::BadSignature)?;
    Ok(bytes)
}

pub fn download(release: &Release) -> Result<Vec<u8>, UpdateError> {
    let build = release.build.as_ref().ok_or(UpdateError::NoBuild)?;
    let bytes = fetch_up_to(&build.url, BUILD_LIMIT, || false, |_, _| {})?;
    verified(bytes, &build.signature, identity::UPDATE_PUBLIC_KEY)
}

pub fn replace_executable(target: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let failed = |source| UpdateError::Replace {
        path: target.to_owned(),
        source,
    };
    let incoming = target.with_extension("new");
    fs::write(&incoming, bytes).map_err(failed)?;
    set_executable(&incoming).map_err(failed)?;
    move_into_place(&incoming, target).map_err(failed)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn move_into_place(incoming: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(incoming, target)
}

#[cfg(not(unix))]
fn move_into_place(incoming: &Path, target: &Path) -> std::io::Result<()> {
    let outgoing = outgoing_path(target);
    fs::remove_file(&outgoing).or_else(ignore_missing)?;
    fs::rename(target, &outgoing)?;
    fs::rename(incoming, target)
}

#[cfg(not(unix))]
fn ignore_missing(error: std::io::Error) -> std::io::Result<()> {
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(not(unix))]
fn outgoing_path(target: &Path) -> std::path::PathBuf {
    target.with_extension("old")
}

#[cfg(not(unix))]
pub fn remove_leftover(executable: &Path) {
    let _ = fs::remove_file(outgoing_path(executable));
}

#[cfg(unix)]
pub fn remove_leftover(_executable: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

    #[test]
    fn versions_order_like_semver() {
        assert!(newer_than("0.1.0", "0.1.1"));
        assert!(newer_than("0.9.9", "0.10.0"));
        assert!(newer_than("0.2.0-beta.1", "0.2.0"));
        assert!(newer_than("0.2.0-beta.2", "0.2.0-beta.10"));
        assert!(newer_than("0.1.0", "v0.2.0-beta.1"));
        assert!(!newer_than("0.2.0", "0.2.0-beta.1"));
        assert!(!newer_than("0.2.0", "0.2.0"));
        assert!(!newer_than("0.2.0", "not a version"));
    }

    #[test]
    fn manifest_yields_the_build_for_this_system() {
        let text = format!(
            "version = \"0.2.0\"\n[artifacts.\"komaru-x86_64-linux\"]\nurl = \"https://example.test/komaru\"\nsignature = '''\n{SIGNATURE}\n'''\n"
        );
        let release = parse_manifest(&text, Some("komaru-x86_64-linux")).unwrap();
        assert_eq!(release.version, "0.2.0");
        let build = release.build.unwrap();
        assert_eq!(build.url, "https://example.test/komaru");
        assert_eq!(build.signature, SIGNATURE);
        assert_eq!(parse_manifest(&text, None).unwrap().build, None);
    }

    #[test]
    fn manifest_without_a_version_is_refused() {
        assert!(matches!(
            parse_manifest("[artifacts]\n", None),
            Err(UpdateError::NoVersion)
        ));
        assert!(matches!(
            parse_manifest("version = \"soon\"\n", None),
            Err(UpdateError::NoVersion)
        ));
    }

    #[test]
    fn only_a_matching_signature_passes() {
        assert_eq!(verified(b"test".to_vec(), SIGNATURE, KEY).unwrap(), b"test");
        assert!(matches!(
            verified(b"tampered".to_vec(), SIGNATURE, KEY),
            Err(UpdateError::BadSignature)
        ));
        assert!(matches!(
            verified(b"test".to_vec(), SIGNATURE, ""),
            Err(UpdateError::NoKey)
        ));
    }

    #[test]
    fn replacing_the_executable_swaps_its_contents() {
        let directory = std::env::temp_dir().join(format!("komaru-update-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("komaru");
        fs::write(&target, b"old").unwrap();
        replace_executable(&target, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!target.with_extension("new").exists());
        fs::remove_dir_all(&directory).unwrap();
    }
}
