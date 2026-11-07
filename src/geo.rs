use std::{error::Error, net::IpAddr, time::Duration};

use maxminddb::Reader;
use reqwest::{Client, header, redirect::Policy};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

pub struct GeoLookup {
    database: Option<Reader<&'static [u8]>>,
    online: Option<Client>,
    online_requests: Semaphore,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    ip: IpAddr,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_name: Option<String>,
}

#[derive(Default, Deserialize)]
struct GeoRecord {
    country_code: Option<String>,
    country: Option<String>,
    asn: Option<String>,
    as_name: Option<String>,
}

impl GeoRecord {
    fn normalize(mut self) -> Self {
        // The legacy database uses country for the ISO code. Lite uses country_code.
        self.country_code = self
            .country_code
            .or(self.country)
            .filter(|value| {
                value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
            })
            .map(|value| value.to_ascii_uppercase());
        self.country = None;
        self.asn = self.asn.filter(|value| !value.trim().is_empty());
        self.as_name = self.as_name.filter(|value| !value.trim().is_empty());
        self
    }

    fn has_details(&self) -> bool {
        self.country_code.is_some() || self.asn.is_some() || self.as_name.is_some()
    }
}

impl GeoLookup {
    pub fn new(api_key: Option<&str>) -> Result<Self, Box<dyn Error>> {
        #[cfg(rate_has_geoip_db)]
        let database = Some(Reader::from_source(
            &include_bytes!(env!("RATE_EMBEDDED_GEOIP_DB"))[..],
        )?);
        #[cfg(not(rate_has_geoip_db))]
        let database = None;

        let online = if let Some(api_key) = api_key {
            let mut authorization = header::HeaderValue::from_str(&format!("Bearer {api_key}"))?;
            authorization.set_sensitive(true);
            let mut headers = header::HeaderMap::new();
            headers.insert(header::AUTHORIZATION, authorization);
            Some(
                Client::builder()
                    .default_headers(headers)
                    .https_only(true)
                    .redirect(Policy::none())
                    .timeout(Duration::from_secs(2))
                    .build()?,
            )
        } else {
            None
        };

        Ok(Self {
            database,
            online,
            online_requests: Semaphore::new(16),
        })
    }

    pub async fn lookup(&self, ip: IpAddr) -> ClientInfo {
        let ip = ip.to_canonical();
        let record = if is_public(ip) {
            self.lookup_online(ip)
                .await
                .or_else(|| self.lookup_database(ip))
                .unwrap_or_default()
        } else {
            GeoRecord::default()
        };
        ClientInfo {
            ip,
            country_code: record.country_code,
            asn: record.asn,
            network_name: record.as_name,
        }
    }

    async fn lookup_online(&self, ip: IpAddr) -> Option<GeoRecord> {
        let online = self.online.as_ref()?;
        // Keep lookup available via the offline database when online capacity is exhausted.
        let _permit = self.online_requests.try_acquire().ok()?;
        let record = online
            .get(format!("https://api.ipinfo.io/lite/{ip}"))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<GeoRecord>()
            .await
            .ok()?
            .normalize();
        record.has_details().then_some(record)
    }

    fn lookup_database(&self, ip: IpAddr) -> Option<GeoRecord> {
        self.database
            .as_ref()?
            .lookup(ip)
            .ok()?
            .decode::<GeoRecord>()
            .ok()?
            .map(GeoRecord::normalize)
    }
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [first, second, ..] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_documentation()
                || ip.is_unspecified()
                || first == 0
                || first >= 224
                || (first == 100 && (64..=127).contains(&second)))
        }
        IpAddr::V6(ip) => {
            let [first, second, ..] = ip.segments();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || (first == 0x2001 && second == 0x0db8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lite_and_legacy_country_fields() {
        for json in [
            r#"{"country_code":"US","country":"United States","as_name":"Example"}"#,
            r#"{"country":"US","as_name":"Example"}"#,
        ] {
            let record = serde_json::from_str::<GeoRecord>(json).unwrap().normalize();
            assert_eq!(record.country_code.as_deref(), Some("US"));
            assert_eq!(record.as_name.as_deref(), Some("Example"));
        }
    }

    #[test]
    fn recognizes_local_addresses() {
        for ip in [
            "127.0.0.1",
            "192.168.1.1",
            "::1",
            "fe80::1",
            "fd00::1",
            "100.64.0.1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_public("8.8.8.8".parse().unwrap()));
        assert!(is_public("2001:4860:4860::8888".parse().unwrap()));
    }
}
