pub mod conn {
    use rocket_sync_db_pools::{database, postgres};

    #[database("postgres")]
    pub struct DbConn(postgres::Client);
}

pub mod error {
    use std::error::Error;
    use std::fmt::Display;

    use bcrypt::BcryptError;

    #[derive(Debug, Clone, Copy)]
    pub struct NotEnoughStockError {}

    impl NotEnoughStockError {
        pub fn new() -> NotEnoughStockError {
            NotEnoughStockError {}
        }
    }

    impl Error for NotEnoughStockError {}

    impl Display for NotEnoughStockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Not enough stock for that operation")
        }
    }

    #[derive(Debug)]
    pub enum CartError {
        NotEnoughStock(NotEnoughStockError),
        DBError(postgres::error::Error),
    }

    impl Display for CartError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::DBError(err) => Display::fmt(&err, f),
                Self::NotEnoughStock(err) => Display::fmt(&err, f),
            }
        }
    }

    impl From<postgres::error::Error> for CartError {
        fn from(e: postgres::error::Error) -> Self {
            CartError::DBError(e)
        }
    }

    impl From<NotEnoughStockError> for CartError {
        fn from(e: NotEnoughStockError) -> Self {
            CartError::NotEnoughStock(e)
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct CredentialError {}

    impl CredentialError {
        pub fn new() -> CredentialError {
            CredentialError {}
        }
    }

    impl Error for CredentialError {}

    impl Display for CredentialError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Invalid email/password")
        }
    }

    #[derive(Debug)]
    pub enum LoginError {
        DBError(postgres::error::Error),
        CredentialError(CredentialError),
        BCryptError(bcrypt::BcryptError),
    }

    impl Error for LoginError {}

    impl Display for LoginError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::DBError(err) => Display::fmt(&err, f),
                Self::CredentialError(err) => Display::fmt(&err, f),
                Self::BCryptError(err) => Display::fmt(&err, f),
            }
        }
    }

    impl From<postgres::error::Error> for LoginError {
        fn from(e: postgres::error::Error) -> Self {
            LoginError::DBError(e)
        }
    }

    impl From<CredentialError> for LoginError {
        fn from(e: CredentialError) -> Self {
            LoginError::CredentialError(e)
        }
    }

    impl From<BcryptError> for LoginError {
        fn from(e: BcryptError) -> Self {
            LoginError::BCryptError(e)
        }
    }
}

pub mod query {
    use super::conn::DbConn;
    use super::error::CartError;
    use super::error::LoginError;
    use super::error::NotEnoughStockError;
    use crate::db::error::CredentialError;
    use crate::schema::entities::*;
    use crate::schema::no_id;
    use serde::Serialize;

    pub async fn get_books(conn: &DbConn) -> Result<Vec<Book>, postgres::error::Error> {
        let rows = conn
            .run(|c| c.query("SELECT * FROM base.book", &[]))
            .await?;
        Ok(rows.iter().flat_map(|row| Book::from_row(row)).collect())
    }

    pub async fn validate_customer_login<T: AsRef<str>>(
        conn: &DbConn,
        email: T,
        password: T,
    ) -> Result<PostgresInt, LoginError> {
        let email = email.as_ref().to_owned();
        let password = password.as_ref().to_owned();

        let customer = conn
            .run(move |c| {
                c.query_opt(
                    "SELECT customer_id, password_hash FROM base.customer WHERE email = $1",
                    &[&email],
                )
            })
            .await?;

        match customer {
            Some(customer) => {
                let customer_id = customer.get("customer_id");
                let password_hash = customer.get("password_hash");

                if bcrypt::verify(password, password_hash)? {
                    Ok(customer_id)
                } else {
                    Err(CredentialError::new())?
                }
            }
            None => Err(CredentialError::new())?,
        }
    }

