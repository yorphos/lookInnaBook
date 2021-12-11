pub mod entities {
    use rocket::serde::Serialize;

    pub type PostgresInt = i32;
    pub type PostgresNumeric = rust_decimal::Decimal;

    pub type ISBN = PostgresInt;
    pub type PublisherID = PostgresInt;

    #[derive(Serialize, Clone, Debug)]
    pub struct OwnerLogin {
        pub owner_id: PostgresInt,
        pub email: String,
        pub name: String,
    }

    #[derive(Serialize, Clone, Debug)]
    pub struct CustomerLogin {
        pub customer_id: PostgresInt,
        pub email: String,
        pub name: String,
    }

    #[derive(Serialize, Clone, Debug)]
    pub struct Publisher {
        pub publisher_id: PublisherID,
        pub company_name: String,
        pub email: String,
        pub phone_number: String,
        pub bank_number: String,
        pub address_id: PostgresInt,
    }

    impl Publisher {
        pub fn new(
            publisher_id: PublisherID,
            company_name: String,
            email: String,
            phone_number: String,
            bank_number: String,
            address_id: PostgresInt,
        ) -> Publisher {
            Publisher {
                publisher_id,
                company_name,
                email,
                phone_number,
                bank_number,
                address_id,
            }
        }

        pub fn from_row(row: &postgres::Row) -> Result<Publisher, postgres::error::Error> {
            Ok(Publisher::new(
                row.try_get("publisher_id")?,
                row.try_get("company_name")?,
                row.try_get("email")?,
                row.try_get("phone_number")?,
                row.try_get("bank_number")?,
                row.try_get("address_id")?,
            ))
        }
    }

    #[derive(Serialize, Clone, Debug)]
    pub struct BookWithPublisherName {
        pub isbn: ISBN,
        pub title: String,
        pub author_name: String,
        pub genre: String,
        pub publisher_id: PostgresInt,
        pub publisher_name: String,
        pub num_pages: PostgresInt,
        pub price: PostgresNumeric,
        pub author_royalties: PostgresNumeric,
        pub reorder_threshold: PostgresInt,
        pub stock: PostgresInt,
        pub discontinued: bool,
    }

    impl BookWithPublisherName {
        pub fn new(
            isbn: ISBN,
            title: String,
            author_name: String,
            genre: String,
            publisher_id: PostgresInt,
            publisher_name: String,
            num_pages: PostgresInt,
            price: PostgresNumeric,
            author_royalties: PostgresNumeric,
            reorder_threshold: PostgresInt,
            stock: PostgresInt,
            discontinued: bool,
        ) -> BookWithPublisherName {
            BookWithPublisherName {
                isbn,
                title,
                author_name,
                genre,
                publisher_id,
                publisher_name,
                num_pages,
                price,
                author_royalties,
                reorder_threshold,
                stock,
                discontinued,
            }
        }

        pub fn from_row(
            row: &postgres::Row,
        ) -> Result<BookWithPublisherName, postgres::error::Error> {
            Ok(BookWithPublisherName::new(
                row.try_get("isbn")?,
                row.try_get("title")?,
                row.try_get("author_name")?,
                row.try_get("genre")?,
                row.try_get("publisher_id")?,
                row.try_get("publisher_name")?,
                row.try_get("num_pages")?,
                row.try_get("price")?,
                row.try_get("author_royalties")?,
                row.try_get("reorder_threshold")?,
                row.try_get("stock")?,
                row.try_get("discontinued")?,
            ))
        }
    }

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
        pub stock: PostgresInt,
        pub discontinued: bool,
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
            stock: PostgresInt,
            discontinued: bool,
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
                stock,
                discontinued,
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
                row.try_get("stock")?,
                row.try_get("discontinued")?,
            ))
        }
    }

    #[derive(Serialize, Clone, Debug, Default)]
    pub struct Customer {
        pub name: String,
        pub email: String,
        pub default_shipping_address_id: PostgresInt,
        pub default_payment_info_id: PostgresInt,
    }
}

pub mod joined {
    use rocket::serde::Serialize;

    use super::{
        entities::{Book, PostgresInt},
        no_id::{Address, PaymentInfo},
    };

    #[derive(Serialize, Clone, Debug, Default)]
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

    #[derive(Serialize, Clone, Debug)]
    pub struct OrderNoBooks {
        pub order_id: PostgresInt,
        pub shipping_address: Address,
        pub tracking_number: String,
        pub order_status: String,
        pub order_date: String,
        pub payment_info: PaymentInfo,
    }

    #[derive(Serialize, Clone, Debug)]
    pub struct Order {
        pub order_id: PostgresInt,
        pub shipping_address: Address,
        pub tracking_number: String,
        pub order_status: String,
        pub order_date: String,
        pub payment_info: PaymentInfo,
        pub books: Vec<(Book, u32)>,
    }

    impl Order {
        pub fn from_order_with_id(order: OrderNoBooks, books: Vec<(Book, u32)>) -> Order {
            let OrderNoBooks {
                order_id,
                shipping_address,
                tracking_number,
                order_status,
                order_date,
                payment_info,
            } = order;
            Order {
                order_id,
                books,
                shipping_address,
                tracking_number,
                order_date,
                order_status,
                payment_info,
            }
        }
    }
}

pub mod no_id {
    use crate::db::query::Expiry;
    use rocket::serde::Serialize;

    #[derive(Clone, Debug, Serialize)]
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

    #[derive(Clone, Debug, Serialize)]
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
