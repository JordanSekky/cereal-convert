mod configuration;
use configuration::Configuration;
mod aggregator;
mod calibre;
mod chapter;
mod royalroad;
mod smtp;
#[macro_use]
extern crate simple_error;
use std::env;

#[tokio::main]
async fn main() {
    println!("Hello, world! :D");
    let config = Configuration::from_config_file();
    println!("{:?}", config);
    let royalroad_books = royalroad::download(&config.royalroad).await.unwrap();
    let aggregate_royalroad_books = aggregator::aggregate(&royalroad_books);
    println!("{:?}", aggregate_royalroad_books);
    for book in aggregate_royalroad_books {
        let path = calibre::convert_to_mobi(&book)
            .expect(&format!("Failed to convert book {} to mobi.", book.title));
        println!("{:?}", path);
        smtp::send_file_smtp(
            &path,
            &env::var("CEREAL_DESTINATION_ADDRESS").unwrap(),
            &book,
        )
        .await;
    }
}
