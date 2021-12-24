use std::env;

use diesel::{Connection, ConnectionError, PgConnection};
use mobc::{async_trait, Manager, Pool};

pub struct PgConnectionManager;

#[async_trait]
impl Manager for PgConnectionManager {
    type Connection = PgConnection;
    type Error = ConnectionError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url)
    }

    async fn check(&self, conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        Ok(conn)
    }
}

pub fn establish_connection_pool() -> Pool<PgConnectionManager> {
    Pool::new(PgConnectionManager)
}
