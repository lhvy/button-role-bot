use crate::database::Database;
use serenity::{
    client::Context,
    model::{
        gateway::Ready,
        id::GuildId,
        interactions::{
            ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            Interaction, InteractionData, InteractionResponseType, InteractionType,
        },
    },
};
use std::env;
use tokio::sync::Mutex;

pub struct Handler {
    database: Mutex<Database>,
}

impl Handler {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            database: Mutex::new(Database::load().await?),
        })
    }

    pub async fn ready(&self, ctx: Context, ready: Ready) -> anyhow::Result<()> {
        println!("{} is connected!", ready.user.name);

        GuildId(env::var("GUILD_ID")?.parse()?)
            .create_application_commands(&ctx.http, |commands| {
                commands
                    .create_application_command(|command| {
                        command.name("ping").description("A ping command")
                    })
                    .create_application_command(|command| {
                        command
                            .name("role")
                            .description("Toggle a role button")
                            .create_option(|option| {
                                option
                                    .name("role")
                                    .description("The role to toggle")
                                    .kind(ApplicationCommandOptionType::Role)
                                    .required(true)
                            })
                    })
            })
            .await?;

        Ok(())
    }

    pub async fn interaction_create(
        &self,
        ctx: Context,
        interaction: Interaction,
    ) -> anyhow::Result<()> {
        if interaction.kind != InteractionType::ApplicationCommand {
            return Ok(());
        }

        let data =
            if let Some(InteractionData::ApplicationCommand(data)) = interaction.data.as_ref() {
                data
            } else {
                return Ok(());
            };

        let content = match data.name.as_str() {
            "ping" => "pong".to_string(),
            "role" => {
                let option_value = data.options[0].resolved.as_ref().unwrap();

                if let ApplicationCommandInteractionDataOptionValue::Role(role) = option_value {
                    if let Some(guild) = interaction.guild_id {
                        let mut database = self.database.lock().await;
                        database.toggle_role(guild, role.id).await?;

                        format!("Toggled button for {} role.", role.name)
                    } else {
                        "This command can only be used in servers.".to_string()
                    }
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        };

        interaction
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content(content))
            })
            .await?;

        Ok(())
    }
}
