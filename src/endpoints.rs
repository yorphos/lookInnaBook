use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::db::conn::DbConn;
use crate::db::error::{CartError, OrderError, StateError};
use crate::db::query::{
    add_to_cart, cart_set_book_quantity, create_book, delete_customer_account,
    delete_owner_account, discontinue_books, get_books, get_books_for_order,
    get_books_with_publisher_name, get_customer_accounts, get_customer_cart, get_customer_info,
    get_customer_orders_info, get_order_info, get_owner_accounts, get_publishers,
    get_sales_by_date, get_sales_by_publisher, try_create_new_customer, try_create_new_owner,
    try_create_publisher, undiscontinue_books, validate_customer_login, validate_owner_login,
    Expiry, OwnerLoginType,
};
use crate::request_guards::state::SessionType;
use crate::schema::entities::{Book, BookWithPublisherName, PostgresInt, ISBN};
use crate::schema::joined::Order;
use crate::schema::no_id::{Address, PaymentInfo};
use crate::schema::{self, no_id};
use chrono::{Duration, Local, NaiveDate};
use rand::{RngCore, SeedableRng};
use rocket::form::validate::Contains;
use rocket::form::Form;
use rocket::http::{ContentType, Cookie, CookieJar, Status};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::State;
use rocket_dyn_templates::tera::Context;
use rocket_dyn_templates::Template;
use rust_decimal::Decimal;
use serde::Serialize;
use std::str::FromStr;
use strsim::sorensen_dice;

use crate::{request_guards::*, SessionTokenState};

async fn render_error_template<T: AsRef<str>>(
    error: T,
    conn: &DbConn,
    customer: &Option<Customer>,
) -> Template {
    let mut context = Context::new();

    add_customer_info(conn, customer, &mut context).await;

    context.insert("error", error.as_ref());

    Template::render("error", context.into_json())
}

async fn add_customer_info(conn: &DbConn, customer: &Option<Customer>, context: &mut Context) {
    if let Some(customer) = customer {
        if let Ok(customer_info) = get_customer_info(&conn, customer.customer_id).await {
            context.insert("customer", &customer_info);
        } else {
            context.insert("customer", &crate::schema::joined::Customer::default());
        }

        if let Ok(cart) = get_customer_cart(&conn, customer.customer_id).await {
            context.insert("cart_size", &cart.len());
        } else {
            context.insert("cart_size", &0);
        }
    }
}

fn add_owner_tag(owner: &Option<Owner>, context: &mut Context) {
    if let Some(_) = owner {
        context.insert("owner_logged_in", &true);
    }
}

fn extract_genre_list(books: &Vec<BookWithPublisherName>) -> HashSet<String> {
    books.iter().map(|book| book.genre.clone()).collect()
}

#[derive(FromForm, Debug)]
pub struct Search<'r> {
    title: Option<&'r str>,
    isbn: Option<&'r str>,
    genre: Option<&'r str>,
    author: Option<&'r str>,
    publisher: Option<&'r str>,
    min_pages: Option<i32>,
    max_pages: Option<i32>,
    min_price: Option<&'r str>,
    max_price: Option<&'r str>,
    show_discontinued: Option<bool>,
    show_no_stock: Option<bool>,
}

fn filter_books(
    books: Vec<BookWithPublisherName>,
    search: Search<'_>,
) -> Vec<BookWithPublisherName> {
    let Search {
        title,
        isbn,
        genre,
        author,
        publisher,
        min_pages,
        max_pages,
        min_price,
        max_price,
        show_discontinued,
        show_no_stock,
    } = search;
    let mut books = books;

    if !show_discontinued.unwrap_or(false) {
        books.retain(|book| !book.discontinued);
    }

    if !show_no_stock.unwrap_or(false) {
        books.retain(|book| book.stock != 0);
    }

    let isbn = isbn.unwrap_or("");
    if isbn != "" {
        if let Ok(isbn) = isbn.parse::<i32>() {
            books.retain(|book| book.isbn == isbn);
        }
    }

    let genre = genre.unwrap_or("");
    if genre != "" {
        if genre != "N/A" {
            books.retain(|book| book.genre == genre);
        }
    }

    let author = author.unwrap_or("");
    if author != "" {
        books.retain(|book| book.author_name == author);
    }

    let publisher = publisher.unwrap_or("");
    if publisher != "" {
        books.retain(|book| book.publisher_name == publisher);
    }

    if let Some(min_pages) = min_pages {
        books.retain(|book| book.num_pages >= min_pages);
    }

    if let Some(max_pages) = max_pages {
        books.retain(|book| book.num_pages <= max_pages);
    }

    if let Some(min_price) = min_price {
        if let Ok(min_price) = Decimal::from_str(min_price) {
            books.retain(|book| book.price >= min_price);
        }
    }

    if let Some(max_price) = max_price {
        if let Ok(max_price) = Decimal::from_str(max_price) {
            books.retain(|book| book.price <= max_price);
        }
    }

    let title = title.unwrap_or("");
    if title != "" {
        books.sort_by(|a, b| {
            let a_score = sorensen_dice(&a.title, title);
            let b_score = sorensen_dice(&b.title, title);

            // Descending sort
            match b_score.partial_cmp(&a_score) {
                Some(ord) => ord,
                None => Ordering::Equal,
            }
        });
    }

    books
}

