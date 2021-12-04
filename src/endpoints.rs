use std::collections::HashMap;

use crate::db::conn::DbConn;
use crate::db::query::{
    get_books, try_create_new_customer, validate_customer_login, AddressInsert, Expiry,
    PaymentInfoInsert,
};
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

#[get("/login/failed/<e>")]
pub async fn login_failed(e: &str) -> Template {
    let mut context = HashMap::<&str, String>::new();
    context.insert("error", format!("Login Failed: {}", e));
    Template::render("error", &context)
}

#[get("/register")]
pub async fn register_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("registration", &context)
}

#[get("/register/failed/<reason>")]
pub async fn register_failed(reason: &str) -> Template {
    let mut context = HashMap::<&str, String>::new();
    context.insert("error", format!("Registration Failed: {}", reason));
    Template::render("error", &context)
}

#[get("/customer")]
pub async fn customer_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("customer", &context)
}

#[get("/owner")]
pub async fn owner_page() -> Template {
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
    match validate_customer_login(&conn, login_data.email, login_data.password).await {
        Ok(v) => match v {
            false => Redirect::to(uri!(login_failed("Invalid Email/Password"))),
            true => Redirect::to(uri!(customer_page())),
        },
        Err(e) => Redirect::to(uri!(login_failed("Server error occured"))),
    }
}

#[derive(FromForm)]
pub struct Register<'r> {
    email: &'r str,
    name: &'r str,
    password: &'r str,
    street_address: &'r str,
    postal_code: &'r str,
    province: &'r str,
    name_on_card: &'r str,
    card_number: &'r str,
    expiry: &'r str,
    cvv: &'r str,
    billing_street_address: &'r str,
    billing_postal_code: &'r str,
    billing_province: &'r str,
}

#[post("/register", data = "<register_data>")]
pub async fn register(conn: DbConn, register_data: Form<Register<'_>>) -> Redirect {
    let Register {
        email,
        name,
        password,
        street_address,
        postal_code,
        province,
        name_on_card,
        card_number,
        expiry,
        cvv,
        billing_street_address,
        billing_postal_code,
        billing_province,
    } = *register_data;

    let address = AddressInsert::new(street_address, postal_code, province);
    let billing_address = AddressInsert::new(
        billing_street_address,
        billing_postal_code,
        billing_province,
    );

    let expiry = if let Some(e) = Expiry::from_str(expiry) {
        e
    } else {
        return Redirect::to(uri!(register_failed(format!(
            "{:?}",
            "Invalid Credit Card Expiry"
        ))));
    };

    let payment_info =
        PaymentInfoInsert::new(name_on_card, expiry, card_number, cvv, billing_address);

    match try_create_new_customer(&conn, email, password, name, address, payment_info).await {
        Ok(customer_id) => Redirect::to(uri!(customer_page())),
        Err(e) => Redirect::to(uri!(register_failed(format!("{:?}", e)))),
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
