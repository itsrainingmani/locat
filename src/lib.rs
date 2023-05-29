use std::net::IpAddr;

/// Allows geo-locating IPs and keeps analytics
pub struct Locat {
    geoip: maxminddb::Reader<Vec<u8>>,
    analytics: Db,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("maxminddb error: {0}")]
    MaxMindDb(#[from] maxminddb::MaxMindDBError),

    #[error("rusqlite error: {0}")]
    Rusqlite(#[from] rusqlite::Error),
}

impl Locat {
    pub fn new(geoip_country_db_path: &str, analytics_db_path: &str) -> Result<Self, Error> {
        // TODO: create analytics db

        Ok(Self {
            geoip: maxminddb::Reader::open_readfile(geoip_country_db_path)?,
            analytics: Db {
                path: analytics_db_path.to_string(),
            },
        })
    }

    /// Converts an address to an ISO 3166-1 alpha-2 country code
    pub fn ip_to_iso_code(&self, addr: IpAddr) -> Option<&str> {
        let iso_code = self
            .geoip
            .lookup::<maxminddb::geoip2::Country>(addr)
            .ok()?
            .country?
            .iso_code?;

        if let Err(e) = self.analytics.increment(iso_code) {
            eprintln!("Could not increment analytics: {e}");
        }

        Some(iso_code)
    }

    /// Returns a map of country codes to number of requests
    pub fn get_analytics(&self) -> Result<Vec<(String, u64)>, Error> {
        Ok(self.analytics.list()?)
    }
}

struct Db {
    path: String,
}

impl Db {
    fn list(&self) -> Result<Vec<(String, u64)>, rusqlite::Error> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT iso_code, count FROM analytics")?;
        let mut rows = stmt.query([])?;
        let mut analytics = Vec::new();
        while let Some(row) = rows.next()? {
            let iso_code: String = row.get(0)?;
            let count: u64 = row.get(1)?;
            analytics.push((iso_code, count));
        }

        Ok(analytics)
    }

    fn increment(&self, iso_code: &str) -> Result<(), rusqlite::Error> {
        let conn = self.get_conn().unwrap();
        let mut stmt = conn
            .prepare("INSERT INTO analytics (iso_code, count) VALUES (?, 1) ON CONFLICT (iso_code) DO UPDATE SET count = count + 1")?;

        stmt.execute([iso_code])?;
        Ok(())
    }

    fn get_conn(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        let conn = rusqlite::Connection::open(&self.path).unwrap();
        self.migrate(&conn)?;
        Ok(conn)
    }

    fn migrate(&self, conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // create analytics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics (
                iso_code TEXT PRIMARY KEY,
                count INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Db;

    struct RemoveOnDrop {
        path: String,
    }

    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn test_db() {
        let db = Db {
            path: "/tmp/locat-test.db".to_string(),
        };
        let _remove_on_drop = RemoveOnDrop {
            path: db.path.clone(),
        };

        let analytics = db.list().unwrap();
        assert_eq!(analytics.len(), 0);

        db.increment("US").unwrap();
        let analytics = db.list().unwrap();
        assert_eq!(analytics.len(), 1);

        db.increment("US").unwrap();
        db.increment("FR").unwrap();
        let analytics = db.list().unwrap();
        assert_eq!(analytics.len(), 2);
        // contains US at count 2
        assert!(analytics.contains(&("US".to_string(), 2)));
        // contains FR at count 1
        assert!(analytics.contains(&("FR".to_string(), 1)));
        // doesn't contain DE
        assert!(!analytics.contains(&("DE".to_string(), 0)));
    }
}
