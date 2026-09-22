mod commands;
mod events;

use std::{path::PathBuf, sync::Arc};

use poise::{
    FrameworkError, FrameworkOptions, PrefixFrameworkOptions,
    serenity_prelude::{ClientBuilder, GatewayIntents, GuildId},
};
use songbird::Songbird;
use tokio::sync::{RwLock, mpsc::Receiver};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
type Context<'a> = poise::Context<'a, Data, Error>;

struct Data {
    songbird: Arc<Songbird>,
    connected_guild: Arc<RwLock<Option<GuildId>>>,
}

pub enum SignalFromWeb {
    Stop,
    Play(PathBuf),
}

pub async fn bot(token: &str, rx: Receiver<SignalFromWeb>) -> Result<(), anyhow::Error> {
    let framework_options = FrameworkOptions {
        commands: vec![commands::ping(), commands::join(), commands::leave()],
        event_handler: |framework, event| Box::pin(events::handle(framework, event)),
        prefix_options: PrefixFrameworkOptions {
            mention_as_prefix: false,
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        ..Default::default()
    };

    let songbird = Songbird::serenity();

    let framework = poise::Framework::builder()
        .setup({
            let songbird = Arc::clone(&songbird);
            |ctx, ready, framework| {
                Box::pin(async move {
                    tracing::info!("logged in as {}", ready.user.tag());
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                    let connected_guild = Default::default();
                    {
                        let songbird = Arc::clone(&songbird);
                        let connected_guild = Arc::clone(&connected_guild);

                        setup(songbird, rx, connected_guild);
                    }

                    Ok(Data {
                        songbird,
                        connected_guild,
                    })
                })
            }
        })
        .options(framework_options)
        .build();

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = ClientBuilder::new(token, intents)
        .voice_manager_arc(songbird)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

fn setup(
    songbird: Arc<Songbird>,
    rx: Receiver<SignalFromWeb>,
    connected_guild: Arc<RwLock<Option<GuildId>>>,
) {
    tokio::spawn(async move {
        events::receive_signal(songbird, rx, connected_guild).await;
    });
}

async fn on_error(error: FrameworkError<'_, Data, Error>) {
    match error {
        FrameworkError::Setup { error, .. } => {
            tracing::error!("Failed to start bot: {:?}", error);
            panic!("{:?}", error);
        }
        FrameworkError::Command { error, ctx, .. } => {
            tracing::error!("Error in command {}: {:?}", ctx.command().name, error)
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                tracing::error!("Error while handling error: {}", e);
            }
        }
    }
}
