use anyhow::Result;
use entities::{images, prelude::*, users};
use migration::{Alias, BinOper, Migrator, MigratorTrait, SimpleExpr};
use sea_orm::{
    prelude::*, ActiveValue, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    FromQueryResult, IntoSimpleExpr, QueryOrder, QuerySelect,
};
use tracing::log::LevelFilter;

#[derive(FromQueryResult)]
pub struct ImageWithIds {
    pub id: i32,
    pub file_id: String,
}

pub struct Db {
    dc: DatabaseConnection,
}

impl Db {
    pub async fn new() -> Result<Self> {
        let db_url = std::env::var("DATABASE_URL")?;

        let mut conn_options = ConnectOptions::new(db_url);
        conn_options.sqlx_logging_level(LevelFilter::Debug);
        conn_options.sqlx_logging(true);

        let dc = Database::connect(conn_options).await?;
        Migrator::up(&dc, None).await?;
        Ok(Self { dc })
    }

    pub async fn update_user(&self, id: i64) -> Result<()> {
        if Users::find_by_id(id).one(&self.dc).await?.is_some() {
            Users::update_many()
                .col_expr(
                    users::Column::LastActivity,
                    Expr::current_timestamp().into(),
                )
                .filter(users::Column::Id.eq(id))
                .exec(&self.dc)
                .await?;
        } else {
            let user = users::ActiveModel {
                id: ActiveValue::Set(id),
                ..Default::default()
            };
            Users::insert(user).exec(&self.dc).await?;
        }
        Ok(())
    }

    pub async fn create_image(
        &self,
        user: i64,
        embedding: Vec<f32>,
        file_id: String,
        unique_id: String,
    ) -> Result<()> {
        let image = images::ActiveModel {
            user_id: ActiveValue::Set(user),
            file_id: ActiveValue::Set(file_id),
            unique_id: ActiveValue::Set(unique_id),
            embedding: ActiveValue::Set(embedding),
            ..Default::default()
        };
        Images::insert(image).exec(&self.dc).await?;
        Ok(())
    }

    pub async fn delete_image(&self, user: i64, unique_id: String) -> Result<bool> {
        let res = Images::delete_many()
            .filter(images::Column::UserId.eq(user))
            .filter(images::Column::UniqueId.eq(unique_id))
            .exec(&self.dc)
            .await?;
        Ok(res.rows_affected >= 1)
    }

    pub async fn search_images(&self, user: i64, embedding: Vec<f32>) -> Result<Vec<ImageWithIds>> {
        let res = Images::find()
            .select_only()
            .column(images::Column::Id)
            .column(images::Column::FileId)
            .filter(images::Column::UserId.eq(user))
            .order_by_asc(images::Column::Embedding.into_simple_expr().binary(
                BinOper::Custom("<->"),
                SimpleExpr::from(embedding).cast_as(Alias::new("vector")),
            ))
            .limit(50)
            .into_model::<ImageWithIds>()
            .all(&self.dc)
            .await?;
        Ok(res)
    }

    pub async fn get_latest_images(&self, user: i64) -> Result<Vec<ImageWithIds>> {
        let res = Images::find()
            .select_only()
            .column(images::Column::Id)
            .column(images::Column::FileId)
            .filter(images::Column::UserId.eq(user))
            .order_by_desc(images::Column::CreationTime)
            .limit(50)
            .into_model::<ImageWithIds>()
            .all(&self.dc)
            .await?;
        Ok(res)
    }
}
