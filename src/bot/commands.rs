use anyhow::anyhow;
use poise::{CreateReply, command};

use crate::bot::{Context, Error};

#[command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let ping = ctx.ping().await;
    ctx.say(format!("Pong! {}ms", ping.as_millis())).await?;
    Ok(())
}

#[command(slash_command)]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();

    if ctx.author().id != data.user_id {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("指定されたユーザー以外は使用できません"),
        )
        .await?;
        return Ok(());
    }

    ctx.defer().await?;

    if data.connected_guild.read().await.is_some() {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("先に、すでに接続しているサーバーから退出させてください\nそのサーバーが分からない場合でもここで`/leave`を使用することで退出させることができます"),
        )
        .await?;

        return Ok(());
    }

    let (guild_id, channel_id) = {
        let guild = ctx.guild().ok_or_else(|| anyhow!("can't get guild"))?;
        let channel_id = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            ctx.send(
                CreateReply::new()
                    .ephemeral(true)
                    .content("ボイスチャットに参加してください"),
            )
            .await?;

            return Ok(());
        }
    };

    let manager = &data.songbird;

    if manager.get(guild_id).is_some() {
        ctx.say("すでに参加しています").await?;
        return Ok(());
    }

    if manager.join(guild_id, connect_to).await.is_ok() {
        *data.connected_guild.write().await = Some(guild_id);

        ctx.say("参加しました").await?;
    } else {
        ctx.say("なにかしらのエラーが発生しました").await?;
    }

    Ok(())
}

#[command(slash_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();

    if ctx.author().id != data.user_id {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("指定されたユーザー以外は使用できません"),
        )
        .await?;
        return Ok(());
    }

    ctx.defer().await?;

    let Some(guild_id) = *data.connected_guild.read().await else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("ボットがボイスチャットにいません"),
        )
        .await?;

        return Ok(());
    };

    let manager = &data.songbird;
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            ctx.say(format!("エラーが発生しました: {:?}", e)).await?;
        } else {
            *data.connected_guild.write().await = None;
            ctx.say("離脱しました").await?;
        }
    } else {
        ctx.send(
            CreateReply::new()
                .ephemeral(true)
                .content("ボットがボイスチャットにいません"),
        )
        .await?;
    }

    Ok(())
}
