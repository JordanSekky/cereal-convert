use std::env;

use diesel::sql_types::Integer;
use diesel::{Connection, ConnectionError, RunQueryDsl};
use diesel_tracing::pg::InstrumentedPgConnection;
use mobc::{async_trait, Manager, Pool};

use crate::util::InstrumentedPgConnectionPool;

#[derive(QueryableByName)]
struct TestResult {
    #[sql_type = "Integer"]
    _a: i32,
}

pub struct PgConnectionManager;

#[async_trait]
impl Manager for PgConnectionManager {
    type Connection = InstrumentedPgConnection;
    type Error = ConnectionError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        InstrumentedPgConnection::establish(&database_url)
    }

    async fn check(&self, conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        match diesel::sql_query("SELECT 1 as _a").load::<TestResult>(&conn) {
            Ok(_) => Ok(conn),
            Err(_) => Err(ConnectionError::BadConnection(String::from(
                "Failed to select 1.",
            ))),
        }
    }
}

pub fn establish_connection_pool() -> InstrumentedPgConnectionPool {
    InstrumentedPgConnectionPool(Pool::builder().max_open(30).build(PgConnectionManager))
}
