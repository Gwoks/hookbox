//! `UpdateBuilder`: a partial `UPDATE <table> SET … WHERE … RETURNING *`
//! over sqlx's `QueryBuilder`, shared by both PATCH routes. Table and column
//! names are `&'static str` only (never caller data); every value is a bind.

use sqlx::sqlite::SqliteRow;
use sqlx::{Encode, FromRow, QueryBuilder, Sqlite, SqlitePool, Type};

pub(crate) struct UpdateBuilder<'args> {
    query: QueryBuilder<'args, Sqlite>,
    sets: usize,
    filters: usize,
}

impl<'args> UpdateBuilder<'args> {
    pub(crate) fn new(table: &'static str) -> Self {
        UpdateBuilder {
            query: QueryBuilder::new(format!("UPDATE {table} SET ")),
            sets: 0,
            filters: 0,
        }
    }

    /// `column = ?` with `value` bound. Every `set` precedes the filters.
    pub(crate) fn set<T>(&mut self, column: &'static str, value: T) -> &mut Self
    where
        T: 'args + Encode<'args, Sqlite> + Type<Sqlite>,
    {
        debug_assert_eq!(self.filters, 0, "set() after where_eq()");
        if self.sets > 0 {
            self.query.push(", ");
        }
        self.query.push(column).push(" = ").push_bind(value);
        self.sets += 1;
        self
    }

    /// `set` when the PATCH body carried the field; an absent field (`None`)
    /// leaves the column alone.
    pub(crate) fn set_some<T>(&mut self, column: &'static str, value: Option<T>) -> &mut Self
    where
        T: 'args + Encode<'args, Sqlite> + Type<Sqlite>,
    {
        if let Some(v) = value {
            self.set(column, v);
        }
        self
    }

    /// One `column = ?` row filter (joined with `AND`).
    pub(crate) fn where_eq<T>(&mut self, column: &'static str, value: T) -> &mut Self
    where
        T: 'args + Encode<'args, Sqlite> + Type<Sqlite>,
    {
        let joiner = if self.filters == 0 {
            " WHERE "
        } else {
            " AND "
        };
        self.query
            .push(joiner)
            .push(column)
            .push(" = ")
            .push_bind(value);
        self.filters += 1;
        self
    }

    /// Run the update and return the updated row. `Ok(None)` — without
    /// touching the database — when no column was set (an empty PATCH).
    pub(crate) async fn fetch_returning<R>(
        mut self,
        pool: &SqlitePool,
    ) -> Result<Option<R>, sqlx::Error>
    where
        R: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    {
        if self.sets == 0 {
            return Ok(None);
        }
        self.query.push(" RETURNING *");
        self.query
            .build_query_as::<R>()
            .fetch_one(pool)
            .await
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::endpoints::{find, EndpointRow};
    use crate::testing::pool_with_endpoint;

    #[tokio::test]
    async fn empty_update_is_a_no_op() {
        let (pool, _owner) = pool_with_endpoint("tok").await;
        let mut b = UpdateBuilder::new("endpoints");
        b.where_eq("token", "tok");
        let out: Option<EndpointRow> = b.fetch_returning(&pool).await.unwrap();
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn sets_bind_values_and_return_the_row() {
        let (pool, _owner) = pool_with_endpoint("tok").await;
        let mut b = UpdateBuilder::new("endpoints");
        b.set("name", Some("it's; DROP TABLE endpoints".to_string()))
            .set_some("latency_ms", Some(7_i64))
            .set_some("chaos_pct", None::<i64>)
            .set("cors_enabled", false)
            .where_eq("token", "tok");
        let row: EndpointRow = b.fetch_returning(&pool).await.unwrap().unwrap();
        assert_eq!(row.name.as_deref(), Some("it's; DROP TABLE endpoints"));
        assert_eq!(row.latency_ms, 7);
        assert_eq!(row.chaos_pct, 0);
        assert!(!row.cors_enabled);
        assert_eq!(find(&pool, "tok").await.unwrap().unwrap().latency_ms, 7);
    }
}
