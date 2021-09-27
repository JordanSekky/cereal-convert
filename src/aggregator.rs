use crate::chapter::{AggregateBook, Book};

pub fn get_book_html(book: &Book) -> AggregateBook {
    let mut html = String::new();
    let mut chapter_titles = vec![];
    html += &format!("<h1>{}</h1>", book.title);
    for chapter in &book.chapters {
        html += &format!("<h2>{}</h2>", chapter.title);
        html += &chapter.body;
        chapter_titles.push(chapter.title.clone())
    }
    AggregateBook {
        author: book.author.clone(),
        title: book.title.clone(),
        body: html,
        chapter_titles: chapter_titles,
    }
}
