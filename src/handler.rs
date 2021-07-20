use crate::database::Database;
use serenity::{
    builder::CreateComponents,
    client::Context,
    model::{
        gateway::Ready,
        guild::Role,
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
                    .create_application_command(|command| {
                        command
                            .name("channel")
                            .description("Pick the channel for the buttons")
                            .create_option(|option| {
                                option
                                    .name("channel")
                                    .description("The channel for the buttons message")
                                    .kind(ApplicationCommandOptionType::Channel)
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
            "channel" => self.channel_slash_command(data, &ctx).await?,
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

        let roles = roles(database.roles().iter().copied(), ctx).await;

        let mut was_there_channel = true;

        match (database.button_message_id(), database.channel_id()) {
            (Some(button_message_id), Some(channel_id)) => {
                let mut button_message = ctx
                    .http
                    .get_message(channel_id.0, button_message_id.0)
                    .await?;

                button_message
                    .edit(&ctx.http, |message| {
                        if !roles.is_empty() {
                            message.components(|components| {
                                add_components(&roles, components);
                                components
                            });
                        }
                        message
                    })
                    .await?;
            }

            (None, Some(channel_id)) => {
                self.send_button_message(ctx, &mut *database, channel_id, &roles)
                    .await?;
            }

            (None, None) => was_there_channel = false,

            // we can never have a message
            // but not a channel
            (Some(_), None) => unreachable!(),
        }

        let mut output = format!("Toggled button for {} role.", role.name);

        if !was_there_channel {
            output.push_str("\nYou have not selected a channel. Choose one with /channel.");
        }

        Ok(output)
    }

    async fn channel_slash_command(
        &self,
        data: &ApplicationCommandInteractionData,
        ctx: &Context,
    ) -> anyhow::Result<String> {
        let option_value = data.options[0].resolved.as_ref().unwrap();

        let channel =
            if let ApplicationCommandInteractionDataOptionValue::Channel(channel) = option_value {
                channel
            } else {
                unreachable!()
            };

        let mut database = self.database.lock().await;

        let button_message_id = database.button_message_id();
        let channel_id = database.channel_id();

        if let (Some(button_message_id), Some(channel_id)) = (button_message_id, channel_id) {
            ctx.http
                .delete_message(channel_id.0, button_message_id.0)
                .await?;
        }

        database.set_channel_id(channel.id).await?;

        let roles = roles(database.roles().iter().copied(), ctx).await;
        self.send_button_message(ctx, &mut *database, channel.id, &roles)
            .await?;

        Ok(format!("Set button channel to {}.", channel.name))
    }

    async fn send_button_message(
        &self,
        ctx: &Context,
        database: &mut Database,
        channel: ChannelId,
        roles: &[Role],
    ) -> anyhow::Result<()> {
        let message = channel
            .send_message(&ctx.http, |message| {
                message.content("Choose your roles:");

                if !roles.is_empty() {
                    message.components(|components| {
                        add_components(roles, components);
                        components
                    });
                }

                message
            })
            .await?;

        database.set_button_message_id(message.id).await?;

        Ok(())
    }
}

async fn roles(
    role_ids: impl Iterator<Item = RoleId> + ExactSizeIterator,
    ctx: &Context,
) -> Vec<Role> {
    let mut vec = Vec::with_capacity(role_ids.len());

    for role_id in role_ids {
        vec.push(role_id.to_role_cached(&ctx.cache).await.unwrap());
    }

    vec
}

fn add_components(roles: &[Role], components: &mut CreateComponents) {
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
    });
}
