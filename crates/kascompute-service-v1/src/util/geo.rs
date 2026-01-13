use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    country: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

pub async fn geo_lookup(ip: IpAddr) -> Option<(f64, f64, Option<String>)> {
    // ip-api free: HTTP only
    let url = format!("http://ip-api.com/json/{ip}?fields=status,country,lat,lon");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;

    let resp = client
        .get(url)
        .header("User-Agent", "kascompute-protocol-v1/1.0")
        .send()
        .await
        .ok()?;

    let body: IpApiResponse = resp.json().await.ok()?;
    if body.status == "success" {
        if let (Some(lat), Some(lon)) = (body.lat, body.lon) {
            return Some((lat, lon, body.country));
        }
    }
    None
}
