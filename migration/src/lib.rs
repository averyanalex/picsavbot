pub use sea_orm_migration::prelude::*;

mod m20240205_113957_create_users;
mod m20240205_114643_create_images;
mod m20240214_091109_add_image_type;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240205_113957_create_users::Migration),
            Box::new(m20240205_114643_create_images::Migration),
            Box::new(m20240214_091109_add_image_type::Migration),
        ]
    }
}