#[get("/?<search>")]
pub async fn index(
    conn: DbConn,
    customer: Option<Customer>,
    owner: Option<Owner>,
    search: Search<'_>,
) -> Template {
    let mut context = Context::new();
    add_customer_info(&conn, &customer, &mut context).await;
    add_owner_tag(&owner, &mut context);

    let books = get_books_with_publisher_name(&conn).await;
    if let Ok(books) = books {
        let books = filter_books(books, search);

        context.insert("books", &books);
        context.insert("genres", &extract_genre_list(&books));

        Template::render("index", context.into_json())
    } else {
        render_error_template(
            format!("Could not query books: {:?}", books),
            &conn,
            &customer,
        )
        .await
    }
}

#[get("/login")]
pub async fn login_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("login", &context)
}

#[get("/login/failed/<e>")]
pub async fn login_failed(e: &str, conn: DbConn) -> Template {
    render_error_template(format!("Login failed: {}", e), &conn, &None).await
}

#[get("/register")]
pub async fn register_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("registration", &context)
}

#[get("/register/failed/<reason>")]
pub async fn register_failed(reason: &str, conn: DbConn) -> Template {
    render_error_template(format!("Registration failed: {}", reason), &conn, &None).await
}

#[get("/customer")]
pub async fn customer_page(cust: Customer, conn: DbConn) -> Template {
    let mut context = Context::new();

    match get_customer_info(&conn, cust.customer_id.clone()).await {
        Ok(v) => match v {
            Some(v) => v,
            None => {
                return render_error_template("No such customer", &conn, &None).await;
            }
        },
        Err(e) => {
            return render_error_template(format!("Server error: {}", e), &conn, &None).await;
        }
    };

    add_customer_info(&conn, &Some(cust), &mut context).await;
    Template::render("customer", context.into_json())
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

fn create_session_token() -> String {
    let mut rng = rand_chacha::ChaCha12Rng::from_entropy();
    let mut token: [u8; 32] = [0; 32];
    rng.fill_bytes(&mut token);

    let token = base64::encode(token);

    token
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
            let token = create_session_token();

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
        Ok(_) => Redirect::to("/"),
        Err(e) => Redirect::to(uri!(register_failed(format!("{:?}", e)))),
    }
}

#[get("/book/<isbn>")]
pub async fn book(conn: DbConn, isbn: &str, customer: Option<Customer>) -> Template {
    let mut context = Context::new();
    add_customer_info(&conn, &customer, &mut context).await;

    match isbn.parse::<i32>() {
        Ok(isbn) => {
            let books = get_books(&conn).await;

            match books {
                Ok(books) => match books.iter().find(|book| book.isbn == isbn) {
                    Some(book) => {
                        context.insert("book", &book);
                        Template::render("book", context.into_json())
                    }
                    None => {
                        render_error_template(
                            format!("No book with ISBN: {}", isbn),
                            &conn,
                            &customer,
                        )
                        .await
                    }
                },
                Err(e) => {
                    render_error_template(format!("Server error: {}", e), &conn, &customer).await
                }
            }
        }
        Err(_) => {
            render_error_template(format!("{} is not a valid ISBN", isbn), &conn, &customer).await
        }
    }
}

