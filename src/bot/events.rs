use std::sync::Arc;

use poise::{
    FrameworkContext,
    serenity_prelude::{FullEvent, GuildId},
};
use songbird::Songbird;
use tokio::sync::{RwLock, mpsc::Receiver};

use crate::bot::{Data, Error, SignalFromWeb};

pub async fn handle(
    framework: FrameworkContext<'_, Data, Error>,
    event: &FullEvent,
) -> Result<(), Error> {
    if let FullEvent::VoiceStateUpdate { old, new } = event {
        let was_connected = old.as_ref().and_then(|state| state.channel_id).is_some();

        if new.user_id == framework.bot_id() && was_connected && new.channel_id.is_none() {
            let songbird = &framework.user_data.songbird;
            let mut connected = framework.user_data.connected_guild.write().await;

            if let Some(guild_id) = new.guild_id
                && connected.is_some_and(|connected| connected == guild_id)
            {
                // it means the bot is already not in voice chat

                songbird.remove(guild_id).await?;
                *connected = None;
            }
        }
    }
    Ok(())
}

pub async fn receive_signal(
    songbird: Arc<Songbird>,
    mut rx: Receiver<SignalFromWeb>,
    connected_guild: Arc<RwLock<Option<GuildId>>>,
) {
    loop {
        if let Some(signal) = rx.recv().await {
            match signal {
                SignalFromWeb::Play(path) => {
                    if let Some(connected_guild) = *connected_guild.read().await
                        && let Some(handler) = songbird.get(GuildId::new(connected_guild.into()))
                        && let Ok(bytes) = tokio::fs::read(path.clone()).await
                    {
                        handler.lock().await.play(bytes.into());
                    }
                }

                SignalFromWeb::Stop => {
                    if let Some(connected_guild) = *connected_guild.read().await
                        && let Some(handler) = songbird.get(GuildId::new(connected_guild.into()))
                    {
                        handler.lock().await.stop();
                    }
                }
            }
        }
    }
}
