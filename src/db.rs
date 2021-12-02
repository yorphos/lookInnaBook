pub mod conn {
    use rocket_sync_db_pools::{database, postgres};

    #[database("postgres")]
    pub struct DbConn(postgres::Client);
}

pub mod query {
    use super::conn::DbConn;
    use crate::schema::entities::*;

    pub enum LoginType {
        OwnerLogin(PostgresInt),
        CustomerLogin(PostgresInt),
        FailedLogin,
    }

    pub async fn get_books(conn: &DbConn) -> Result<Vec<Book>, postgres::error::Error> {
        let rows = conn
            .run(|c| c.query("SELECT * FROM base.book", &[]))
            .await?;
        Ok(rows.iter().flat_map(|row| Book::from_row(row)).collect())
    }

    pub async fn validate_login<T: AsRef<str>>(
        conn: &DbConn,
        email: T,
        password: T,
    ) -> Result<LoginType, postgres::error::Error> {
        Ok(LoginType::FailedLogin)
    }
}