#[get("/customer/cart")]
pub async fn customer_cart_page(conn: DbConn, customer: Option<Customer>) -> Template {
    let mut context = Context::new();
    add_customer_info(&conn, &customer, &mut context).await;

    use schema::entities::Book;

    match customer {
        Some(customer) => match get_customer_cart(&conn, customer.customer_id).await {
            Ok(cart) => match get_books(&conn).await {
                Ok(books) => {
                    context.insert("cart", &cart);

                    let isbns: Vec<ISBN> = cart.iter().map(|c| c.0).collect();
                    let quantities: HashMap<ISBN, u32> = cart.iter().map(|p| p.clone()).collect();

                    #[derive(serde::Serialize)]
                    struct BookWithQuantity {
                        book: Book,
                        quantity: u32,
                    }

                    let books: Vec<BookWithQuantity> = books
                        .into_iter()
                        .filter(|b| isbns.contains(&b.isbn))
                        .map(|b| BookWithQuantity {
                            book: b.clone(),
                            quantity: *quantities.get(&b.isbn).unwrap(),
                        })
                        .collect();

                    context.insert("books", &books);
                    Template::render("customer_cart", context.into_json())
                }
                Err(e) => {
                    render_error_template(
                        format!("Error fetching books: {}", e),
                        &conn,
                        &Some(customer),
                    )
                    .await
                }
            },
            Err(_) => {
                render_error_template(
                    format!("Could not fetch cart for {}", customer.customer_id),
                    &conn,
                    &Some(customer),
                )
                .await
            }
        },
        None => render_error_template("Please login to see your cart.", &conn, &customer).await,
    }
}

#[put("/customer/cart/add/<isbn>")]
pub async fn customer_cart_add(conn: DbConn, customer: Customer, isbn: ISBN) -> Status {
    match add_to_cart(&conn, customer.customer_id, isbn).await {
        Ok(_) => Status::Ok,
        Err(_) => Status::InternalServerError,
    }
}

#[put("/customer/cart/quantity/<isbn>/<quantity>")]
pub async fn customer_cart_set_quantity(
    conn: DbConn,
    customer: Customer,
    isbn: ISBN,
    quantity: u32,
) -> Result<(), (Status, String)> {
    cart_set_book_quantity(&conn, customer.customer_id, isbn, quantity)
        .await
        .map_err(|e| match e {
            CartError::NotEnoughStock => (Status::Conflict, "Insufficient book stock".to_owned()),
            CartError::DBError(e) => (Status::InternalServerError, e.to_string()),
        })
}

#[post("/account/logout")]
pub async fn account_logout(
    cookies: &CookieJar<'_>,
    session_tokens: &State<SessionTokenState>,
) -> () {
    let mut session_tokens = session_tokens.lock().await;
    if let Some(cookie) = cookies.get_private(CUST_SESSION_COOKIE_NAME) {
        session_tokens.remove(cookie.value());
    }

    if let Some(cookie) = cookies.get_private(OWNER_SESSION_COOKIE_NAME) {
        session_tokens.remove(cookie.value());
    }
}

#[get("/checkout")]
pub async fn checkout_page(conn: DbConn, customer: Customer) -> Template {
    let mut context = Context::new();
    let customer_id = customer.customer_id;

    use schema::entities::Book;

    add_customer_info(&conn, &Some(customer), &mut context).await;
    match get_customer_cart(&conn, customer_id).await {
        Ok(cart) => match get_books(&conn).await {
            Ok(books) => {
                context.insert("cart", &cart);

                let isbns: Vec<ISBN> = cart.iter().map(|c| c.0).collect();
                let quantities: HashMap<ISBN, u32> = cart.iter().map(|p| p.clone()).collect();

                #[derive(serde::Serialize)]
                struct BookWithQuantity {
                    book: Book,
                    quantity: u32,
                }

                let books: Vec<BookWithQuantity> = books
                    .into_iter()
                    .filter(|b| isbns.contains(&b.isbn))
                    .map(|b| BookWithQuantity {
                        book: b.clone(),
                        quantity: *quantities.get(&b.isbn).unwrap(),
                    })
                    .collect();

                context.insert("books", &books);

                Template::render("checkout_page", context.into_json())
            }
            Err(e) => {
                render_error_template(
                    format!("Error fetching books: {}", e),
                    &conn,
                    &Some(customer),
                )
                .await
            }
        },
        Err(e) => {
            render_error_template(format!("Server error: {}", e), &conn, &Some(customer)).await
        }
    }
}

#[derive(FromForm)]
pub struct CreateOrder<'r> {
    default_shipping: bool,
    default_payment: bool,
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

