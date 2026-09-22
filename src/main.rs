use std::{path::PathBuf, sync::Arc};

use mysoundboard::{app::WebAppContext, bot::SignalFromWeb, sounds::Sounds};
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use tokio::sync::mpsc::Sender;

#[derive(Deserialize)]
struct Config {
    token: String,
    sounds_folder: PathBuf,
    user_id: serenity::UserId,
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = tokio::fs::read_to_string("./config.json").await.unwrap();
    let config: Config = serde_json::from_str(&config).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(10);

    let bot;

    #[cfg(feature = "backend")]
    {
        let token = config.token.clone();
        bot = Some(tokio::spawn(async move {
            mysoundboard::bot::bot(token.as_str(), config.user_id, rx).await?;

            Ok::<(), anyhow::Error>(())
        }));
    }

    tokio::select! {
        res = web(config.sounds_folder, tx) => {
            res?;
        }

        _ = tokio::signal::ctrl_c() => {}
    }

    if let Some(bot) = bot {
        bot.abort();
        bot.await??;
    }

    Ok(())
}

async fn web(sounds_folder: PathBuf, tx: Sender<SignalFromWeb>) -> std::io::Result<()> {
    use axum::Router;
    use leptos::{logging, prelude::*};
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use local_ip_address::local_ip;
    use qrcode::QrCode;

    use mysoundboard::app::{App, shell};

    tracing_subscriber::fmt().init();

    let conf = get_configuration(None).unwrap();

    let mut options = conf.leptos_options;
    options.site_addr.set_ip([0, 0, 0, 0].into());

    let addr = options.site_addr;

    let routes = generate_route_list(App);

    let sounds = Arc::new(Sounds::new(sounds_folder)?);
    let tx = Arc::new(tx);

    let additional_context = move || {
        let sounds = Arc::clone(&sounds);
        let tx = Arc::clone(&tx);
        provide_context(WebAppContext { sounds, tx });
    };

    let app = Router::new()
        .leptos_routes_with_context(&options, routes, additional_context.clone(), {
            let options = options.clone();

            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler_with_context(
            additional_context,
            shell,
        ))
        .with_state(options);

    logging::log!("listening on {}", addr);

    let qr = QrCode::new(format!("http://{}:{}", local_ip().unwrap(), addr.port())).unwrap();
    let string = qr.render::<qrcode::render::unicode::Dense1x2>().build();
    logging::log!("{}", string);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
