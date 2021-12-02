pub mod entities {
    use rocket::serde::Serialize;

    pub type PostgresInt = i32;
    pub type PostgresNumeric = rust_decimal::Decimal;

    pub type ISBN = PostgresInt;
    pub type PublisherID = PostgresInt;

    #[derive(Serialize, Clone, Debug)]
    pub struct Book {
        pub isbn: ISBN,
        pub title: String,
        pub author_name: String,
        pub genre: String,
        pub publisher: PublisherID,
        pub num_pages: PostgresInt,
        pub price: PostgresNumeric,
        pub author_royalties: PostgresNumeric,
        pub reorder_threshold: PostgresInt,
    }

    impl Book {
        pub fn new(
            isbn: ISBN,
            title: String,
            author_name: String,
            genre: String,
            publisher: PublisherID,
            num_pages: PostgresInt,
            price: PostgresNumeric,
            author_royalties: PostgresNumeric,
            reorder_threshold: PostgresInt,
        ) -> Book {
            Book {
                isbn,
                title,
                author_name,
                genre,
                publisher,
                num_pages,
                price,
                author_royalties,
                reorder_threshold,
            }
        }

        pub fn from_row(row: &postgres::Row) -> Result<Book, postgres::error::Error> {
            Ok(Book::new(
                row.try_get("isbn")?,
                row.try_get("title")?,
                row.try_get("author_name")?,
                row.try_get("genre")?,
                row.try_get("publisher_id")?,
                row.try_get("num_pages")?,
                row.try_get("price")?,
                row.try_get("author_royalties")?,
                row.try_get("reorder_threshold")?,
            ))
        }
    }
}
