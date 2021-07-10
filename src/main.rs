use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{gateway::Ready, interactions::Interaction},
    Client,
};
use std::env;

struct Handler {
    inner: button_role_bot::Handler,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Err(e) = self.inner.interaction_create(ctx, interaction).await {
            eprintln!("Error: {:?}", e.context("Failed responding to interaction"));
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        if let Err(e) = self.inner.ready(ctx, ready).await {
            eprintln!("Error: {:?}", e.context("Failed responding to ready event"));
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;

    let token = env::var("DISCORD_TOKEN")?;

    let application_id: u64 = env::var("APPLICATION_ID")?.parse()?;

    let handler = Handler {
        inner: button_role_bot::Handler::new().await?,
    };

    let mut client = Client::builder(token)
        .event_handler(handler)
        .application_id(application_id)
        .await?;

    client.start().await?;

    Ok(())
}
