mod app;
mod args;
mod config;
mod db;
mod timetables;
mod ui;

use app::App;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Args.
    let app_args = args::parse();

    App::run(app_args).await?;

    Ok(())
}
