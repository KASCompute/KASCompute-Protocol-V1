use axum::http::HeaderMap;
use std::net::IpAddr;

/// Render/Proxy aware client ip extraction.
pub fn client_ip_from_headers(headers: &HeaderMap, fallback: IpAddr) -> IpAddr {
    if let Some(v) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first) = v
            .split(',')
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if let Ok(ip) = first.parse::<IpAddr>() {
                return ip;
            }
        }
    }
    if let Some(v) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        if let Ok(ip) = v.trim().parse::<IpAddr>() {
            return ip;
        }
    }
    fallback
}
