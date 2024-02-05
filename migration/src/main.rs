// use migration::{Migrator, MigratorTrait};
// use sea_orm::Database;
use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(migration::Migrator).await;
}

// #[tokio::main]
// async fn main() {
//     tracing_subscriber::fmt()
//         .with_max_level(tracing::Level::DEBUG)
//         .with_test_writer()
//         .init();

//     let db_url = std::env::var("DATABASE_URL").unwrap();
//     let conn = Database::connect(db_url).await.unwrap();
//     Migrator::up(&conn, None).await.unwrap();
// }
