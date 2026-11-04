use std::path::PathBuf;

use toml::{Table, Value};

use crate::core::ingest::Origin;

const KIND: &str = "kind";
const TARGET: &str = "target";
const FILE_KIND: &str = "file";
const LINK_KIND: &str = "link";
const ITEMS: &str = "items";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Recent {
    File(PathBuf),
    Link(String),
}

impl Recent {
    pub fn from_origin(origin: &Origin) -> Option<Self> {
        match origin {
            Origin::File(path) => Some(Self::File(
                std::path::absolute(path).unwrap_or_else(|_| path.clone()),
            )),
            Origin::Url(url) => Some(Self::Link(url.clone())),
            Origin::Unnamed => None,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::File(path) => Origin::File(path.clone()).title(),
            Self::Link(url) => Origin::Url(url.clone()).title(),
        }
    }

    pub fn location(&self) -> String {
        match self {
            Self::File(path) => path.to_string_lossy().into_owned(),
            Self::Link(url) => url.clone(),
        }
    }
}

#[derive(Default)]
pub struct Recents {
    items: Vec<Recent>,
}

impl Recents {
    pub fn load(section: Option<&Table>) -> Self {
        let items = section
            .and_then(|section| section.get(ITEMS))
            .and_then(Value::as_array)
            .map(|entries| entries.iter().filter_map(read).collect())
            .unwrap_or_default();
        Self { items }
    }

    pub fn to_section(&self) -> Table {
        let mut section = Table::new();
        if !self.items.is_empty() {
            let entries = self.items.iter().map(write).collect();
            section.insert(ITEMS.to_owned(), Value::Array(entries));
        }
        section
    }

    pub fn items(&self) -> &[Recent] {
        &self.items
    }

    pub fn get(&self, index: usize) -> Option<&Recent> {
        self.items.get(index)
    }

    pub fn remember(&mut self, recent: Recent, limit: usize) {
        self.items.retain(|item| *item != recent);
        self.items.insert(0, recent);
        self.items.truncate(limit);
    }

    pub fn truncate(&mut self, limit: usize) -> bool {
        let changed = self.items.len() > limit;
        self.items.truncate(limit);
        changed
    }

    pub fn clear(&mut self) -> bool {
        let changed = !self.items.is_empty();
        self.items.clear();
        changed
    }
}

fn read(value: &Value) -> Option<Recent> {
    let entry = value.as_table()?;
    let target = entry.get(TARGET)?.as_str()?;
    match entry.get(KIND)?.as_str()? {
        FILE_KIND => Some(Recent::File(PathBuf::from(target))),
        LINK_KIND => Some(Recent::Link(target.to_owned())),
        _ => None,
    }
}

fn write(recent: &Recent) -> Value {
    let (kind, target) = match recent {
        Recent::File(path) => (FILE_KIND, path.to_string_lossy().into_owned()),
        Recent::Link(url) => (LINK_KIND, url.clone()),
    };
    let mut entry = Table::new();
    entry.insert(KIND.to_owned(), kind.into());
    entry.insert(TARGET.to_owned(), target.into());
    Value::Table(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Recent {
        Recent::File(PathBuf::from(name))
    }

    #[test]
    fn newest_first_without_duplicates_and_within_the_limit() {
        let mut recents = Recents::default();
        for name in ["a", "b", "c", "a"] {
            recents.remember(file(name), 3);
        }
        assert_eq!(recents.items(), [file("a"), file("c"), file("b")]);
        recents.remember(file("d"), 3);
        assert_eq!(recents.items(), [file("d"), file("a"), file("c")]);
    }

    #[test]
    fn items_survive_a_round_trip_and_bad_entries_are_skipped() {
        let mut recents = Recents::default();
        recents.remember(Recent::Link("https://x.test/a.png".to_owned()), 5);
        recents.remember(file("/tmp/b.png"), 5);
        let mut section = recents.to_section();
        if let Some(Value::Array(entries)) = section.get_mut(ITEMS) {
            entries.push(Value::Integer(7));
        }
        let loaded = Recents::load(Some(&section));
        assert_eq!(loaded.items(), recents.items());
    }

    #[test]
    fn shrinking_and_clearing_report_whether_anything_changed() {
        let mut recents = Recents::default();
        assert!(!recents.clear());
        recents.remember(file("a"), 5);
        recents.remember(file("b"), 5);
        assert!(recents.truncate(1));
        assert!(!recents.truncate(1));
        assert!(recents.clear());
        assert!(recents.to_section().is_empty());
    }

    #[test]
    fn unnamed_images_are_not_remembered() {
        assert_eq!(Recent::from_origin(&Origin::Unnamed), None);
    }
}
