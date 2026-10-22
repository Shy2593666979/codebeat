//! SQLite 表映射；业务模型位于 crate::models。
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "quota_reset_consumptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub attempt_id: String,
    pub account_key: String,
    pub consumed_at: String,
    pub reconciled: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
