use crate::database::Database;
use serenity::{
    client::Context,
    model::{
        gateway::Ready,
        id::{GuildId, RoleId},
        interactions::{
            ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            ButtonStyle, Interaction, InteractionApplicationCommandCallbackDataFlags,
            InteractionData, InteractionResponseType, InteractionType,
        },
    },
};
use std::env;
use tokio::sync::Mutex;

pub struct Handler {
    database: Mutex<Database>,
    guild_id: GuildId,
}

impl Handler {
    pub async fn new() -> anyhow::Result<Self> {
        let database = Database::load().await?;
        let guild_id = GuildId(env::var("GUILD_ID")?.parse()?);

        Ok(Self {
            database: Mutex::new(database),
            guild_id,
        })
    }

    pub async fn ready(&self, ctx: Context, ready: Ready) -> anyhow::Result<()> {
        println!("{} is connected!", ready.user.name);

        self.guild_id
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
        match interaction.kind {
            InteractionType::ApplicationCommand => {
                self.interaction_application_command(ctx, interaction).await
            }
            InteractionType::MessageComponent => self.interaction_button(ctx, interaction).await,
            _ => Ok(()),
        }
    }

    async fn interaction_application_command(
        &self,
        ctx: Context,
        interaction: Interaction,
    ) -> anyhow::Result<()> {
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

                        // we know there will be a channel ID since there is a guild ID
                        let channel_id = interaction.channel_id.unwrap();

                        let roles = {
                            let guild_roles = database.guild_roles(self.guild_id);
                            let mut roles = Vec::with_capacity(guild_roles.len());
                            for role_id in guild_roles.iter().copied() {
                                roles.push(role_id.to_role_cached(&ctx.cache).await.unwrap());
                            }

                            roles
                        };

                        channel_id
                            .send_message(&ctx.http, |message| {
                                message.content("This is a test");

                                if roles.is_empty() {
                                    return message;
                                }

                                message.components(|components| {
                                    components.create_action_row(|row| {
                                        for role in roles {
                                            row.create_button(|button| {
                                                button
                                                    .label(role.name.clone())
                                                    .style(ButtonStyle::Primary)
                                                    .custom_id(role.id.to_string())
                                            });
                                        }

                                        row
                                    })
                                })
                            })
                            .await?;

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

    async fn interaction_button(
        &self,
        ctx: Context,
        mut interaction: Interaction,
    ) -> anyhow::Result<()> {
        let data = if let Some(InteractionData::MessageComponent(data)) = interaction.data.as_ref()
        {
            data
        } else {
            return Ok(());
        };

        let role_id: RoleId = data.custom_id.parse().unwrap();
        let role_name = role_id.to_role_cached(&ctx.cache).await.unwrap().name;

        // can only be triggered from a guild
        let member = interaction.member.as_mut().unwrap();

        let does_user_already_have_role = member.roles.contains(&role_id);

        if does_user_already_have_role {
            member.remove_role(&ctx.http, role_id).await?;
        } else {
            member.add_role(&ctx.http, role_id).await?;
        }

        interaction
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| {
                        if does_user_already_have_role {
                            message.content(format!("Removed role {}", role_name));
                        } else {
                            message.content(format!("Added role {}", role_name));
                        }

                        message.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL)
                    })
            })
            .await?;

        Ok(())
    }
}
