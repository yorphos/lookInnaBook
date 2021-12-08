pub mod conn {
    use rocket_sync_db_pools::{database, postgres};

    #[database("postgres")]
    pub struct DbConn(postgres::Client);
}

pub mod error {
    use std::error::Error;
    use std::fmt::Display;

    use bcrypt::BcryptError;

    #[derive(Debug, Clone)]
    pub struct StateError {
        what: String,
    }

    impl StateError {
        pub fn new<T: AsRef<str>>(what: T) -> StateError {
            StateError {
                what: what.as_ref().to_owned(),
            }
        }
    }

    impl Error for StateError {}

    impl Display for StateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Invalid application state: {}", self.what)
        }
    }

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

    #[derive(Debug)]
    pub enum OrderError {
        NotEnoughStock(NotEnoughStockError),
        DBError(postgres::error::Error),
        StateError(StateError),
    }

    impl Display for OrderError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::DBError(err) => Display::fmt(&err, f),
                Self::NotEnoughStock(err) => Display::fmt(&err, f),
                Self::StateError(err) => Display::fmt(&err, f),
            }
        }
    }

    impl From<postgres::error::Error> for OrderError {
        fn from(e: postgres::error::Error) -> Self {
            Self::DBError(e)
        }
    }

    impl From<NotEnoughStockError> for OrderError {
        fn from(e: NotEnoughStockError) -> Self {
            Self::NotEnoughStock(e)
        }
    }

    impl From<StateError> for OrderError {
        fn from(e: StateError) -> Self {
            Self::StateError(e)
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
    use super::error::OrderError;
    use super::error::StateError;
    use crate::db::error::CredentialError;
    use crate::schema;
    use crate::schema::entities::*;
    use crate::schema::no_id;
    use chrono::Local;
    use rand::RngCore;
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

    pub async fn get_customer(
        conn: &DbConn,
        customer_id: PostgresInt,
    ) -> Result<Option<crate::schema::entities::Customer>, postgres::error::Error> {
        let row = conn
            .run(move |c| {
                c.query_opt(
                    "SELECT name, email, default_shipping_address, default_payment_info_id
            FROM base.customer WHERE customer_id = $1",
                    &[&customer_id],
                )
            })
            .await?;

        Ok(match row {
            Some(row) => Some(schema::entities::Customer {
                name: row.try_get("name")?,
                email: row.try_get("email")?,
                default_shipping_address_id: row.try_get("default_shipping_address")?,
                default_payment_info_id: row.try_get("default_payment_info_id")?,
            }),
            None => None,
        })
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
    ) -> Result<Vec<(ISBN, u32)>, postgres::error::Error> {
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
            .map(|b| (b.0, i32::max(b.1, 0) as u32))
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

    async fn check_enough_stock(
        conn: &DbConn,
        isbn: ISBN,
        quantity: u32,
    ) -> Result<bool, postgres::error::Error> {
        let stock: PostgresInt = conn
            .run(move |c| c.query_one("SELECT stock FROM base.book WHERE isbn = $1", &[&isbn]))
            .await?
            .try_get("stock")?;

        let quantity = u32::max(quantity, 0) as i32;
        Ok(quantity < stock)
    }

    pub async fn cart_set_book_quantity(
        conn: &DbConn,
        customer_id: PostgresInt,
        isbn: ISBN,
        quantity: u32,
    ) -> Result<(), CartError> {
        if !check_enough_stock(conn, isbn, quantity).await? {
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

    async fn find_address(
        conn: &DbConn,
        address: schema::no_id::Address,
    ) -> Result<Option<PostgresInt>, postgres::error::Error> {
        let schema::no_id::Address {
            street_address,
            postal_code,
            province,
        } = address;

        let row = conn.run(move |c| {
            c.query_opt("SELECT address_id FROM base.address WHERE street_address = $1 AND postal_code = $2 AND province = $3", &[&street_address, &postal_code, &province])
        }).await?;

        match row {
            Some(row) => Ok(row.try_get("address_id")?),
            None => Ok(None),
        }
    }

    async fn find_payment_info(
        conn: &DbConn,
        payment_info: schema::no_id::PaymentInfo,
    ) -> Result<Option<PostgresInt>, postgres::error::Error> {
        let schema::no_id::PaymentInfo {
            card_number,
            name_on_card,
            expiry,
            cvv,
            billing_address,
        } = payment_info;

        let schema::no_id::Address {
            street_address,
            postal_code,
            province,
        } = billing_address;

        let row = conn
            .run(move |c| {
                c.query_opt(
                    "
                SELECT payment_info_id FROM
                base.payment_info INNER JOIN
                base.address ON billing_address_id = address_id WHERE
                name_on_card = $1 AND
                expiry = $2 AND
                card_number = $3 AND 
                cvv = $4 AND
                street_address = $5 AND
                postal_code = $6 AND
                province = $7;
                ",
                    &[
                        &name_on_card,
                        &expiry.to_string(),
                        &card_number,
                        &cvv,
                        &street_address,
                        &postal_code,
                        &province,
                    ],
                )
            })
            .await?;

        Ok(match row {
            Some(row) => Some(row.try_get("payment_info_id")?),
            None => None,
        })
    }

    async fn get_or_insert_address(
        conn: &DbConn,
        address: schema::no_id::Address,
    ) -> Result<PostgresInt, postgres::error::Error> {
        match find_address(conn, address.clone()).await? {
            Some(address_id) => Ok(address_id),
            None => {
                let schema::no_id::Address {
                    street_address,
                    postal_code,
                    province,
                } = address;

                Ok(conn
                    .run(move |c| {
                        c.query_one(
                            "INSERT INTO base.address
                    (street_address, postal_code, province)
                    VALUES ($1, $2, $3)
                    RETURNING address_id;",
                            &[&street_address, &postal_code, &province],
                        )
                    })
                    .await?
                    .try_get("address_id")?)
            }
        }
    }

    async fn get_or_insert_payment_info(
        conn: &DbConn,
        payment_info: schema::no_id::PaymentInfo,
    ) -> Result<PostgresInt, postgres::error::Error> {
        match find_payment_info(conn, payment_info.clone()).await? {
            Some(payment_info_id) => Ok(payment_info_id),
            None => {
                let schema::no_id::PaymentInfo {
                    card_number,
                    name_on_card,
                    expiry,
                    cvv,
                    billing_address,
                } = payment_info;

                let address_id = get_or_insert_address(conn, billing_address).await?;

                Ok(conn
                    .run(move |c| {
                        c.query_one(
                            "INSERT INTO base.payment_info
                    (name_on_card, expiry, card_number, cvv, billing_address_id)
                    VALUES ($1, $2, $3, $4)
                    RETURNING payment_info_id;",
                            &[
                                &name_on_card,
                                &expiry.to_string(),
                                &card_number,
                                &cvv,
                                &address_id,
                            ],
                        )
                    })
                    .await?
                    .try_get("payment_info_id")?)
            }
        }
    }

    fn get_tracking_number() -> String {
        let mut rng = rand::thread_rng();
        rng.next_u32().to_string()
    }

    pub async fn create_order(
        conn: &DbConn,
        customer_id: PostgresInt,
        books: Vec<(ISBN, u32)>,
        address: Option<schema::no_id::Address>,
        payment_info: Option<schema::no_id::PaymentInfo>,
    ) -> Result<(), OrderError> {
        for (isbn, quantity) in books {
            if !check_enough_stock(conn, isbn, quantity).await? {
                Err(NotEnoughStockError::new())?;
            }
        }

        let address_id = match address {
            Some(address) => get_or_insert_address(conn, address).await?,
            None => {
                let customer = get_customer(conn, customer_id).await?;
                customer
                    .ok_or(StateError::new(format!(
                        "No customer with the ID ({})",
                        customer_id
                    )))?
                    .default_shipping_address_id
            }
        };

        let payment_info_id = match payment_info {
            Some(payment_info) => get_or_insert_payment_info(conn, payment_info).await?,
            None => {
                let customer = get_customer(conn, customer_id).await?;
                customer
                    .ok_or(StateError::new(format!(
                        "No customer with the ID ({})",
                        customer_id
                    )))?
                    .default_payment_info_id
            }
        };

        let tracking_number = get_tracking_number();

        conn.run(move |c| {
            c.execute(
                "
                INSERT INTO base.orders
                (customer_id, shipping_address_id, tracking_number, order_status, order_date, payment_info_id)
                VALUES
                ($1, $2, $3, $4, $5, $6)
                ",
                &[&customer_id, &address_id, &tracking_number, &"PR", &Local::today().naive_local(), &payment_info_id],
            )
        })
        .await?;

        Ok(())
    }
}
