mod candidates;
mod fetch;
mod file;
mod html;
mod inspect;
mod relative_url;

use std::path::PathBuf;

pub use candidates::Candidates;
pub use fetch::{fetch, fetch_up_to};
pub use file::read_file;

pub struct RawPixels {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub enum Acquired {
    Encoded(Vec<u8>),
    Pixels(RawPixels),
}

pub struct HtmlFragment {
    pub markup: String,
    pub base_url: Option<String>,
}

impl HtmlFragment {
    pub fn without_base(markup: String) -> Self {
        Self {
            markup,
            base_url: None,
        }
    }
}

pub enum Payload {
    Files(Vec<PathBuf>),
    Pixels(RawPixels),
    Bytes(Vec<u8>),
    Html(HtmlFragment),
    Text(String),
}

pub enum Reference {
    Path(PathBuf),
    Url(String),
    Inline(Acquired),
}

impl Reference {
    fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Path(first), Self::Path(second)) => first == second,
            (Self::Url(first), Self::Url(second)) => first == second,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    File(PathBuf),
    Url(String),
    Unnamed,
}

impl Origin {
    pub fn title(&self) -> String {
        match self {
            Self::File(path) => path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            Self::Url(url) => url_title(url),
            Self::Unnamed => "Unnamed image".to_owned(),
        }
    }
}

fn url_title(url: &str) -> String {
    let without_suffix = url.split(['?', '#']).next().unwrap_or(url);
    let last_segment = without_suffix
        .rsplit('/')
        .find(|segment| !segment.is_empty() && !segment.ends_with(':'));
    last_segment.unwrap_or(url).to_owned()
}

pub fn looks_like_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes).is_ok()
}

pub fn keep_first_discard_rest<T>(items: impl IntoIterator<Item = T>) -> Option<T> {
    items.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_first_item() {
        assert_eq!(keep_first_discard_rest([1, 2, 3]), Some(1));
        assert_eq!(keep_first_discard_rest(Vec::<u8>::new()), None);
    }

    #[test]
    fn titles_urls_by_last_path_segment() {
        let title = |url: &str| Origin::Url(url.to_owned()).title();
        assert_eq!(title("https://example.com/a/cat.png?size=2#top"), "cat.png");
        assert_eq!(title("https://example.com/"), "example.com");
        assert_eq!(title("https://example.com"), "example.com");
    }

    #[test]
    fn titles_files_by_name() {
        assert_eq!(Origin::File(PathBuf::from("/x/y/z.jpg")).title(), "z.jpg");
    }
}
