use anyhow::Result;
use entities::{images, prelude::*, sea_orm_active_enums::MediaType, users};
use migration::{Alias, BinOper, Migrator, MigratorTrait, SimpleExpr};
use sea_orm::{
    prelude::*, ActiveValue, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    FromQueryResult, IntoSimpleExpr, QueryOrder, QuerySelect,
};
use tracing::log::LevelFilter;

#[derive(FromQueryResult)]
pub struct ImageWithIds {
    pub id: i32,
    pub media_type: MediaType,
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

    pub async fn increment_image_uses(&self, image: i32, user: i64) -> Result<()> {
        Images::update_many()
            .col_expr(
                images::Column::UsesCount,
                images::Column::UsesCount.into_simple_expr().add(1),
            )
            .filter(images::Column::Id.eq(image))
            .filter(images::Column::UserId.eq(user))
            .exec(&self.dc)
            .await?;
        Ok(())
    }

    pub async fn create_image(
        &self,
        user: i64,
        embedding: Vec<f32>,
        file_id: String,
        unique_id: String,
        media_type: MediaType,
    ) -> Result<()> {
        let image = images::ActiveModel {
            user_id: ActiveValue::Set(user),
            media_type: ActiveValue::Set(media_type),
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

    pub async fn search_images(
        &self,
        user: i64,
        embedding: Vec<f32>,
        offset: Option<u64>,
    ) -> Result<Vec<ImageWithIds>> {
        let res = Images::find()
            .select_only()
            .column(images::Column::Id)
            .column(images::Column::MediaType)
            .column(images::Column::FileId)
            .filter(images::Column::UserId.eq(user))
            .order_by_asc(images::Column::Embedding.into_simple_expr().binary(
                BinOper::Custom("<=>"),
                SimpleExpr::from(embedding).cast_as(Alias::new("vector")),
            ))
            .limit(51)
            .offset(offset)
            .into_model::<ImageWithIds>()
            .all(&self.dc)
            .await?;
        Ok(res)
    }

    pub async fn get_most_used_images(
        &self,
        user: i64,
        offset: Option<u64>,
    ) -> Result<Vec<ImageWithIds>> {
        let res = Images::find()
            .select_only()
            .column(images::Column::Id)
            .column(images::Column::MediaType)
            .column(images::Column::FileId)
            .filter(images::Column::UserId.eq(user))
            .order_by_desc(images::Column::UsesCount)
            .order_by_desc(images::Column::CreationTime)
            .limit(51)
            .offset(offset)
            .into_model::<ImageWithIds>()
            .all(&self.dc)
            .await?;
        Ok(res)
    }

    pub async fn get_all_images(&self) -> Result<Vec<ImageWithIds>> {
        let res = Images::find()
            .select_only()
            .column(images::Column::Id)
            .column(images::Column::MediaType)
            .column(images::Column::FileId)
            .into_model::<ImageWithIds>()
            .all(&self.dc)
            .await?;
        Ok(res)
    }

    pub async fn update_image(&self, id: i32, embedding: Vec<f32>) -> Result<()> {
        Images::update_many()
            .col_expr(images::Column::Embedding, embedding.into())
            .filter(images::Column::Id.eq(id))
            .exec(&self.dc)
            .await?;
        Ok(())
    }
}
