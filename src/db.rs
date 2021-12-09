pub mod conn {
    use rocket_sync_db_pools::{database, postgres};

    #[database("postgres")]
    pub struct DbConn(postgres::Client);
}

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Clone, Error)]
    pub struct StateError {
        msg: String,
    }

    impl StateError {
        pub fn new<T: AsRef<str>>(msg: T) -> StateError {
            StateError {
                msg: msg.as_ref().to_owned(),
            }
        }
    }

    impl std::fmt::Display for StateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    #[derive(Debug, Error)]
    pub enum CartError {
        #[error("Insufficient stock for quantity change")]
        NotEnoughStock,
        #[error("Internal DB error: `{0}`")]
        DBError(#[from] postgres::error::Error),
    }

    #[derive(Debug, Error)]
    pub enum OrderError {
        #[error("Insufficient stock for order")]
        NotEnoughStock,
        #[error("Internal DB error: `{0}`")]
        DBError(#[from] postgres::error::Error),
        #[error("Internal state error: `{0}`")]
        StateError(#[from] StateError),
    }

    #[derive(Debug, Error)]
    pub enum LoginError {
        #[error("Internal DB error: `{0}`")]
        DBError(#[from] postgres::error::Error),
        #[error("Invalid email/password")]
        CredentialError,
        #[error("Internal bcrypt error")]
        BCryptError(#[from] bcrypt::BcryptError),
    }
}

pub mod query {
    use super::conn::DbConn;
    use super::error::CartError;
    use super::error::LoginError;
    use super::error::OrderError;
    use super::error::StateError;
    use crate::schema;
    use crate::schema::entities::*;
    use crate::schema::joined::Order;
    use crate::schema::joined::OrderNoBooks;
    use crate::schema::no_id;
    use crate::schema::no_id::Address;
    use crate::schema::no_id::PaymentInfo;
    use chrono::Local;
    use chrono::NaiveDate;
    use rand::RngCore;
    use serde::Serialize;

    pub async fn get_books(conn: &DbConn) -> Result<Vec<Book>, postgres::error::Error> {
        let rows = conn
            .run(|c| c.query("SELECT * FROM base.book", &[]))
            .await?;
        Ok(rows.iter().flat_map(|row| Book::from_row(row)).collect())
    }

    pub async fn get_books_with_publisher_name(
        conn: &DbConn,
    ) -> Result<Vec<BookWithPublisherName>, postgres::error::Error> {
        let rows = conn
            .run(|c| c.query("
                            SELECT
                            isbn,
                            title,
                            author_name,
                            genre,
                            base.book.publisher_id,
                            company_name AS publisher_name,
                            num_pages,
                            price,
                            author_royalties,
                            reorder_threshold,
                            stock,
                            discontinued,
                            company_name AS publisher_name
                            FROM base.book INNER JOIN base.publisher ON base.book.publisher_id = base.publisher.publisher_id;",
                             &[]))
            .await?;
        Ok(rows
            .iter()
            .flat_map(|row| BookWithPublisherName::from_row(row))
            .collect())
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
                    Err(LoginError::CredentialError)?
                }
            }
            None => Err(LoginError::CredentialError)?,
        }
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

    pub enum OwnerLoginType {
        DefaultOwner,
        OwnerAccount(PostgresInt),
    }

    pub async fn validate_owner_login<T: AsRef<str>>(
        conn: &DbConn,
        email: T,
        password: T,
    ) -> Result<OwnerLoginType, LoginError> {
        let email = email.as_ref().to_owned();
        let password = password.as_ref().to_owned();

        if !owner_exists(conn).await? {
            if email == "admin@local" && password == "default" {
                Ok(OwnerLoginType::DefaultOwner)
            } else {
                Err(LoginError::CredentialError)
            }
        } else {
            let row = conn
                .run(move |c| {
                    c.query_opt(
                        "SELECT owner_id, password_hash FROM base.owner WHERE email = $1",
                        &[&email],
                    )
                })
                .await?;

            if let Some(row) = row {
                let owner_id = row.try_get("owner_id")?;
                let password_hash = row.try_get("password_hash")?;

                if bcrypt::verify(password, password_hash)? {
                    Ok(OwnerLoginType::OwnerAccount(owner_id))
                } else {
                    Err(LoginError::CredentialError)
                }
            } else {
                Err(LoginError::CredentialError)
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
            Err(CartError::NotEnoughStock)?;
        }

        let quantity = quantity as i32;

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

    pub async fn clear_cart(
        conn: &DbConn,
        customer_id: PostgresInt,
    ) -> Result<(), postgres::error::Error> {
        conn.run(move |c| {
            c.execute(
                "DELETE FROM base.in_cart WHERE customer_id = $1",
                &[&customer_id],
            )
        })
        .await?;

        Ok(())
    }

    async fn add_books_to_order(
        conn: &DbConn,
        books: Vec<(ISBN, PostgresInt)>,
        order_id: PostgresInt,
    ) -> Result<(), postgres::error::Error> {
        for (isbn, quantity) in books {
            conn.run(move |c| {
                c.execute(
                    "
                    INSERT INTO base.in_order
                    VALUES ($1, $2, $3);
                    ",
                    &[&isbn, &order_id, &quantity],
                )
            })
            .await?;
        }
        Ok(())
    }

    async fn remove_book_stock(
        conn: &DbConn,
        books: Vec<(ISBN, PostgresInt)>,
    ) -> Result<(), postgres::error::Error> {
        for (isbn, quantity) in books {
            conn.run(move |c| {
                c.execute(
                    "UPDATE base.book SET stock = stock - $1 WHERE isbn = $2;",
                    &[&quantity, &isbn],
                )
            })
            .await?;
        }

        Ok(())
    }

    pub async fn create_order(
        conn: &DbConn,
        customer_id: PostgresInt,
        books: Vec<(ISBN, u32)>,
        address: Option<schema::no_id::Address>,
        payment_info: Option<schema::no_id::PaymentInfo>,
    ) -> Result<PostgresInt, OrderError> {
        let books: Vec<(ISBN, PostgresInt)> = books
            .into_iter()
            .map(|(isbn, quantity)| (isbn, quantity as i32))
            .collect();
        for (isbn, quantity) in books.iter() {
            let quantity = i32::max(*quantity, 0) as u32;
            if !check_enough_stock(conn, *isbn, quantity).await? {
                Err(OrderError::NotEnoughStock)?;
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

        let order_id: PostgresInt = conn.run(move |c| {
            c.query_one(
                "
                INSERT INTO base.orders
                (customer_id, shipping_address_id, tracking_number, order_status, order_date, payment_info_id)
                VALUES
                ($1, $2, $3, $4, $5, $6)
                RETURNING order_id;
                ",
                &[&customer_id, &address_id, &tracking_number, &"PR", &Local::today().naive_local(), &payment_info_id],
            )
        })
        .await?.try_get("order_id")?;

        add_books_to_order(conn, books.clone(), order_id).await?;
        remove_book_stock(conn, books).await?;

        clear_cart(conn, customer_id).await?;

        Ok(order_id)
    }

    pub async fn get_books_for_order(
        conn: &DbConn,
        order: OrderNoBooks,
    ) -> Result<Order, postgres::error::Error> {
        let books = conn.run(move |c| {
            c.query("SELECT * FROM base.in_order INNER JOIN base.book ON base.in_order.isbn = base.book.isbn WHERE order_id = $1;", &[&order.order_id])
        }).await?.iter().flat_map(|row| {
            let result: Result<(Book, u32), OrderError> = try {
                let quantity: i32 = row.try_get("quantity")?;
                (Book::from_row(row)?, i32::max(quantity, 0) as u32)
            };

            result.ok()
        }).collect();

        Ok(Order::from_order_with_id(order, books))
    }

    pub async fn get_order_info(
        conn: &DbConn,
        order_id: PostgresInt,
    ) -> Result<OrderNoBooks, OrderError> {
        let row = conn.run(move |c| c.query_one(
            "
            SELECT
            order_id,
            add.street_address,
            add.postal_code,
            add.province,
            bill.street_address AS bill_street_address,
            bill.postal_code AS bill_postal_code,
            bill.province AS bill_province,
            order_status,
            order_date,
            tracking_number,
            name_on_card,
            card_number,
            expiry,
            cvv
            FROM
            base.orders AS orders
            INNER JOIN base.address AS add ON orders.shipping_address_id = add.address_id
            INNER JOIN base.payment_info AS payment ON orders.payment_info_id = payment.payment_info_id
            INNER JOIN base.address AS bill ON payment.billing_address_id = bill.address_id
            WHERE order_id = $1;
            ",
            &[&order_id])).await?;

        let address = Address::new::<&str>(
            row.try_get("street_address")?,
            row.try_get("postal_code")?,
            row.try_get("province")?,
        );
        let billing_address = Address::new::<&str>(
            row.try_get("bill_street_address")?,
            row.try_get("bill_postal_code")?,
            row.try_get("bill_province")?,
        );
        let expiry = Expiry::from_str::<&str>(row.try_get("expiry")?)
            .ok_or(StateError::new("Invalid expiry"))?;
        let payment_info = PaymentInfo::new::<&str>(
            row.try_get("name_on_card")?,
            expiry,
            row.try_get("card_number")?,
            row.try_get("cvv")?,
            billing_address,
        );

        let date: NaiveDate = row.try_get("order_date")?;

        let order = OrderNoBooks {
            order_id: row.try_get("order_id")?,
            shipping_address: address,
            tracking_number: row.try_get("tracking_number")?,
            order_status: row.try_get("order_status")?,
            order_date: date.to_string(),
            payment_info,
        };

        Ok(order)
    }

    pub async fn get_customer_orders_info(
        conn: &DbConn,
        customer_id: PostgresInt,
    ) -> Result<Vec<schema::joined::Order>, postgres::error::Error> {
        let orders_no_books: Vec<OrderNoBooks> = conn.run(move |c| c.query(
            "
            SELECT
            order_id,
            add.street_address,
            add.postal_code,
            add.province,
            bill.street_address AS bill_street_address,
            bill.postal_code AS bill_postal_code,
            bill.province AS bill_province,
            order_status,
            order_date,
            tracking_number,
            name_on_card,
            card_number,
            expiry,
            cvv
            FROM
            base.orders AS orders
            INNER JOIN base.address AS add ON orders.shipping_address_id = add.address_id
            INNER JOIN base.payment_info AS payment ON orders.payment_info_id = payment.payment_info_id
            INNER JOIN base.address AS bill ON payment.billing_address_id = bill.address_id
            WHERE customer_id = $1;
            ",
            &[&customer_id])).await?.iter()
           .filter_map(
            |row| {
                let result: Result<OrderNoBooks, OrderError> = try {
                    let address = Address::new::<&str>(row.try_get("street_address")?, row.try_get("postal_code")?, row.try_get("province")?);
                    let billing_address = Address::new::<&str>(row.try_get("bill_street_address")?, row.try_get("bill_postal_code")?, row.try_get("bill_province")?);
                    let expiry = Expiry::from_str::<&str>(row.try_get("expiry")?).ok_or(StateError::new("Invalid expiry"))?;
                    let payment_info = PaymentInfo::new::<&str>(row.try_get("name_on_card")?, expiry, row.try_get("card_number")?, row.try_get("cvv")?, billing_address);

                    let date: NaiveDate = row.try_get("order_date")?; 

                    OrderNoBooks {
                        order_id: row.try_get("order_id")?,
                        shipping_address: address,
                        tracking_number: row.try_get("tracking_number")?,
                        order_status: row.try_get("order_status")?,
                        order_date: date.to_string(),
                        payment_info,
                    }
                };

                result.ok()
        }).collect();

        let mut orders: Vec<Order> = vec![];

        for order in orders_no_books {
            orders.push(get_books_for_order(conn, order).await?);
        }

        Ok(orders)
    }
}
