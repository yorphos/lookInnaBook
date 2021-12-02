use std::collections::HashMap;

use crate::db::conn::DbConn;
use crate::db::query::{get_books, validate_login, LoginType};
use crate::schema::entities::PostgresInt;
use rocket::form::Form;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;

#[get("/")]
pub async fn index(conn: DbConn) -> Template {
    let books = get_books(&conn).await;
    if let Ok(books) = books {
        let mut context = HashMap::new();
        context.insert("books", books);
        Template::render("index", &context)
    } else {
        let mut context = HashMap::new();
        context.insert("error", format!("Could not query books: {:?}", books));
        Template::render("error", &context)
    }
}

#[get("/login")]
pub async fn login_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("login", &context)
}

#[get("/customer/<customer_id>")]
pub async fn customer_page(customer_id: PostgresInt) -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("customer", &context)
}

#[get("/owner/<owner_id>")]
pub async fn owner_page(owner_id: PostgresInt) -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("owner", &context)
}

#[derive(FromForm)]
pub struct Login<'r> {
    email: &'r str,
    password: &'r str,
}

#[post("/login", data = "<login_data>")]
pub async fn login(conn: DbConn, login_data: Form<Login<'_>>) -> Redirect {
    match validate_login(&conn, login_data.email, login_data.password).await {
        Ok(v) => {
            use LoginType::*;
            match v {
                FailedLogin => Redirect::to(uri!("/login/failed")),
                CustomerLogin(customer_id) => Redirect::to(uri!(customer_page(customer_id))),
                OwnerLogin(customer_id) => Redirect::to(uri!(owner_page(customer_id))),
            }
        }
        Err(e) => Redirect::to(uri!("/login/failed")),
    }
}

#[get("/book/<isbn>")]
pub async fn book(conn: DbConn, isbn: &str) -> Template {
    match isbn.parse::<i32>() {
        Ok(isbn) => {
            let books = get_books(&conn).await;

            match books {
                Ok(books) => match books.iter().find(|book| book.isbn == isbn) {
                    Some(book) => Template::render("book", &book),
                    None => {
                        let mut context = HashMap::new();
                        context.insert("error", format!("No book with ISBN {}", isbn));
                        Template::render("error", &context)
                    }
                },
                Err(e) => {
                    let mut context = HashMap::new();
                    context.insert("error", format!("{}", e));
                    Template::render("error", &context)
                }
            }
        }
        Err(_) => {
            let mut context = HashMap::new();
            context.insert("error", format!("ISBN {} is not a valid ISBN", isbn));
            Template::render("error", &context)
        }
    }
}