    pub async fn does_customer_with_email_exist<'a, T: AsRef<str>>(
        conn: &DbConn,
        email: T,
    ) -> Result<bool, postgres::error::Error> {
        let email = email.as_ref().to_string();
        Ok(conn
            .run(move |c| {
                c.query_opt(
                    "SELECT * FROM base.customer WHERE base.customer.email = $1",
                    &[&email],
                )
            })
            .await?
            .is_some())
    }

    pub async fn try_add_address(
        conn: &DbConn,
        address: no_id::Address,
    ) -> Result<PostgresInt, postgres::error::Error> {
        let no_id::Address {
            street_address,
            postal_code,
            province,
        } = address;
        Ok(conn.run(move |c| {
            c.query_one(
                "INSERT INTO base.address (street_address, postal_code, province) VALUES ($1, $2, $3) RETURNING address_id",
                &[&street_address, &postal_code, &province],
            )
        }).await?.get("address_id"))
    }

    #[derive(Debug, Clone, Copy, Serialize)]
    pub struct Expiry {
        month: u32,
        year: u32,
    }

    impl Expiry {
        pub fn from_str<T: AsRef<str>>(s: T) -> Option<Expiry> {
            let mut split = s.as_ref().split('/');
            let month = split.next()?.parse().ok()?;
            if month < 1 || month > 12 {
                return None;
            }

            let year = split.next()?.parse().ok()?;
            if year < 21 {
                return None;
            }

            if split.next().is_some() {
                return None;
            }

            Some(Expiry { month, year })
        }
    }

    impl ToString for Expiry {
        fn to_string(&self) -> String {
            format!("{}/{}", self.month, self.year)
        }
    }

    pub async fn try_add_payment_info(
        conn: &DbConn,
        payment_info: no_id::PaymentInfo,
    ) -> Result<PostgresInt, postgres::error::Error> {
        let no_id::PaymentInfo {
            name_on_card,
            expiry,
            card_number,
            cvv,
            billing_address,
        } = payment_info;

        let billing_address_id = try_add_address(conn, billing_address).await?;
        Ok(conn.run(move |c| {
            c.query_one(
                "INSERT INTO base.payment_info (name_on_card, expiry, card_number, cvv, billing_address_id) VALUES ($1, $2, $3, $4, $5) RETURNING payment_info_id",
                &[&name_on_card, &expiry.to_string(), &card_number, &cvv, &billing_address_id],
            )
        }).await?.get("payment_info_id"))
    }

    pub async fn try_create_new_customer<'a, T: AsRef<str>>(
        conn: &DbConn,
        email: T,
        password: T,
        name: T,
        address: no_id::Address,
        payment_info: no_id::PaymentInfo,
    ) -> Result<PostgresInt, Box<dyn std::error::Error>> {
        let name = name.as_ref().to_string();
        let email = email.as_ref().to_string();
        let password = password.as_ref().to_string();

        let address_id = try_add_address(conn, address).await?;
        let payment_info_id = try_add_payment_info(conn, payment_info).await?;

        let password_hash = bcrypt::hash(password, 10)?;

        Ok(conn
            .run(move |c| {
                c.query_one(
                    "INSERT INTO base.customer (name, email, password_hash, default_shipping_address, default_payment_info_id) VALUES ($1, $2, $3, $4, $5) RETURNING customer_id;",
                    &[&name, &email, &password_hash, &address_id, &payment_info_id],
                )
            })
            .await?
            .get("customer_id"))
    }

    pub async fn owner_exists(conn: &DbConn) -> Result<bool, postgres::error::Error> {
        Ok(!conn
            .run(|c| c.query("SELECT * FROM base.owner;", &[]))
            .await?
            .is_empty())
    }

    pub async fn validate_owner_login<T: AsRef<str>>(
        conn: &DbConn,
        email: T,
        password: T,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let email = email.as_ref().to_owned();
        let password = password.as_ref().to_owned();

        if !owner_exists(conn).await? {
            Ok(email == "admin@local" && password == "default")
        } else {
            let owner = conn
                .run(move |c| {
                    c.query_opt(
                        "SELECT password_hash FROM base.customer WHERE email = $1",
                        &[&email],
                    )
                })
                .await?;

            match owner {
                Some(owner) => {
                    let password_hash = owner.get("password_hash");
                    Ok(bcrypt::verify(password, password_hash)?)
                }
                None => Ok(false),
            }
        }
    }

    pub async fn get_customer_info(
        conn: &DbConn,
        customer_id: PostgresInt,
    ) -> Result<Option<crate::schema::joined::Customer>, postgres::error::Error> {
        Ok(conn
            .run(move |c| {
                c.query_opt(
                    "
                SELECT
                customer_id,
                name,
                email,
                expiry,
                name_on_card,
                def_shipping.street_address AS def_street_address,
                def_shipping.postal_code AS def_postal,
                def_shipping.province AS def_province,
                billing_add.street_address AS bill_street_address,
                billing_add.postal_code AS bill_postal,
                billing_add.province AS bill_province
                FROM
                base.customer AS customer
                INNER JOIN base.address AS def_shipping ON customer.default_shipping_address = def_shipping.address_id
                INNER JOIN base.payment_info AS payment ON customer.default_payment_info_id = payment.payment_info_id
                INNER JOIN base.address AS billing_add ON payment.billing_address_id = billing_add.address_id
                WHERE customer.customer_id = $1;
            ",
                    &[&customer_id],
                )
            })
            .await?
           .map(|row| {
               use crate::schema::joined::Customer;
            let result: Result<Customer, postgres::error::Error> = try { Customer {
                            name: row.try_get("name")?,
                            email: row.try_get("email")?,
                            street_address: row.try_get("def_street_address")?,
                            postal_code: row.try_get("def_postal")?,
                            province: row.try_get("def_province")?,
                            expiry: row.try_get("expiry")?,
                            name_on_card: row.try_get("name_on_card")?,
                            billing_street_address: row.try_get("bill_street_address")?,
                            billing_postal_code: row.try_get("bill_postal")?,
                            billing_province: row.try_get("bill_province")?,
            }};
               result.ok()
           }).flatten())
    }

    pub async fn get_customer_cart(
        conn: &DbConn,
        customer_id: PostgresInt,
    ) -> Result<Vec<(ISBN, i32)>, postgres::error::Error> {
        Ok(conn
            .run(move |c| {
                c.query(
                    "SELECT isbn, quantity FROM base.in_cart WHERE customer_id = $1",
                    &[&customer_id],
                )
            })
            .await?
            .iter()
            .filter_map(|row| {
                let result: Result<(ISBN, i32), postgres::error::Error> =
                    try { (row.try_get("isbn")?, row.try_get("quantity")?) };
                result.ok()
            })
            .collect())
    }

    pub async fn add_to_cart(
        conn: &DbConn,
        customer_id: PostgresInt,
        isbn: ISBN,
    ) -> Result<(), postgres::error::Error> {
        let cart_row = conn
            .run(move |c| {
                c.query_opt(
                    "SELECT quantity FROM base.in_cart WHERE customer_id = $1 AND isbn = $2",
                    &[&customer_id, &isbn],
                )
            })
            .await?;

        match cart_row {
            Some(_) => {
                conn.run(move |c| c.execute("UPDATE base.in_cart SET quantity = quantity + 1 WHERE isbn = $1 AND customer_id = $2", &[&isbn, &customer_id]))
                    .await?;
                Ok(())
            }
            None => {
                conn.run(move |c| {
                    c.execute(
                        "INSERT INTO base.in_cart (isbn, customer_id, quantity) VALUES ($1, $2, 1)",
                        &[&isbn, &customer_id],
                    )
                })
                .await?;
                Ok(())
            }
        }
    }

    pub async fn cart_set_book_quantity(
        conn: &DbConn,
        customer_id: PostgresInt,
        isbn: ISBN,
        quantity: u32,
    ) -> Result<(), CartError> {
        let stock: PostgresInt = conn
            .run(move |c| c.query_one("SELECT stock FROM base.book WHERE isbn = $1", &[&isbn]))
            .await?
            .try_get("stock")?;

        let quantity = u32::max(quantity, 0) as i32;

        if quantity > stock {
            Err(NotEnoughStockError::new())?;
        }

        if quantity == 0 {
            conn.run(move |c| {
                c.execute(
                    "DELETE FROM base.in_cart WHERE isbn = $1 AND customer_id = $2;",
                    &[&isbn, &customer_id],
                )
            })
            .await?;
        } else {
            conn.run(move |c| {
                c.execute(
                    "UPDATE base.in_cart SET quantity = $1 WHERE isbn = $2 AND customer_id = $3;",
                    &[&quantity, &isbn, &customer_id],
                )
            })
            .await?;
        }
        Ok(())
    }
}
