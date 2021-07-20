use crate::database::Database;
use serenity::{
    builder::CreateComponents,
    client::Context,
    model::{
        gateway::Ready,
        id::{ChannelId, GuildId, RoleId},
        interactions::{
            ApplicationCommandInteractionData, ApplicationCommandInteractionDataOptionValue,
            ApplicationCommandOptionType, ButtonStyle, Interaction,
            InteractionApplicationCommandCallbackDataFlags, InteractionData,
            InteractionResponseType, InteractionType,
        },
    },
};
use std::env;
use tokio::sync::Mutex;

pub struct Handler {
    database: Mutex<Database>,
    guild_id: GuildId,
    channel_id: ChannelId,
}

impl Handler {
    pub async fn new() -> anyhow::Result<Self> {
        let database = Database::load().await?;
        let guild_id = GuildId(env::var("GUILD_ID")?.parse()?);
        let channel_id = ChannelId(env::var("CHANNEL_ID")?.parse()?);

        Ok(Self {
            database: Mutex::new(database),
            guild_id,
            channel_id,
        })
    }

    pub async fn ready(&self, ctx: Context, ready: Ready) -> anyhow::Result<()> {
        println!("{} is connected!", ready.user.name);

        self.guild_id
            .create_application_commands(&ctx.http, |commands| {
                commands.create_application_command(|command| {
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
            "role" => self.role_slash_command(data, &interaction, &ctx).await?,
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

    async fn role_slash_command(
        &self,
        data: &ApplicationCommandInteractionData,
        interaction: &Interaction,
        ctx: &Context,
    ) -> anyhow::Result<String> {
        let option_value = data.options[0].resolved.as_ref().unwrap();

        let role = if let ApplicationCommandInteractionDataOptionValue::Role(role) = option_value {
            role
        } else {
            unreachable!()
        };

        // interaction.member is always Some,
        // since this command only works when in a guild
        let member = interaction.member.as_ref().unwrap();

        let has_admin = member.permissions(&ctx).await?.administrator();
        if !has_admin {
            return Ok("You must be admin to toggle a role!".to_string());
        }

        let mut database = self.database.lock().await;
        database.toggle_role(role.id).await?;

        let roles = {
            let roles = database.roles();
            let mut vec = Vec::with_capacity(roles.len());
            for role_id in roles.iter().copied() {
                vec.push(role_id.to_role_cached(&ctx.cache).await.unwrap());
            }

            vec
        };

        let add_components = |components: &mut CreateComponents| {
            components.create_action_row(|row| {
                for role in &roles {
                    row.create_button(|button| {
                        button
                            .label(role.name.clone())
                            .style(ButtonStyle::Primary)
                            .custom_id(role.id.to_string())
                    });
                }

                row
            });
        };

        let button_message = database.button_message();
        if let Some(button_message) = button_message {
            button_message
                .edit(&ctx.http, |message| {
                    if !roles.is_empty() {
                        message.components(|components| {
                            add_components(components);
                            components
                        });
                    }
                    message
                })
                .await?;
        } else {
            let message = self
                .channel_id
                .send_message(&ctx.http, |message| {
                    message.content("Choose your roles:");

                    if !roles.is_empty() {
                        message.components(|components| {
                            add_components(components);
                            components
                        });
                    }

                    message
                })
                .await?;

            *button_message = Some(message);
        }

        Ok(format!("Toggled button for {} role.", role.name))
    }
}
