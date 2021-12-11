use chrono::Local;
use rocket::{http, outcome::Outcome, request::FromRequest};

use crate::{db::query::does_owner_exist, schema::entities::PostgresInt};

use self::state::{SessionTokens, SessionType};

pub mod state {
    use chrono::Local;

    use crate::schema::entities::PostgresInt;
    use chrono::DateTime;
    use std::collections::HashMap;

    pub enum SessionType {
        Customer(PostgresInt),
        Owner(PostgresInt),
        DefaultOwner,
    }

    pub type ExpirationTime = DateTime<Local>;
    pub type SessionTokens = HashMap<String, (SessionType, ExpirationTime)>;
}

pub const CUST_SESSION_COOKIE_NAME: &str = "lookinnabook_custsession";
pub const OWNER_SESSION_COOKIE_NAME: &str = "lookinnabook_ownersession";

fn does_owner_session_token_exist(session_tokens: &SessionTokens) -> bool {
    session_tokens
        .values()
        .filter(|(session_type, _)| {
            if let SessionType::Owner(_) = &session_type {
                true
            } else {
                false
            }
        })
        .count()
        > 0
}

#[derive(Clone, Copy)]
pub enum OwnerType {
    DefaultOwner,
    OwnerAccount(PostgresInt),
}

#[derive(Clone, Copy)]
pub struct Owner {
    pub owner: OwnerType,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Owner {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let result: Result<Owner, ()> = try {
            let session_token_lock = request
                .rocket()
                .state::<crate::SessionTokenState>()
                .ok_or(())?;

            let mut session_tokens = session_token_lock.lock().await;
            let cookies = request.cookies();

            let owner_session_cookie = cookies.get_private(OWNER_SESSION_COOKIE_NAME).ok_or(())?;

            let (session_type, expiration_time) =
                session_tokens.get(owner_session_cookie.value()).ok_or(())?;
            if Local::now() > expiration_time.clone() {
                session_tokens.remove(owner_session_cookie.value());
                Err(())?
            } else {
                use state::SessionType;
                match session_type {
                    &SessionType::Owner(owner_id) => Owner {
                        owner: OwnerType::OwnerAccount(owner_id),
                    },
                    &SessionType::DefaultOwner => {
                        let conn = request.rocket().state::<crate::DbConn>().ok_or(())?;

                        // Short circuit so we avoid DB access if possible
                        if does_owner_session_token_exist(&session_tokens) {
                            Err(())?
                        } else {
                            let does_owner_exist = does_owner_exist(conn).await.unwrap_or(true);

                            if does_owner_exist {
                                Err(())?
                            }

                            Owner {
                                owner: OwnerType::DefaultOwner,
                            }
                        }
                    }
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

#[derive(Clone, Copy)]
pub struct Customer {
    pub customer_id: PostgresInt,
}

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
