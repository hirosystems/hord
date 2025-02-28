pub mod types;
pub mod utils;

use config::PgDatabaseConfig;
use deadpool_postgres::{Manager, ManagerConfig, Object, Pool, RecyclingMethod, Transaction};
use tokio_postgres::{Client, Config, NoTls, Row};

/// Standard chunk size to use when we're batching multiple query inserts into a single SQL statement to save on DB round trips.
/// This number is designed to not hit the postgres limit of 65536 query parameters in a single SQL statement, but results may
/// vary depending on column counts. Queries should use other custom chunk sizes as needed.
pub const BATCH_QUERY_CHUNK_SIZE: usize = 500;

/// Creates a Postgres connection pool based on a single database config. You can then use this pool to create ad-hoc clients and
/// transactions for interacting with the database.
pub fn pg_pool(config: &PgDatabaseConfig) -> Result<Pool, String> {
    let mut pg_config = Config::new();
    pg_config
        .dbname(&config.dbname)
        .host(&config.host)
        .port(config.port)
        .user(&config.user)
        .options(format!(
            "-csearch_path={}",
            config.search_path.as_ref().unwrap_or(&"public".to_string())
        ));
    if let Some(password) = &config.password {
        pg_config.password(password);
    }
    let manager = Manager::from_config(
        pg_config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    let mut pool_builder = Pool::builder(manager);
    if let Some(size) = config.pool_max_size {
        pool_builder = pool_builder.max_size(size);
    }
    Ok(pool_builder
        .build()
        .map_err(|e| format!("unable to build pg connection pool: {e}"))?)
}

/// Returns a new pg connection client taken from a pool.
pub async fn pg_pool_client(pool: &Pool) -> Result<Object, String> {
    pool.get()
        .await
        .map_err(|e| format!("unable to get pg client: {e}"))
}

/// Returns a new pg transaction taken from an existing pool connection
pub async fn pg_begin(client: &mut Object) -> Result<Transaction<'_>, String> {
    client
        .transaction()
        .await
        .map_err(|e| format!("unable to begin pg transaction: {e}"))
}

/// Connects to postgres directly (without a Pool) and returns an open client.
pub async fn pg_connect(config: &PgDatabaseConfig) -> Result<Client, String> {
    let mut pg_config = Config::new();
    pg_config
        .dbname(&config.dbname)
        .host(&config.host)
        .port(config.port)
        .user(&config.user);
    if let Some(password) = &config.password {
        pg_config.password(password);
    }
    match pg_config.connect(NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    println!("postgres connection error: {e}");
                }
            });
            Ok(client)
        }
        Err(e) => Err(format!("error connecting to postgres: {e}")),
    }
}

/// Connects to postgres with infinite retries and returns an open client.
pub async fn pg_connect_with_retry(config: &PgDatabaseConfig) -> Client {
    loop {
        match pg_connect(config).await {
            Ok(client) => return client,
            Err(e) => {
                println!("error connecting to postgres: {e}");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

/// Transforms a Postgres row into a model struct.
pub trait FromPgRow {
    fn from_pg_row(row: &Row) -> Self;
}

#[cfg(test)]
pub async fn pg_test_client() -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(
        "host=localhost user=postgres password=postgres",
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("test connection error: {}", e);
        }
    });
    client
}

#[cfg(test)]
pub async fn pg_test_roll_back_migrations(pg_client: &mut tokio_postgres::Client) {
    match pg_client
        .batch_execute(
            "
            DO $$ DECLARE
                r RECORD;
            BEGIN
                FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = current_schema()) LOOP
                    EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
                END LOOP;
            END $$;
            DO $$ DECLARE
                r RECORD;
            BEGIN
                FOR r IN (SELECT typname FROM pg_type WHERE typtype = 'e' AND typnamespace = (SELECT oid FROM pg_namespace WHERE nspname = current_schema())) LOOP
                    EXECUTE 'DROP TYPE IF EXISTS ' || quote_ident(r.typname) || ' CASCADE';
                END LOOP;
            END $$;",
        )
        .await {
            Ok(rows) => rows,
            Err(_) => unreachable!()
        };
}

#[cfg(test)]
mod test {
    use config::PgDatabaseConfig;

    use crate::{pg_begin, pg_pool, pg_pool_client};

    #[tokio::test]
    async fn test_pg_connection_and_transaction() -> Result<(), String> {
        let pool = pg_pool(&PgDatabaseConfig {
            dbname: "postgres".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            user: "postgres".to_string(),
            password: Some("postgres".to_string()),
            search_path: None,
            pool_max_size: None,
        })?;
        let mut client = pg_pool_client(&pool).await?;
        let transaction = pg_begin(&mut client).await?;
        let row = transaction
            .query_opt("SELECT 1 AS result", &[])
            .await
            .unwrap()
            .unwrap();
        let count: i32 = row.get("result");
        assert_eq!(1, count);
        transaction.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