#[post("/order/create", data = "<create_order>")]
pub async fn create_order_req(
    conn: DbConn,
    create_order: Form<CreateOrder<'_>>,
    customer: Customer,
) -> Template {
    let result: Result<PostgresInt, OrderError> = try {
        let cart = get_customer_cart(&conn, customer.customer_id).await?;

        let address = if create_order.default_shipping {
            None
        } else {
            Some(Address::new(
                create_order.street_address,
                create_order.postal_code,
                create_order.province,
            ))
        };

        let payment_info = if create_order.default_payment {
            None
        } else {
            let address = Address::new(
                create_order.billing_street_address,
                create_order.billing_postal_code,
                create_order.billing_province,
            );
            Some(PaymentInfo::new(
                create_order.name_on_card,
                Expiry::from_str(create_order.expiry).ok_or(StateError::new("Invalid expiry"))?,
                create_order.card_number,
                create_order.cvv,
                address,
            ))
        };

        crate::db::query::create_order(&conn, customer.customer_id, cart, address, payment_info)
            .await?
    };

    match result {
        Ok(order_id) => {
            let mut context = Context::new();
            context.insert("order_id", &order_id);
            Template::render("order_success", context.into_json())
        }
        Err(e) => {
            render_error_template(format!("Order error: {}", e), &conn, &Some(customer)).await
        }
    }
}

#[derive(Serialize, Debug)]
struct CensoredPaymentInfo {
    pub name_on_card: String,
    pub expiry: Expiry,
    pub censored_card_number: String,
    pub billing_address: Address,
}

#[derive(Serialize, Debug)]
struct BookWithQuantity {
    book: Book,
    quantity: u32,
}

#[derive(Serialize, Debug)]
struct CensoredOrder {
    pub order_id: PostgresInt,
    pub shipping_address: Address,
    pub tracking_number: String,
    pub order_status: String,
    pub order_date: String,
    pub payment_info: CensoredPaymentInfo,
    pub books: Vec<BookWithQuantity>,
}

fn censor_order(order: Order) -> CensoredOrder {
    let Order {
        order_id,
        shipping_address,
        tracking_number,
        order_status,
        order_date,
        payment_info,
        books,
    } = order;
    let PaymentInfo {
        name_on_card,
        expiry,
        card_number,
        cvv: _,
        billing_address,
    } = payment_info;

    let num_last_digits = usize::min(card_number.len(), 4);
    let censored_card_number = "*".repeat(12) + &card_number[card_number.len() - num_last_digits..];
    let censored_payment_info = CensoredPaymentInfo {
        name_on_card,
        expiry,
        censored_card_number,
        billing_address,
    };

    let books = books
        .into_iter()
        .map(|(book, quantity)| BookWithQuantity { book, quantity })
        .collect();

    CensoredOrder {
        order_id,
        shipping_address,
        tracking_number,
        order_status,
        order_date,
        payment_info: censored_payment_info,
        books,
    }
}

fn add_orders_to_context(orders: Vec<Order>, context: &mut Context) {
    let orders: Vec<CensoredOrder> = orders.into_iter().map(censor_order).collect();

    context.insert("orders", &orders);
}

#[get("/order/view")]
pub async fn orders_page(conn: DbConn, customer: Customer) -> Template {
    let mut context = Context::new();

    let customer_id = customer.customer_id;

    add_customer_info(&conn, &Some(customer), &mut context).await;

    match get_customer_orders_info(&conn, customer_id).await {
        Ok(orders) => {
            context.insert("num_orders", &orders.len());
            add_orders_to_context(orders, &mut context);
            Template::render("orders", context.into_json())
        }
        Err(e) => {
            render_error_template(format!("Server error: {}", e), &conn, &Some(customer)).await
        }
    }
}

#[get("/order/view/<order_id>")]
pub async fn view_order(conn: DbConn, customer: Customer, order_id: i32) -> Template {
    let mut context = Context::new();

    add_customer_info(&conn, &Some(customer), &mut context).await;

    match get_order_info(&conn, order_id).await {
        Ok(order_info) => match get_books_for_order(&conn, order_info).await {
            Ok(order) => {
                let censored_order = censor_order(order);
                context.insert("order", &censored_order);
                Template::render("order", context.into_json())
            }
            Err(e) => {
                render_error_template(format!("Server error: {}", e), &conn, &Some(customer)).await
            }
        },
        Err(e) => {
            render_error_template(format!("Server error: {}", e), &conn, &Some(customer)).await
        }
    }
}

