#![feature(try_blocks)]
#[macro_use]
extern crate rocket;

mod db;
mod endpoints;
mod request_guards;
mod schema;

use std::sync::Arc;

use db::conn::DbConn;
use endpoints::*;
use rocket::{fs::FileServer, futures::lock::Mutex};
use rocket_dyn_templates::Template;

use request_guards::state::SessionTokens;

pub type SessionTokenState = Arc<Mutex<SessionTokens>>;

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount(
            "/",
            routes![
                book,
                index,
                login,
                login_page,
                login_failed,
                customer_page,
                owner_page,
                register,
                register_page,
                register_failed,
                customer_cart_page,
                customer_cart_add,
            ],
        )
        .mount("/style", FileServer::from("style/"))
        .mount("/images", FileServer::from("image/"))
        .manage(SessionTokenState::new(Mutex::new(SessionTokens::new())))
        .attach(DbConn::fairing())
        .attach(Template::fairing())
}
