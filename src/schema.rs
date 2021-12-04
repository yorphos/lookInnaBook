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

pub mod joined {
    use rocket::serde::Serialize;

    #[derive(Serialize, Clone, Debug)]
    pub struct Customer {
        pub name: String,
        pub email: String,
        pub street_address: String,
        pub postal_code: String,
        pub province: String,
        pub name_on_card: String,
        pub expiry: String,
        pub billing_street_address: String,
        pub billing_postal_code: String,
        pub billing_province: String,
    }
}

pub mod no_id {
    use crate::db::query::Expiry;

    pub struct Address {
        pub street_address: String,
        pub postal_code: String,
        pub province: String,
    }

    impl Address {
        pub fn new<T: AsRef<str>>(street_address: T, postal_code: T, province: T) -> Address {
            Address {
                street_address: street_address.as_ref().to_string(),
                postal_code: postal_code.as_ref().to_string(),
                province: province.as_ref().to_string(),
            }
        }
    }

    pub struct PaymentInfo {
        pub name_on_card: String,
        pub expiry: Expiry,
        pub card_number: String,
        pub cvv: String,
        pub billing_address: Address,
    }

    impl PaymentInfo {
        pub fn new<T: AsRef<str>>(
            name_on_card: T,
            expiry: Expiry,
            card_number: T,
            cvv: T,
            billing_address: Address,
        ) -> PaymentInfo {
            PaymentInfo {
                name_on_card: name_on_card.as_ref().to_string(),
                expiry,
                card_number: card_number.as_ref().to_string(),
                cvv: cvv.as_ref().to_string(),
                billing_address,
            }
        }
    }
}
