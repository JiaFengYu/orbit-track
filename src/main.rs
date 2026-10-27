use anyhow::{Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use orbit_track::data;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
#[derive(Parser)]
#[command(
    version,
    about = "Reproducible SGP4 predictions and an offline ground-track map"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, ValueEnum)]
enum Format {
    Json,
    Geojson,
}
#[derive(Subcommand)]
enum Command {
    Inspect {
        #[arg(long)]
        tle: PathBuf,
    },
    Predict {
        #[arg(long)]
        tle: PathBuf,
        #[arg(long, default_value = "epoch")]
        start: String,
        #[arg(long, default_value_t = 90)]
        minutes: u32,
        #[arg(long, default_value_t = 30)]
        step_seconds: u32,
        #[arg(long, value_enum, default_value = "json")]
        format: Format,
        #[arg(long)]
        teme: bool,
    },
    Fetch {
        #[arg(long, default_value = "celestrak")]
        provider: String,
        #[arg(long)]
        catalog_id: u64,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = ".orbit-cache")]
        cache: PathBuf,
        #[arg(long)]
        allow_stale: bool,
    },
    Serve {
        #[arg(long, default_value = "fixtures")]
        data: PathBuf,
        #[arg(long, default_value = "127.0.0.1:8788")]
        listen: SocketAddr,
    },
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inspect { tle } => println!(
            "{}",
            serde_json::to_string_pretty(&data::load(&tle)?.metadata())?
        ),
        Command::Predict {
            tle,
            start,
            minutes,
            step_seconds,
            format,
            teme,
        } => {
            let track = data::load(&tle)?.track(&start, minutes, step_seconds, teme)?;
            let output = match format {
                Format::Json => serde_json::to_value(track)?,
                Format::Geojson => track.geojson(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Command::Fetch {
            provider,
            catalog_id,
            output,
            cache,
            allow_stale,
        } => {
            ensure!(provider == "celestrak", "supported provider: celestrak");
            ensure!(
                (1..=99999).contains(&catalog_id),
                "supported catalog IDs: 1..99999"
            );
            let url = format!(
                "https://celestrak.org/NORAD/elements/gp.php?CATNR={catalog_id}&FORMAT=TLE"
            );
            let report = data::fetch(
                &url,
                catalog_id,
                &cache,
                &output,
                allow_stale,
                chrono::Utc::now(),
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Serve { data, listen } => {
            ensure!(
                listen.ip().is_loopback(),
                "MVP server is local-only; use SSH forwarding"
            );
            let catalog = Arc::new(data::catalog(&data)?);
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind(listen).await?;
                    eprintln!("Orbit explorer: http://{}", listener.local_addr()?);
                    axum::serve(listener, orbit_track::api::router(catalog))
                        .with_graceful_shutdown(async {
                            let _ = tokio::signal::ctrl_c().await;
                        })
                        .await
                })?;
        }
    }
    Ok(())
}
