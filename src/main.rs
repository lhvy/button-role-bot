use button_role_bot::Database;
use serenity::{
    async_trait,
    model::{
        gateway::Ready,
        id::GuildId,
        interactions::{
            ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            Interaction, InteractionData, InteractionResponseType, InteractionType,
        },
    },
    prelude::*,
};
use std::env;
use tokio::sync::Mutex;

struct Handler {
    database: Mutex<Database>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if interaction.kind != InteractionType::ApplicationCommand {
            return;
        }

        let data =
            if let Some(InteractionData::ApplicationCommand(data)) = interaction.data.as_ref() {
                data
            } else {
                return;
            };

        let content = match data.name.as_str() {
            "ping" => "pong".to_string(),
            "role" => {
                let option_value = data.options[0].resolved.as_ref().unwrap();

                if let ApplicationCommandInteractionDataOptionValue::Role(role) = option_value {
                    if let Some(guild) = interaction.guild_id {
                        let mut database = self.database.lock().await;
                        database.toggle_role(guild, role.id).await.unwrap();

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

        if let Err(why) = interaction
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content(content))
            })
            .await
        {
            println!("Cannot respond to slash command: {}", why);
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let result = GuildId(env::var("GUILD_ID").unwrap().parse().unwrap())
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
            .await;

        if let Err(error) = result {
            eprintln!("Error: {}", error);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;

    let token = env::var("DISCORD_TOKEN")?;

    let application_id: u64 = env::var("APPLICATION_ID")?.parse()?;

    let handler = Handler {
        database: Mutex::new(Database::load().await?),
    };

    let mut client = Client::builder(token)
        .event_handler(handler)
        .application_id(application_id)
        .await?;

    client.start().await?;

    Ok(())
}
