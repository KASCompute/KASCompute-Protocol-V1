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

#[derive(Debug, Deserialize)]
struct IpWhoResponse {
    success: Option<bool>,
    country: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

/// Geo lookup with fallback providers.
/// 1) ip-api (HTTP only on free tier)
/// 2) ipwho.is (HTTPS, no key)
pub async fn geo_lookup(ip: IpAddr) -> Option<(f64, f64, Option<String>)> {
    // shared client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .ok()?;

    // ---- Provider 1: ip-api (free = HTTP only)
    if let Some(v) = geo_ip_api(&client, ip).await {
        return Some(v);
    }

    // ---- Provider 2: ipwho.is (HTTPS)
    geo_ipwho(&client, ip).await
}

async fn geo_ip_api(
    client: &reqwest::Client,
    ip: IpAddr,
) -> Option<(f64, f64, Option<String>)> {
    let url = format!("http://ip-api.com/json/{ip}?fields=status,country,lat,lon");

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

async fn geo_ipwho(
    client: &reqwest::Client,
    ip: IpAddr,
) -> Option<(f64, f64, Option<String>)> {
    // ipwho supports both /<ip> and query param
    let url = format!("https://ipwho.is/{ip}");

    let resp = client
        .get(url)
        .header("User-Agent", "kascompute-protocol-v1/1.0")
        .send()
        .await
        .ok()?;

    let body: IpWhoResponse = resp.json().await.ok()?;
    if body.success == Some(false) {
        return None;
    }

    let lat = body.latitude?;
    let lon = body.longitude?;
    Some((lat, lon, body.country))
}
