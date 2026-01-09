use serde::Deserialize;
use std::net::IpAddr;

#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    country: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

pub async fn geo_lookup(ip: IpAddr) -> Option<(f64, f64, Option<String>)> {
    let url = format!("https://ip-api.com/json/{ip}?fields=status,country,lat,lon");
    let resp = reqwest::get(&url).await.ok()?;
    let body: IpApiResponse = resp.json().await.ok()?;
    if body.status == "success" {
        if let (Some(lat), Some(lon)) = (body.lat, body.lon) {
            return Some((lat, lon, body.country));
        }
    }
    None
}
