use anyhow::Result;
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};

mod app;
mod domain;
mod routes;
mod state;
mod util;

use state::{AppState, JOB_SCHEDULE_EVERY_SEC};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = AppState::new(util::time::now_unix());

    // Background: job scheduler
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                let _ = s.create_scheduled_job(false).await;
                sleep(Duration::from_secs(JOB_SCHEDULE_EVERY_SEC)).await;
            }
        });
    }

    // Background: block miner (1 block per minute)
    {
        let s = state.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(60)).await;
                s.mine_new_block().await;
            }
        });
    }

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let app = app::build_app(state);

    println!("Protocol-v1 running on 0.0.0.0:{port}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}
