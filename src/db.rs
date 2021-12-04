pub mod conn {
    use rocket_sync_db_pools::{database, postgres};

    #[database("postgres")]
    pub struct DbConn(postgres::Client);
}

pub mod query {
    use postgres::GenericClient;

    use super::conn::DbConn;
    use crate::schema::entities::*;

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
    ) -> Result<bool, postgres::error::Error> {
        Ok(false)
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

    pub struct AddressInsert {
        street_address: String,
        postal_code: String,
        province: String,
    }

    impl AddressInsert {
        pub fn new<T: AsRef<str>>(street_address: T, postal_code: T, province: T) -> AddressInsert {
            AddressInsert {
                street_address: street_address.as_ref().to_string(),
                postal_code: postal_code.as_ref().to_string(),
                province: province.as_ref().to_string(),
            }
        }
    }

    pub async fn try_add_address(
        conn: &DbConn,
        address: AddressInsert,
    ) -> Result<PostgresInt, postgres::error::Error> {
        let AddressInsert {
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

    pub struct PaymentInfoInsert {
        name_on_card: String,
        expiry: Expiry,
        card_number: String,
        cvv: String,
        billing_address: AddressInsert,
    }

    impl PaymentInfoInsert {
        pub fn new<T: AsRef<str>>(
            name_on_card: T,
            expiry: Expiry,
            card_number: T,
            cvv: T,
            billing_address: AddressInsert,
        ) -> PaymentInfoInsert {
            PaymentInfoInsert {
                name_on_card: name_on_card.as_ref().to_string(),
                expiry,
                card_number: card_number.as_ref().to_string(),
                cvv: cvv.as_ref().to_string(),
                billing_address,
            }
        }
    }

    pub async fn try_add_payment_info(
        conn: &DbConn,
        payment_info: PaymentInfoInsert,
    ) -> Result<PostgresInt, postgres::error::Error> {
        let PaymentInfoInsert {
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
        address: AddressInsert,
        payment_info: PaymentInfoInsert,
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
}
