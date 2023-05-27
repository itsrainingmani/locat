use std::net::IpAddr;

/// Allows geo-locating IPs and keeps analytics
pub struct Locat {}

impl Locat {
    pub fn new(_geoip_country_db_path: &str, _analytics_db_path: &str) -> Self {
        // TODO: read geoip db, create analytics db
        Self {}
    }

    /// Converts an address to an ISO 3166-1 alpha-2 country code
    pub fn ip_to_iso_code(&self, _addr: IpAddr) -> Option<&str> {
        None
    }

    /// Returns a map of country codes to number of requests
    pub fn get_analytics(&self) -> Vec<(String, u64)> {
        Default::default()
    }
}
