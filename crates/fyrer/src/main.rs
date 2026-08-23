use anyhow::Result;

mod app;
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    let app = app::App::new();
    app.run().await?;
    Ok(())
}
