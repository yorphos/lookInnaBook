#[macro_use]
extern crate rocket;

mod db;
mod endpoints;
mod schema;

use db::conn::DbConn;
use endpoints::*;
use rocket::fs::FileServer;
use rocket_dyn_templates::Template;

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
            ],
        )
        .mount("/style", FileServer::from("style/"))
        .mount("/images", FileServer::from("image/"))
        .attach(DbConn::fairing())
        .attach(Template::fairing())
}
