use sea_orm_migration::prelude::*;

use crate::m20240205_114643_create_images::Images;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Images::Table)
                    .add_column(
                        ColumnDef::new(Images::UsesCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Images::Table)
                    .drop_column(Images::UsesCount)
                    .to_owned(),
            )
            .await
    }
}
