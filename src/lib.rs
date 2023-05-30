use std::net::IpAddr;

// We're using tokio-rusqlite's own Connection type now
use tokio_rusqlite::Connection;

/// Allows geo-locating IPs and keeps analytics
pub struct Locat {
    reader: maxminddb::Reader<Vec<u8>>,
    analytics: Db,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("maxminddb error: {0}")]
    MaxMindDb(#[from] maxminddb::MaxMindDBError),

    // this can happen while reading the geoip db from disk
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("rusqlite error: {0}")]
    Rusqlite(#[from] rusqlite::Error),
}

impl Locat {
    pub async fn new(geoip_country_db_path: &str, analytics_db_path: &str) -> Result<Self, Error> {
        // read geoip db into memory asynchronously
        let geoip_data = tokio::fs::read(geoip_country_db_path).await?;

        Ok(Self {
            reader: maxminddb::Reader::from_source(geoip_data)?,
            analytics: Db::open(analytics_db_path).await?,
        })
    }

    /// Converts an address to an ISO 3166-1 alpha-2 country code
    pub async fn ip_to_iso_code(&self, addr: IpAddr) -> Option<&str> {
        let iso_code = self
            .reader
            .lookup::<maxminddb::geoip2::Country>(addr)
            .ok()?
            .country?
            .iso_code?;

        if let Err(e) = self.analytics.increment(iso_code).await {
            eprintln!("Could not increment analytics: {e}");
        }

        Some(iso_code)
    }

    /// Returns a map of country codes to number of requests
    pub async fn get_analytics(&self) -> Result<Vec<(String, u64)>, Error> {
        Ok(self.analytics.list().await?)
    }
}

struct Db {
    conn: Connection
}

impl Db {
    async fn open(path: &str) -> Result<Self, rusqlite::Error> {
        // open and migrate a db in a non-blocking way
        let conn = Connection::open(path).await?;

        // this is how operations are run on a thread pool: we pass a
        // closure. not that it must be `'static`, so we can't borrow
        // anything from the outside: owned types only.
        conn.call(|conn| {
            // create analytics table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS analytics (
                iso_code TEXT PRIMARY KEY,
                count INTEGER NOT NULL
            )",
                [],
            )?;

            Ok::<_, rusqlite::Error>(())
        })
        .await?;

        Ok(Self { conn })
    }

    async fn list(&self) -> Result<Vec<(String, u64)>, rusqlite::Error> {
        self.conn
            .call(|conn| {
                let mut stmt = conn.prepare("SELECT iso_code, count FROM analytics")?;
                let mut rows = stmt.query([])?;
                let mut analytics = Vec::new();
                while let Some(row) = rows.next()? {
                    let iso_code: String = row.get(0)?;
                    let count: u64 = row.get(1)?;
                    analytics.push((iso_code, count));
                }
                Ok(analytics)
            })
            .await
    }

    async fn increment(&self, iso_code: &str) -> Result<(), rusqlite::Error> {
        // we have to use `iso_code` from within the closure and the closure
        // must be 'static, so:
        let iso_code = iso_code.to_owned();

        self.conn.call(|conn| {
            let mut stmt = conn
                .prepare("INSERT INTO analytics (iso_code, count) VALUES (?, 1) ON CONFLICT (iso_code) DO UPDATE SET count = count + 1")
                ?;
            stmt.execute([iso_code])?;
            Ok(())
        }).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Db;

    struct RemoveOnDrop {
        path: &'static str,
    }

    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            _ = std::fs::remove_file(self.path);
        }
    }

    // this test needs an async runtime now, hence, `tokio::test`
    #[tokio::test]
    async fn test_db() {
        let path = "/tmp/loca-test.db";
        let db = Db::open(path).await.unwrap();

        let _remove_on_drop = RemoveOnDrop { path };

        let analytics = db.list().await.unwrap();
        assert_eq!(analytics.len(), 0);

        db.increment("US").await.unwrap();
        let analytics = db.list().await.unwrap();
        assert_eq!(analytics.len(), 1);

        db.increment("US").await.unwrap();
        db.increment("FR").await.unwrap();
        let analytics = db.list().await.unwrap();
        assert_eq!(analytics.len(), 2);
        // contains US at count 2
        assert!(analytics.contains(&("US".to_string(), 2)));
        // contains FR at count 1
        assert!(analytics.contains(&("FR".to_string(), 1)));
        // doesn't contain DE
        assert!(!analytics.contains(&("DE".to_string(), 0)));
    }
}
