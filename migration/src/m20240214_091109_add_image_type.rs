use sea_orm::{sea_query::extension::postgres::Type, EnumIter, Iterable};
use sea_orm_migration::prelude::*;

use crate::m20240205_114643_create_images::Images;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(MediaType::Table)
                    .values(MediaType::iter().skip(1))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Images::Table)
                    .add_column(
                        ColumnDef::new(Images::MediaType)
                            .enumeration(MediaType::Table, MediaType::iter().skip(1))
                            .not_null()
                            .default(SimpleExpr::Custom(
                                "CAST('photo' AS media_type)".to_string(),
                            )),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Images::Table)
                    .drop_column(Images::MediaType)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(Type::drop().name(MediaType::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden, EnumIter)]
enum MediaType {
    Table,
    Photo,
    Sticker,
}
