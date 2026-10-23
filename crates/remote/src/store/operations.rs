use super::*;

impl BindingStore {
    /// Atomic durable accept. A duplicate ID with a different body is a conflict, never a new command.
    pub fn accept_operation(
        &self,
        id: &str,
        owner: &str,
        request: &str,
        initial: &str,
    ) -> Result<(bool, String)> {
        ensure!(
            id.len() >= 16
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
            "invalid operation ID"
        );
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String, String)> = tx
            .query_row(
                "SELECT owner, digest, result FROM operations WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((old_owner, old_digest, result)) = existing {
            ensure!(
                old_owner == owner && old_digest == digest(request.as_bytes()),
                "idempotency key conflicts with a different request"
            );
            return Ok((false, result));
        }
        // Bound the journal without silently dropping deduplication records.
        let count: i64 = tx.query_row("SELECT count(*) FROM operations", [], |r| r.get(0))?;
        ensure!(
            count < 100_000,
            "operation journal is full; maintenance required"
        );
        tx.execute(
            "INSERT INTO operations VALUES(?1,?2,?3,?4,?5)",
            params![id, owner, digest(request.as_bytes()), request, initial],
        )?;
        tx.commit()?;
        Ok((true, initial.to_owned()))
    }
    pub fn finish_operation(&self, id: &str, result: &str) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        ensure!(
            connection.execute(
                "UPDATE operations SET result=?2 WHERE id=?1",
                params![id, result]
            )? == 1,
            "operation missing"
        );
        Ok(())
    }
    pub fn operation(&self, id: &str, owner: &str) -> Result<Option<(String, String)>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        Ok(connection
            .query_row(
                "SELECT request,result FROM operations WHERE id=?1 AND owner=?2",
                params![id, owner],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
    pub fn operation_records(&self) -> Result<Vec<(String, String)>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        let mut statement = connection.prepare("SELECT id,result FROM operations")?;
        Ok(statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
