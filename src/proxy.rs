use std::net::IpAddr;

use axum::http::HeaderMap;

pub fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> IpAddr {
    let peer = peer.to_canonical();
    if !trusted_proxies.contains(&peer) {
        return peer;
    }

    forwarded_client(headers, trusted_proxies).unwrap_or(peer)
}

fn forwarded_client(headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> Option<IpAddr> {
    let values: Vec<_> = headers.get_all("x-forwarded-for").iter().collect();

    // Start with the closest hop. Entries before the first untrusted hop are unverified.
    for value in values.into_iter().rev() {
        for address in value.to_str().ok()?.rsplit(',') {
            let address = address.trim().parse::<IpAddr>().ok()?.to_canonical();
            if !trusted_proxies.contains(&address) {
                return Some(address);
            }
        }
    }
    None
}