#[get("/login/owner")]
pub async fn owner_login_page() -> Template {
    let context = HashMap::<&str, &str>::new();
    Template::render("owner_login", &context)
}

#[post("/login/owner", data = "<login_data>")]
pub async fn owner_login(
    conn: DbConn,
    login_data: Form<Login<'_>>,
    session_tokens: &State<SessionTokenState>,
    cookies: &CookieJar<'_>,
) -> Redirect {
    match validate_owner_login(&conn, login_data.email, login_data.password).await {
        Ok(login) => match login {
            OwnerLoginType::DefaultOwner => {
                let token = create_session_token();

                cookies.add_private(Cookie::new(OWNER_SESSION_COOKIE_NAME, token.clone()));

                let mut session_tokens = session_tokens.lock().await;

                let expiry = Local::now() + Duration::days(30);

                session_tokens.insert(token, (SessionType::DefaultOwner, expiry));
                Redirect::to(uri!("/"))
            }
            OwnerLoginType::OwnerAccount(owner_id) => {
                let token = create_session_token();

                cookies.add_private(Cookie::new(OWNER_SESSION_COOKIE_NAME, token.clone()));

                let mut session_tokens = session_tokens.lock().await;

                let expiry = Local::now() + Duration::days(30);

                session_tokens.insert(token, (SessionType::Owner(owner_id), expiry));
                Redirect::to(uri!("/"))
            }
        },
        Err(e) => Redirect::to(uri!(login_failed(e.to_string()))),
    }
}

#[get("/owner/manage/view?<search>")]
pub async fn book_management(conn: DbConn, owner: Owner, search: Search<'_>) -> Template {
    let mut context = Context::new();
    add_owner_tag(&Some(owner), &mut context);

    let books = get_books_with_publisher_name(&conn).await;
    if let Ok(books) = books {
        let books = filter_books(books, search);

        context.insert("books", &books);
        context.insert("genres", &extract_genre_list(&books));

        Template::render("book_management", context.into_json())
    } else {
        render_error_template(format!("Could not query books: {:?}", books), &conn, &None).await
    }
}

#[put("/owner/manage/books/discontinue", data = "<books>")]
pub async fn discontinue_books_endpoint(
    conn: DbConn,
    _owner: Owner,
    books: Json<Vec<ISBN>>,
) -> Result<(), (Status, String)> {
    match discontinue_books(&conn, books.into_inner()).await {
        Ok(_) => Ok(()),
        Err(e) => Err((Status::InternalServerError, e.to_string())),
    }
}

#[put("/owner/manage/books/undiscontinue", data = "<books>")]
pub async fn undiscontinue_books_endpoint(
    conn: DbConn,
    _owner: Owner,
    books: Json<Vec<ISBN>>,
) -> Result<(), (Status, String)> {
    match undiscontinue_books(&conn, books.into_inner()).await {
        Ok(_) => Ok(()),
        Err(e) => Err((Status::InternalServerError, e.to_string())),
    }
}

#[derive(FromForm)]
pub struct CreatePublisher<'r> {
    company_name: &'r str,
    email: &'r str,
    street_address: &'r str,
    postal_code: &'r str,
    province: &'r str,
    phone_number: &'r str,
    bank_number: &'r str,
}

#[get("/owner/create/publisher")]
pub async fn create_publisher_page(owner: Owner) -> Template {
    let mut context = Context::new();
    add_owner_tag(&Some(owner), &mut context);

    Template::render("create_publisher", context.into_json())
}

#[post("/owner/create/publisher", data = "<publisher>")]
pub async fn create_publisher(
    conn: DbConn,
    _owner: Owner,
    publisher: Form<CreatePublisher<'_>>,
) -> Template {
    let CreatePublisher {
        company_name,
        email,
        street_address,
        postal_code,
        province,
        phone_number,
        bank_number,
    } = *publisher;
    let address = Address::new(street_address, postal_code, province);

    match try_create_publisher(
        &conn,
        company_name,
        email,
        address,
        phone_number,
        bank_number,
    )
    .await
    {
        Ok(_) => {
            let context = Context::new();
            Template::render("create_publisher_success", context.into_json())
        }
        Err(e) => render_error_template(e.to_string(), &conn, &None).await,
    }
}

