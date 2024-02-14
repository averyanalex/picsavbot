use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::m20240205_113957_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "CREATE EXTENSION IF NOT EXISTS vector;",
        ))
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(Images::Table)
                    .col(
                        ColumnDef::new(Images::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Images::UserId).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Images::Table, Images::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .col(ColumnDef::new(Images::FileId).string().not_null())
                    .col(ColumnDef::new(Images::UniqueId).string().not_null())
                    .col(
                        ColumnDef::new(Images::CreationTime)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Images::Embedding)
                            .custom(Alias::new("vector(1024)"))
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Images::Table)
                    .col(Images::UserId)
                    .col(Images::UniqueId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Images::Table).to_owned())
            .await?;

        let db = manager.get_connection();
        db.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "DROP EXTENSION vector;",
        ))
        .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Images {
    Table,
    Id,
    UserId,
    UsesCount,
    MediaType,
    FileId,
    UniqueId,
    CreationTime,
    Embedding,
}
