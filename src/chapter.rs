pub struct Chapter {
    pub body: String,
    pub title: String,
}

impl std::fmt::Debug for Chapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chapter")
            .field("title", &self.title)
            .field("body_len_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Debug)]
pub struct Book {
    pub title: String,
    pub author: String,
    pub chapters: Vec<Chapter>,
}

pub struct AggregateBook {
    pub title: String,
    pub author: String,
    pub body: String,
    pub chapter_titles: Vec<String>,
}

impl std::fmt::Debug for AggregateBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AggregateBook")
            .field("title", &self.title)
            .field("author", &self.author)
            .field("body_len_bytes", &self.body.len())
            .field("chapter_count", &self.chapter_titles.len())
            .finish()
    }
}
