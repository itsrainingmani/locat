use std::net::IpAddr;

/// Allows geo-locating IPs and keeps analytics
pub struct Locat {
    geoip: maxminddb::Reader<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("maxminddb error: {0}")]
    MaxMindDb(#[from] maxminddb::MaxMindDBError),
}

impl Locat {
    pub fn new(geoip_country_db_path: &str, _analytics_db_path: &str) -> Result<Self, Error> {
        // TODO: create analytics db

        Ok(Self {
            geoip: maxminddb::Reader::open_readfile(geoip_country_db_path)?,
        })
    }

    /// Converts an address to an ISO 3166-1 alpha-2 country code
    pub fn ip_to_iso_code(&self, addr: IpAddr) -> Option<&str> {
        self.geoip
            .lookup::<maxminddb::geoip2::Country>(addr)
            .ok()?
            .country?
            .iso_code
    }

    /// Returns a map of country codes to number of requests
    pub fn get_analytics(&self) -> Vec<(String, u64)> {
        Default::default()
    }
}
