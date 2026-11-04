struct Base<'a> {
    scheme: &'a str,
    authority: &'a str,
    path: &'a str,
}

pub fn join(base: &str, reference: &str) -> Option<String> {
    let base = split_base(base)?;
    let reference = without_fragment(reference.trim());
    if let Some(network_path) = reference.strip_prefix("//") {
        return Some(format!("{}://{network_path}", base.scheme));
    }
    if reference.is_empty() || has_scheme(reference) {
        return None;
    }
    let (path, query) = split_query(reference);
    let merged = if path.is_empty() {
        base.path.to_owned()
    } else if path.starts_with('/') {
        remove_dot_segments(path)
    } else {
        remove_dot_segments(&format!("{}{path}", directory_of(base.path)))
    };
    Some(format!(
        "{}://{}{merged}{query}",
        base.scheme, base.authority
    ))
}

fn split_base(base: &str) -> Option<Base<'_>> {
    let (scheme, rest) = base.trim().split_once("://")?;
    let valid_scheme = !scheme.is_empty()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'));
    if !valid_scheme {
        return None;
    }
    let rest = without_fragment(rest);
    let (rest, _) = split_query(rest);
    let path_start = rest.find('/').unwrap_or(rest.len());
    let (authority, path) = rest.split_at(path_start);
    Some(Base {
        scheme,
        authority,
        path: if path.is_empty() { "/" } else { path },
    })
}

fn has_scheme(reference: &str) -> bool {
    reference
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|first| first.contains(':'))
}

fn without_fragment(text: &str) -> &str {
    text.split_once('#').map_or(text, |(before, _)| before)
}

fn split_query(text: &str) -> (&str, &str) {
    text.find('?').map_or((text, ""), |at| text.split_at(at))
}

fn directory_of(path: &str) -> &str {
    path.rfind('/').map_or("/", |at| &path[..=at])
}

fn remove_dot_segments(path: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut segments = path.trim_start_matches('/').split('/').peekable();
    while let Some(segment) = segments.next() {
        let is_last = segments.peek().is_none();
        match segment {
            "." => {}
            ".." => {
                kept.pop();
            }
            other => kept.push(other),
        }
        if is_last && matches!(segment, "." | "..") {
            kept.push("");
        }
    }
    format!("/{}", kept.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://site.test/blog/post/index.html?id=3#top";

    fn join_page(reference: &str) -> Option<String> {
        join(PAGE, reference)
    }

    #[test]
    fn plain_relative_paths_start_in_the_page_directory() {
        assert_eq!(
            join_page("cat.png").as_deref(),
            Some("https://site.test/blog/post/cat.png")
        );
        assert_eq!(
            join_page("./img/cat.png?w=2").as_deref(),
            Some("https://site.test/blog/post/img/cat.png?w=2")
        );
    }

    #[test]
    fn parent_segments_climb_but_never_above_the_root() {
        assert_eq!(
            join_page("../cat.png").as_deref(),
            Some("https://site.test/blog/cat.png")
        );
        assert_eq!(
            join_page("../../../../cat.png").as_deref(),
            Some("https://site.test/cat.png")
        );
        assert_eq!(
            join_page("a/b/../../c/.").as_deref(),
            Some("https://site.test/blog/post/c/")
        );
    }

    #[test]
    fn root_relative_and_network_paths_replace_more_of_the_base() {
        assert_eq!(
            join_page("/static/cat.png").as_deref(),
            Some("https://site.test/static/cat.png")
        );
        assert_eq!(
            join_page("//cdn.test/cat.png").as_deref(),
            Some("https://cdn.test/cat.png")
        );
    }

    #[test]
    fn query_only_references_keep_the_page_path() {
        assert_eq!(
            join_page("?img=7").as_deref(),
            Some("https://site.test/blog/post/index.html?img=7")
        );
    }

    #[test]
    fn fragments_are_dropped() {
        assert_eq!(
            join_page("cat.png#frag").as_deref(),
            Some("https://site.test/blog/post/cat.png")
        );
        assert_eq!(join_page("#top"), None);
    }

    #[test]
    fn references_with_their_own_scheme_are_not_relative() {
        assert_eq!(join_page("blob:https://site.test/1234"), None);
        assert_eq!(join_page("mailto:a@b.test"), None);
        assert_eq!(join_page(""), None);
        assert_eq!(join_page("   "), None);
    }

    #[test]
    fn bases_without_a_path_or_with_ports_work() {
        assert_eq!(
            join("http://localhost:8080", "cat.png").as_deref(),
            Some("http://localhost:8080/cat.png")
        );
        assert_eq!(
            join("http://localhost:8080?x=1", "a/cat.png").as_deref(),
            Some("http://localhost:8080/a/cat.png")
        );
    }

    #[test]
    fn local_page_bases_give_file_addresses() {
        assert_eq!(
            join("file:///home/me/page.html", "pics/cat.png").as_deref(),
            Some("file:///home/me/pics/cat.png")
        );
    }

    #[test]
    fn unusable_bases_are_rejected() {
        assert_eq!(join("about:blank", "cat.png"), None);
        assert_eq!(join("", "cat.png"), None);
        assert_eq!(join("://x.test/", "cat.png"), None);
        assert_eq!(join("not a url", "cat.png"), None);
    }

    #[test]
    fn hostile_references_do_not_panic() {
        for reference in [
            "....//", "/..", "/../", "./..", "a//..//b", "?", "?#", "\u{0}",
        ] {
            let _ = join_page(reference);
        }
    }
}
