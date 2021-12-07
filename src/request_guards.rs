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
        match request.rocket().state::<crate::SessionTokenState>() {
            Some(session_token_lock) => {
                let mut session_tokens = session_token_lock.lock().await;
                let cookies = request.cookies();

                match cookies.get_private(CUST_SESSION_COOKIE_NAME) {
                    Some(cookie) => match session_tokens.get(cookie.value()) {
                        Some((session_type, expiration_time)) => {
                            if Local::now() > expiration_time.clone() {
                                session_tokens.remove(cookie.value());
                                Outcome::Failure((http::Status::Forbidden, ()))
                            } else {
                                use state::SessionType;
                                match session_type {
                                    &SessionType::Customer(customer_id) => {
                                        Outcome::Success(Customer { customer_id })
                                    }
                                    &_ => Outcome::Failure((http::Status::Forbidden, ())),
                                }
                            }
                        }
                        None => Outcome::Failure((http::Status::Forbidden, ())),
                    },
                    None => Outcome::Failure((http::Status::Forbidden, ())),
                }
            }
            None => Outcome::Failure((http::Status::InternalServerError, ())),
        }
    }
}

pub enum OptionCustomer {
    SomeCustomer(Customer),
    NoCustomer,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OptionCustomer {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        match Customer::from_request(request).await {
            Outcome::Success(v) => Outcome::Success(OptionCustomer::SomeCustomer(v)),
            Outcome::Forward(v) => Outcome::Forward(v),
            Outcome::Failure(_) => Outcome::Success(OptionCustomer::NoCustomer),
        }
    }
}
