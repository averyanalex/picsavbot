use sea_orm::sea_query::extension::postgres::Type;
use sea_orm_migration::prelude::*;

use crate::m20240214_091109_add_image_type::MediaType;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_type(
                Type::alter()
                    .name(MediaType::Table)
                    .add_value(MediaType::Video),
            )
            .await
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
