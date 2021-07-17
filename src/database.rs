use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, RoleId};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Database {
    path: PathBuf,
    guilds: HashMap<GuildId, HashSet<RoleId>>,
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
            guilds: HashMap::new(),
        };

        empty_db.save().await?;

        Ok(empty_db)
    }

    async fn read(path: PathBuf) -> anyhow::Result<Database> {
        let bytes = fs::read(&path).await?;
        let guilds = serde_json::from_slice(&bytes)?;

        Ok(Database { path, guilds })
    }

    pub(crate) fn guild_roles(&mut self, guild: GuildId) -> &HashSet<RoleId> {
        self.guilds.entry(guild).or_default()
    }

    pub(crate) async fn toggle_role(&mut self, guild: GuildId, role: RoleId) -> anyhow::Result<()> {
        let guild_roles = self.guilds.entry(guild).or_default();
        if guild_roles.contains(&role) {
            guild_roles.remove(&role);
        } else {
            guild_roles.insert(role);
        }

        self.clean();
        self.save().await?;

        Ok(())
    }

    fn clean(&mut self) {
        for (guild, roles) in self.guilds.clone() {
            if roles.is_empty() {
                self.guilds.remove(&guild);
            }
        }
    }

    async fn save(&self) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(&self.guilds)?;
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
