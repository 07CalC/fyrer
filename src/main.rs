use anyhow::Result;
use fyrer::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::new();
    app.run()?;
    Ok(())
}
