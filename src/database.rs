use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, MessageId, RoleId};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Database {
    path: PathBuf,
    roles: IndexSet<RoleId>,
    channel_id: Option<ChannelId>,
    button_message_id: Option<MessageId>,
}

impl Database {
    pub(crate) async fn load() -> anyhow::Result<Self> {
        let path = path()?;
        if path.exists() {
            return Self::read(path).await;
        }
        Self::new(path).await
    }

    async fn new(path: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(path.parent().unwrap()).await?;

        let empty_db = Database {
            path,
            roles: IndexSet::new(),
            channel_id: None,
            button_message_id: None,
        };

        empty_db.save().await?;

        Ok(empty_db)
    }

    async fn read(path: PathBuf) -> anyhow::Result<Database> {
        let bytes = fs::read(&path).await?;
        let database = serde_json::from_slice(&bytes)?;

        Ok(database)
    }

    pub(crate) fn roles(&self) -> &IndexSet<RoleId> {
        &self.roles
    }

    pub(crate) fn button_message_id(&self) -> Option<MessageId> {
        self.button_message_id
    }

    pub(crate) async fn set_button_message_id(&mut self, id: MessageId) -> anyhow::Result<()> {
        self.button_message_id = Some(id);
        self.save().await?;

        Ok(())
    }

    pub(crate) fn channel_id(&self) -> Option<ChannelId> {
        self.channel_id
    }

    pub(crate) async fn set_channel_id(&mut self, id: ChannelId) -> anyhow::Result<()> {
        self.channel_id = Some(id);
        self.save().await?;

        Ok(())
    }

    pub(crate) async fn toggle_role(&mut self, role: RoleId) -> anyhow::Result<()> {
        if self.roles.contains(&role) {
            self.roles.remove(&role);
        } else {
            self.roles.insert(role);
        }

        self.save().await?;

        Ok(())
    }

    async fn save(&self) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        fs::write(&self.path, bytes).await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GuildData {
    roles: IndexSet<RoleId>,
    channel: Option<ChannelId>,
}

fn path() -> anyhow::Result<PathBuf> {
    Ok(Xdg::new(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "lhvy".to_string(),
        app_name: "button-role-bot".to_string(),
    })?
    .in_data_dir("roles.json"))
}