#[get("/owner/reports")]
pub async fn reports_page(owner: Owner) -> Template {
    let mut context = Context::new();
    add_owner_tag(&Some(owner), &mut context);

    Template::render("reports", context.into_json())
}

#[get("/owner/reports/sales")]
pub async fn sales_report_image(conn: DbConn, _owner: Owner) -> (ContentType, String) {
    let sales_by_date = get_sales_by_date(&conn).await.unwrap();

    let today = Local::today().naive_local();
    // Converts the date indexing to days since today
    let map_sales = |sales: Vec<(NaiveDate, i64)>| {
        let mapped_sales: Vec<(i128, i128)> = sales
            .into_iter()
            .map(|(date, quantity)| {
                (
                    -today.signed_duration_since(date).num_days() as i128,
                    quantity as i128,
                )
            })
            .collect();

        mapped_sales
    };

    let total_sales: Vec<(i128, i128)> = map_sales(sales_by_date);
    let publishers = match get_publishers(&conn).await {
        Ok(v) => v,
        Err(_) => vec![],
    };

    let mut sales_data: Vec<(String, Vec<(i128, i128)>)> =
        vec![("Total Sales".to_string(), total_sales)];

    for publisher in publishers {
        let sales = get_sales_by_publisher(&conn, publisher.publisher_id).await;
        if let Ok(sales) = sales {
            sales_data.push((publisher.company_name.clone(), map_sales(sales)));
        }
    }

    let days_included = sales_data
        .iter()
        .map(|(_, data)| data.iter().map(|(day, _)| *day))
        .flatten()
        .collect::<HashSet<i128>>();

    // Add missing dates as 0 data points
    for (_, data) in &mut sales_data {
        let days: Vec<i128> = data.iter().map(|(day, _)| *day).collect();
        for day in &days_included {
            if !days.contains(*day) {
                data.push((*day, 0));
            }
        }
    }

    let earliest_day = *sales_data
        .iter()
        .map(|(_, data)| data)
        .flatten()
        .map(|(days_since_today, _)| days_since_today)
        .min()
        .unwrap_or(&0);

    let mut sales_data_as_days_since_earliest: Vec<(String, Vec<(i128, i128)>)> = sales_data
        .into_iter()
        .map(|(name, data)| {
            (
                name,
                data.into_iter()
                    .map(|(days_since_today, quantity)| (days_since_today - earliest_day, quantity))
                    .collect(),
            )
        })
        .collect();

    for (_, data) in &mut sales_data_as_days_since_earliest {
        data.sort_by(|(x0, _), (x1, _)| x0.cmp(x1));
    }

    let mut s = poloto::plot("Book Sales", "Date", "Sales (1 Book)");

    s.ymarker(-1);

    for (name, data) in sales_data_as_days_since_earliest {
        s.line(name, data);
    }

    s.xinterval_fmt(|fmt, val, _| {
        write!(
            fmt,
            "{}",
            today
                .checked_add_signed(Duration::days((val + earliest_day) as i64))
                .unwrap_or(today)
                .to_string()
        )
    });

    let svg = poloto::disp(|a| poloto::simple_theme(a, s)).to_string();

    (ContentType::SVG, svg)
}

#[get("/owner/create/book")]
pub async fn create_book_page(owner: Owner, conn: DbConn) -> Template {
    #[derive(Serialize, Debug)]
    struct Publisher {
        name: String,
        email: String,
        id: PostgresInt,
    }

    let publishers = match get_publishers(&conn).await {
        Ok(v) => v,
        Err(e) => return render_error_template(e.to_string(), &conn, &None).await,
    };

    let publishers: Vec<Publisher> = publishers
        .into_iter()
        .map(|publisher| Publisher {
            name: publisher.company_name,
            email: publisher.email,
            id: publisher.publisher_id,
        })
        .collect();

    let mut context = Context::new();
    add_owner_tag(&Some(owner), &mut context);

    context.insert("publishers", &publishers);

    Template::render("create_book", context.into_json())
}

#[derive(FromForm)]
pub struct CreateBook<'r> {
    isbn: i32,
    title: &'r str,
    author_name: &'r str,
    genre: &'r str,
    publisher_id: i32,
    num_pages: i32,
    price: &'r str,
    author_royalties: &'r str,
    reorder_threshold: i32,
    stock: i32,
    discontinued: bool,
}

