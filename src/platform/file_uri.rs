use std::path::PathBuf;

use percent_encoding::percent_decode_str;

pub fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    const SCHEME: &str = "file://";
    let rest = uri.get(SCHEME.len()..)?;
    if !uri[..SCHEME.len()].eq_ignore_ascii_case(SCHEME) {
        return None;
    }
    let path_start = rest.find('/')?;
    let (authority, encoded_path) = rest.split_at(path_start);
    let decoded: Vec<u8> = percent_decode_str(encoded_path).collect();
    path_from_parts(authority, decoded)
}

fn is_local_authority(authority: &str) -> bool {
    authority.is_empty() || authority.eq_ignore_ascii_case("localhost")
}

#[cfg(unix)]
fn path_from_parts(authority: &str, decoded: Vec<u8>) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    is_local_authority(authority).then(|| PathBuf::from(OsString::from_vec(decoded)))
}

#[cfg(windows)]
fn path_from_parts(authority: &str, decoded: Vec<u8>) -> Option<PathBuf> {
    let path = String::from_utf8(decoded).ok()?.replace('/', "\\");
    if is_local_authority(authority) {
        return Some(PathBuf::from(path.strip_prefix('\\')?));
    }
    Some(PathBuf::from(format!("\\\\{authority}{path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn decodes_local_unix_paths() {
        assert_eq!(
            path_from_file_uri("file:///home/a/b%20c.png"),
            Some(PathBuf::from("/home/a/b c.png"))
        );
        assert_eq!(
            path_from_file_uri("FILE://localhost/x.png"),
            Some(PathBuf::from("/x.png"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_remote_hosts() {
        assert_eq!(path_from_file_uri("file://server/x.png"), None);
    }

    #[test]
    fn rejects_other_schemes_and_missing_paths() {
        assert_eq!(path_from_file_uri("http://a/b"), None);
        assert_eq!(path_from_file_uri("file://"), None);
        assert_eq!(path_from_file_uri("file"), None);
    }

    #[cfg(windows)]
    #[test]
    fn decodes_windows_drive_and_unc_paths() {
        assert_eq!(
            path_from_file_uri("file:///C:/pics/a%20b.png"),
            Some(PathBuf::from("C:\\pics\\a b.png"))
        );
        assert_eq!(
            path_from_file_uri("file://server/share/a.png"),
            Some(PathBuf::from("\\\\server\\share\\a.png"))
        );
    }
}
