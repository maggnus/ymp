//! Immutable content in the same SQLite database as the journal.
use crate::{Database, MAX_CONTENT_BYTES, put_content, read_content, sql_error};
use rusqlite::TransactionBehavior;
use ymp_domain::{Denial, Digest, Result};
use ymp_kernel::journal::ContentStore;

#[derive(Clone)]
pub struct SqliteContent {
    pub(crate) database: Database,
}
impl ContentStore for SqliteContent {
    fn put(&self, bytes: &[u8]) -> Result<Digest> {
        if bytes.len() > MAX_CONTENT_BYTES {
            return Err(Denial::new(
                "content_limit",
                "Content exceeds the storage limit",
            ));
        }
        let digest = Digest::of(bytes);
        let mut connection = self.database.connect(true)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        put_content(&tx, &digest, bytes)?;
        tx.commit().map_err(sql_error)?;
        Ok(digest)
    }
    fn get(&self, digest: &Digest, limit: usize) -> Result<Vec<u8>> {
        let mut connection = self.database.connect(false)?;
        let tx = connection.transaction().map_err(sql_error)?;
        let bytes = read_content(&tx, digest, limit)?;
        tx.commit().map_err(sql_error)?;
        Ok(bytes)
    }
}