#[post("/owner/create/book", data = "<book>")]
pub async fn create_book_endpoint(
    conn: DbConn,
    owner: Owner,
    book: Form<CreateBook<'_>>,
) -> Template {
    let CreateBook {
        isbn,
        title,
        author_name,
        genre,
        publisher_id,
        num_pages,
        price,
        author_royalties,
        reorder_threshold,
        stock,
        discontinued,
    } = *book;

    let book: Result<Book, <Decimal as FromStr>::Err> = try {
        Book::new(
            isbn,
            title.to_string(),
            author_name.to_string(),
            genre.to_string(),
            publisher_id,
            num_pages,
            Decimal::from_str(price)?,
            Decimal::from_str(author_royalties)?,
            reorder_threshold,
            stock,
            discontinued,
        )
    };

    let result = match book {
        Ok(book) => create_book(&conn, book).await,
        Err(e) => return render_error_template(e.to_string(), &conn, &None).await,
    };

    match result {
        Ok(_) => {
            let mut context = Context::new();
            add_owner_tag(&Some(owner), &mut context);

            context.insert("isbn", &isbn);

            Template::render("create_book_success", context.into_json())
        }
        Err(e) => render_error_template(e.to_string(), &conn, &None).await,
    }
}

#[get("/owner/manage/accounts")]
pub async fn manage_accounts(conn: DbConn, owner: Owner) -> Template {
    let mut context = Context::new();

    add_owner_tag(&Some(owner), &mut context);

    let owners = get_owner_accounts(&conn).await.unwrap_or(vec![]);
    let customers = get_customer_accounts(&conn).await.unwrap_or(vec![]);

    context.insert("owners", &owners);
    context.insert("customers", &customers);

    Template::render("manage_accounts", context.into_json())
}

#[get("/owner/manage/customer/delete/<customer_id>")]
pub async fn delete_customer_page(owner: Owner, customer_id: PostgresInt) -> Template {
    let mut context = Context::new();

    add_owner_tag(&Some(owner), &mut context);

    context.insert("customer_id", &customer_id);

    Template::render("confirm_delete", context.into_json())
}

#[get("/owner/manage/owner/delete/<owner_id>")]
pub async fn delete_owner_page(owner: Owner, owner_id: PostgresInt) -> Template {
    let mut context = Context::new();

    add_owner_tag(&Some(owner), &mut context);

    context.insert("customer_id", &owner_id);

    Template::render("confirm_delete", context.into_json())
}

#[get("/success/delete")]
pub async fn delete_success_page(owner: Owner) -> Template {
    let mut context = Context::new();

    add_owner_tag(&Some(owner), &mut context);

    Template::render("delete_success", context.into_json())
}

#[get("/error/<error>")]
pub async fn error_page(
    conn: DbConn,
    error: &'_ str,
    customer: Option<Customer>,
    owner: Option<Owner>,
) -> Template {
    let mut context = Context::new();

    add_owner_tag(&owner, &mut context);

    render_error_template(error, &conn, &customer).await
}

#[post("/owner/manage/customer/delete/<customer_id>")]
pub async fn delete_customer_endpoint(
    conn: DbConn,
    _owner: Owner,
    customer_id: PostgresInt,
) -> Result<(), (Status, String)> {
    match delete_customer_account(&conn, customer_id).await {
        Ok(_) => Ok(()),
        Err(e) => Err((Status::InternalServerError, e.to_string())),
    }
}

#[post("/owner/manage/owner/delete/<owner_id>")]
pub async fn delete_owner_endpoint(
    conn: DbConn,
    _owner: Owner,
    owner_id: PostgresInt,
) -> Result<(), (Status, String)> {
    match delete_owner_account(&conn, owner_id).await {
        Ok(_) => Ok(()),
        Err(e) => Err((Status::InternalServerError, e.to_string())),
    }
}

#[derive(FromForm)]
pub struct RegisterOwner<'r> {
    email: &'r str,
    name: &'r str,
    password: &'r str,
}

#[post("/owner/manage/owner/create", data = "<owner_data>")]
pub async fn create_owner(conn: DbConn, owner_data: Form<RegisterOwner<'_>>) -> Redirect {
    let RegisterOwner {
        email,
        name,
        password,
    } = *owner_data;

    match try_create_new_owner(&conn, email, password, name).await {
        Ok(_) => Redirect::to("/"),
        Err(e) => Redirect::to(uri!(register_failed(format!("{:?}", e)))),
    }
}
