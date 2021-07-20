use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serenity::model::{channel::Message, id::RoleId};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Database {
    path: PathBuf,
    roles: IndexSet<RoleId>,
    button_message: Option<Message>,
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
            button_message: None,
        };

        empty_db.save().await?;

        Ok(empty_db)
    }

    async fn read(path: PathBuf) -> anyhow::Result<Database> {
        let bytes = fs::read(&path).await?;
        let database = serde_json::from_slice(&bytes)?;

        Ok(database)
    }

    pub(crate) fn button_message(&mut self) -> &mut Option<Message> {
        &mut self.button_message
    }

    pub(crate) fn roles(&self) -> &IndexSet<RoleId> {
        &self.roles
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

fn path() -> anyhow::Result<PathBuf> {
    Ok(Xdg::new(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "lhvy".to_string(),
        app_name: "button-role-bot".to_string(),
    })?
    .in_data_dir("roles.json"))
}
