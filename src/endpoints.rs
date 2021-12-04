use std::collections::HashMap;

use crate::db::conn::DbConn;
use crate::db::query::{
    get_books, get_customer_info, try_create_new_customer, validate_customer_login, Expiry,
};
use crate::request_guards::state::SessionType;
use crate::schema::no_id;
use chrono::{Duration, Local};
use rand::{RngCore, SeedableRng};
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar};
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::Template;

use crate::{request_guards::*, SessionTokenState};

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
pub async fn customer_page(cust: Customer, conn: DbConn) -> Template {
    let mut context = HashMap::<&str, String>::new();

    let customer_info = match get_customer_info(&conn, cust.customer_id).await {
        Ok(v) => match v {
            Some(v) => v,
            None => {
                context.insert("error", "Server error: No such customer".to_string());
                return Template::render("error", &context);
            }
        },
        Err(e) => {
            context.insert("error", format!("Server error: {}", e));
            return Template::render("error", &context);
        }
    };

    context.insert("name", customer_info.name);

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
pub async fn login(
    conn: DbConn,
    login_data: Form<Login<'_>>,
    session_tokens: &State<SessionTokenState>,
    cookies: &CookieJar<'_>,
) -> Redirect {
    match validate_customer_login(&conn, login_data.email, login_data.password).await {
        Ok(customer_id) => {
            let mut rng = rand_chacha::ChaCha12Rng::from_entropy();
            let mut token: [u8; 32] = [0; 32];
            rng.fill_bytes(&mut token);

            let token = base64::encode(token);

            cookies.add_private(Cookie::new(CUST_SESSION_COOKIE_NAME, token.clone()));

            let mut session_tokens = session_tokens.lock().await;

            let expiry = Local::now() + Duration::days(30);

            session_tokens.insert(token, (SessionType::Customer(customer_id), expiry));
            Redirect::to(uri!(customer_page()))
        }
        Err(e) => match e {
            _ => Redirect::to(uri!(login_failed("Invalid email/password"))),
        },
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

    let address = no_id::Address::new(street_address, postal_code, province);
    let billing_address = no_id::Address::new(
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
        no_id::PaymentInfo::new(name_on_card, expiry, card_number, cvv, billing_address);

    match try_create_new_customer(&conn, email, password, name, address, payment_info).await {
        Ok(_) => Redirect::to(uri!(customer_page())),
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
