//! 在普通连接和事务内复用相同的实体查询，避免事务中的操作误用外层连接。
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    QueryResult, SqliteTransactionMode, Statement, TransactionOptions, TransactionTrait,
};

pub(super) enum Connection {
    Database(DatabaseConnection),
    Transaction(DatabaseTransaction),
}

impl Connection {
    pub fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        let options = TransactionOptions {
            sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
            ..Default::default()
        };
        match self {
            Self::Database(db) => db.begin_with_options(options),
            Self::Transaction(tx) => tx.begin_with_options(options),
        }
    }
}

impl ConnectionTrait for Connection {
    fn get_database_backend(&self) -> DbBackend {
        DbBackend::Sqlite
    }
    fn execute_raw(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match self {
            Self::Database(db) => db.execute_raw(statement),
            Self::Transaction(tx) => tx.execute_raw(statement),
        }
    }
    fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        match self {
            Self::Database(db) => db.execute_unprepared(sql),
            Self::Transaction(tx) => tx.execute_unprepared(sql),
        }
    }
    fn query_one_raw(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        match self {
            Self::Database(db) => db.query_one_raw(statement),
            Self::Transaction(tx) => tx.query_one_raw(statement),
        }
    }
    fn query_all_raw(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match self {
            Self::Database(db) => db.query_all_raw(statement),
            Self::Transaction(tx) => tx.query_all_raw(statement),
        }
    }
}
