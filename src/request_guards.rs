use chrono::Local;
use rocket::{http, outcome::Outcome, request::FromRequest};

use crate::schema::entities::PostgresInt;

pub mod state {
    use chrono::Local;

    use crate::schema::entities::PostgresInt;
    use chrono::DateTime;
    use std::collections::HashMap;

    pub enum SessionType {
        Customer(PostgresInt),
        Owner(PostgresInt),
        DefaultOwner(PostgresInt),
    }

    pub type ExpirationTime = DateTime<Local>;
    pub type SessionTokens = HashMap<String, (SessionType, ExpirationTime)>;
}

pub struct Customer {
    pub customer_id: PostgresInt,
}

pub const CUST_SESSION_COOKIE_NAME: &str = "lookinnabook_custsession";

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Customer {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let result: Result<Customer, ()> = try {
            let session_token_lock = request
                .rocket()
                .state::<crate::SessionTokenState>()
                .ok_or(())?;
            let mut session_tokens = session_token_lock.lock().await;
            let cookies = request.cookies();

            let cust_session_cookie = cookies.get_private(CUST_SESSION_COOKIE_NAME).ok_or(())?;

            let (session_type, expiration_time) =
                session_tokens.get(cust_session_cookie.value()).ok_or(())?;
            if Local::now() > expiration_time.clone() {
                session_tokens.remove(cust_session_cookie.value());
                Err(())?
            } else {
                use state::SessionType;
                match session_type {
                    &SessionType::Customer(customer_id) => Customer { customer_id },
                    &_ => Err(())?,
                }
            }
        };
        match result {
            Ok(customer) => Outcome::Success(customer),
            Err(_) => Outcome::Failure((http::Status::Forbidden, ())),
        }
    }
}
