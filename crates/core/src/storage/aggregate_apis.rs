use rusqlite::types::ValueRef;
use rusqlite::{Error, Result, Row};

use super::{now_ts, AggregateApi, Storage};

const AGGREGATE_API_SELECT_SQL: &str = "SELECT
    id,
    provider_type,
    supplier_name,
    sort,
    url,
    status,
    created_at,
    updated_at,
    last_test_at,
    last_test_status,
    last_test_error
 FROM aggregate_apis";

impl Storage {
    pub fn insert_aggregate_api(&self, api: &AggregateApi) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO aggregate_apis (
                id,
                provider_type,
                supplier_name,
                sort,
                url,
                status,
                created_at,
                updated_at,
                last_test_at,
                last_test_status,
                last_test_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                &api.id,
                &api.provider_type,
                &api.supplier_name,
                api.sort,
                &api.url,
                &api.status,
                api.created_at,
                api.updated_at,
                &api.last_test_at,
                &api.last_test_status,
                &api.last_test_error,
            ),
        )?;
        Ok(())
    }

    pub fn list_aggregate_apis(&self) -> Result<Vec<AggregateApi>> {
        let mut stmt = self.conn.prepare(&format!(
            "{AGGREGATE_API_SELECT_SQL} ORDER BY sort ASC, updated_at DESC"
        ))?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(map_aggregate_api_row(row)?);
        }
        Ok(out)
    }

    pub fn find_aggregate_api_by_id(&self, api_id: &str) -> Result<Option<AggregateApi>> {
        let mut stmt = self.conn.prepare(&format!(
            "{AGGREGATE_API_SELECT_SQL}
             WHERE id = ?1
             LIMIT 1"
        ))?;
        let mut rows = stmt.query([api_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_aggregate_api_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn update_aggregate_api(&self, api_id: &str, url: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET url = ?1, updated_at = ?2 WHERE id = ?3",
            (url, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_supplier_name(
        &self,
        api_id: &str,
        supplier_name: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET supplier_name = ?1, updated_at = ?2 WHERE id = ?3",
            (supplier_name, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_sort(&self, api_id: &str, sort: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET sort = ?1, updated_at = ?2 WHERE id = ?3",
            (sort, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn update_aggregate_api_type(&self, api_id: &str, provider_type: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE aggregate_apis SET provider_type = ?1, updated_at = ?2 WHERE id = ?3",
            (provider_type, now_ts(), api_id),
        )?;
        Ok(())
    }

    pub fn delete_aggregate_api(&self, api_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM aggregate_api_secrets WHERE aggregate_api_id = ?1",
            [api_id],
        )?;
        self.conn
            .execute("DELETE FROM aggregate_apis WHERE id = ?1", [api_id])?;
        Ok(())
    }

    pub fn upsert_aggregate_api_secret(&self, api_id: &str, secret_value: &str) -> Result<()> {
        let now = now_ts();
        self.conn.execute(
            "INSERT INTO aggregate_api_secrets (aggregate_api_id, secret_value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(aggregate_api_id) DO UPDATE SET
               secret_value = excluded.secret_value,
               updated_at = excluded.updated_at",
            (api_id, secret_value, now),
        )?;
        Ok(())
    }

    pub fn find_aggregate_api_secret_by_id(&self, api_id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT secret_value FROM aggregate_api_secrets WHERE aggregate_api_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query([api_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn update_aggregate_api_test_result(
        &self,
        api_id: &str,
        ok: bool,
        status_code: Option<i64>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = now_ts();
        let last_test_status = if ok { Some("success") } else { Some("failed") };
        self.conn.execute(
            "UPDATE aggregate_apis
             SET last_test_at = ?1,
                 last_test_status = ?2,
                 last_test_error = ?3,
                 updated_at = ?1
             WHERE id = ?4",
            (now, last_test_status, error, api_id),
        )?;
        if let Some(code) = status_code {
            if !ok {
                let message = format!("http_status={code}");
                self.conn.execute(
                    "UPDATE aggregate_apis SET last_test_error = ?1 WHERE id = ?2",
                    (message, api_id),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn ensure_aggregate_apis_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS aggregate_apis (
                id TEXT PRIMARY KEY,
                provider_type TEXT NOT NULL DEFAULT 'codex',
                supplier_name TEXT,
                sort INTEGER NOT NULL DEFAULT 0,
                url TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_test_at INTEGER,
                last_test_status TEXT,
                last_test_error TEXT
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_aggregate_apis_created_at ON aggregate_apis(created_at DESC)",
            [],
        )?;
        self.ensure_column("aggregate_apis", "provider_type", "TEXT")?;
        self.ensure_column("aggregate_apis", "supplier_name", "TEXT")?;
        self.ensure_column("aggregate_apis", "sort", "INTEGER DEFAULT 0")?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET provider_type = COALESCE(NULLIF(TRIM(provider_type), ''), 'codex')
             WHERE provider_type IS NULL OR TRIM(provider_type) = ''",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET sort = COALESCE(sort, 0)
             WHERE sort IS NULL",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET created_at = CAST(created_at AS INTEGER)
             WHERE typeof(created_at) = 'real'
                OR (typeof(created_at) = 'text' AND TRIM(created_at) != '')",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET updated_at = CAST(updated_at AS INTEGER)
             WHERE typeof(updated_at) = 'real'
                OR (typeof(updated_at) = 'text' AND TRIM(updated_at) != '')",
            [],
        )?;
        self.conn.execute(
            "UPDATE aggregate_apis
             SET last_test_at = CAST(last_test_at AS INTEGER)
             WHERE last_test_at IS NOT NULL
               AND (typeof(last_test_at) = 'real'
                 OR (typeof(last_test_at) = 'text' AND TRIM(last_test_at) != ''))",
            [],
        )?;
        Ok(())
    }

    pub(super) fn ensure_aggregate_api_secrets_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS aggregate_api_secrets (
                aggregate_api_id TEXT PRIMARY KEY REFERENCES aggregate_apis(id) ON DELETE CASCADE,
                secret_value TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_aggregate_api_secrets_updated_at ON aggregate_api_secrets(updated_at)",
            [],
        )?;
        Ok(())
    }
}

fn map_aggregate_api_row(row: &Row<'_>) -> Result<AggregateApi> {
    Ok(AggregateApi {
        id: row.get(0)?,
        provider_type: row.get(1)?,
        supplier_name: row.get(2)?,
        sort: row.get(3)?,
        url: row.get(4)?,
        status: row.get(5)?,
        created_at: row_i64_like(row, 6)?,
        updated_at: row_i64_like(row, 7)?,
        last_test_at: row_optional_i64_like(row, 8)?,
        last_test_status: row.get(9)?,
        last_test_error: row.get(10)?,
    })
}

fn row_i64_like(row: &Row<'_>, idx: usize) -> Result<i64> {
    match row.get_ref(idx)? {
        ValueRef::Integer(value) => Ok(value),
        ValueRef::Real(value) => Ok(value as i64),
        ValueRef::Text(bytes) => {
            let text = std::str::from_utf8(bytes).map_err(|err| {
                Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(err))
            })?;
            if let Ok(value) = text.trim().parse::<i64>() {
                return Ok(value);
            }
            let value = text.trim().parse::<f64>().map_err(|err| {
                Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(err))
            })?;
            Ok(value as i64)
        }
        value => Err(Error::InvalidColumnType(
            idx,
            format!("column_{idx}"),
            value.data_type(),
        )),
    }
}

fn row_optional_i64_like(row: &Row<'_>, idx: usize) -> Result<Option<i64>> {
    match row.get_ref(idx)? {
        ValueRef::Null => Ok(None),
        _ => row_i64_like(row, idx).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::Storage;

    #[test]
    fn list_aggregate_apis_accepts_real_timestamp_columns() {
        let storage = Storage::open_in_memory().expect("open in memory");
        storage.init().expect("init schema");

        storage
            .conn
            .execute(
                "INSERT INTO aggregate_apis (
                    id, provider_type, supplier_name, sort, url, status, created_at, updated_at, last_test_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    "ag_real_ts",
                    "codex",
                    "fizzly",
                    0_i64,
                    "https://fizzlycode.com/openai",
                    "active",
                    1774686327.125_f64,
                    1774686327.897_f64,
                    1774687310.4_f64,
                ),
            )
            .expect("insert aggregate api row");

        let items = storage.list_aggregate_apis().expect("list aggregate apis");
        let item = items
            .into_iter()
            .find(|api| api.id == "ag_real_ts")
            .expect("aggregate api exists");
        assert_eq!(item.created_at, 1774686327);
        assert_eq!(item.updated_at, 1774686327);
        assert_eq!(item.last_test_at, Some(1774687310));
    }

    #[test]
    fn ensure_aggregate_apis_table_normalizes_real_timestamps_to_integer() {
        let storage = Storage::open_in_memory().expect("open in memory");
        storage.init().expect("init schema");

        storage
            .conn
            .execute(
                "INSERT INTO aggregate_apis (
                    id, provider_type, supplier_name, sort, url, status, created_at, updated_at, last_test_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    "ag_real_ts_norm",
                    "codex",
                    "crs",
                    -5_i64,
                    "http://47.253.177.201:3000/openai",
                    "active",
                    1774686327.125_f64,
                    1774686327.897_f64,
                    1774687310.4_f64,
                ),
            )
            .expect("insert aggregate api row");

        storage
            .ensure_aggregate_apis_table()
            .expect("normalize aggregate api table");

        let (created_type, updated_type, last_test_type): (String, String, String) = storage
            .conn
            .query_row(
                "SELECT typeof(created_at), typeof(updated_at), typeof(last_test_at)
                 FROM aggregate_apis
                 WHERE id = 'ag_real_ts_norm'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query normalized types");
        assert_eq!(created_type, "integer");
        assert_eq!(updated_type, "integer");
        assert_eq!(last_test_type, "integer");
    }
}
