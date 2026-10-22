//! SQLite 表映射；业务模型位于 crate::models。
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "awo_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub provider_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub slot: String,
    pub attempted_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
