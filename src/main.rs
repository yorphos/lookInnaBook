#[macro_use]
extern crate rocket;

mod schema;

use rocket_sync_db_pools::{database, postgres};

#[database("postgres")]
struct DbConn(postgres::Client);

#[get("/books")]
async fn books(conn: DbConn) -> String {
    let mut result = String::new();
    if let Ok(v) = conn.run(|c| c.query("SELECT * FROM base.book", &[])).await {
        for row in v {
            result += &row.get::<&str, String>("title");
            result += "\n";
        }
    }

    result
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![books])
        .attach(DbConn::fairing())
}
