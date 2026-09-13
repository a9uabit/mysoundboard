use std::path::PathBuf;
use std::sync::Arc;

use leptos::{logging, prelude::*};
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{A, Route, Router, Routes},
    path,
};

use crate::common_sounds::Sound;

#[cfg(feature = "backend")]
use crate::{bot::SignalFromWeb, sounds::Sounds};

#[cfg(feature = "backend")]
use tokio::sync::mpsc::Sender;

#[cfg(feature = "backend")]
#[derive(Clone)]
pub struct WebAppContext {
    pub sounds: Arc<Sounds>,
    pub tx: Arc<Sender<SignalFromWeb>>,
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="ja">
            <head>
                <meta charset="utf-8" />
                <meta
                    name="viewport"
                    content="width=device-width, initial-scale=1"
                />

                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[server]
async fn fetch_sounds() -> Result<Vec<Sound>, ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context.sounds.fetch_new_sounds().await?;

    let sounds = context.sounds.data(|data| data.sounds.clone()).await?;

    Ok(sounds)
}

#[server]
async fn send_sound(path: PathBuf) -> Result<(), ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context.tx.send(SignalFromWeb::Play(path)).await?;

    Ok(())
}

#[server]
async fn stop_sound() -> Result<(), ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context.tx.send(SignalFromWeb::Stop).await?;

    Ok(())
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let sounds = Resource::new(|| (), |_| async { fetch_sounds().await });

    view! {
        <Title text="myサウンドボード" />
        <Stylesheet href="/assets/styles.css" />

        <div style="display: flex; gap: 6px;">
            <a
                class="btn"
                on:click=move |_| {
                    sounds.refetch();
                }
            >
                "再読み込み"
            </a>
            <a
                class="btn"
                on:click=move |_| {
                    leptos::task::spawn_local(async {
                        match stop_sound().await {
                            Ok(()) => {}
                            Err(e) => {
                                logging::error!("{:?}", e);
                            }
                        }
                    });
                }
            >
                "止める"
            </a>
        </div>
        
        <Router>
            <Menu />

            <Routes
                fallback=|| {
                    view! {
                        <p class="error">存在しないページです</p>
                    }
                }
            >
                <Route path=path!() view=move || view! { <All sounds /> } />
            </Routes>
        </Router>
    }
}

#[component]
fn MySuspense(children: Children) -> impl IntoView {
    view! {
        <Suspense
            fallback=|| {
                view! {
                    <p class="loading">がー</p>
                }
            }
        >
            {children()}
        </Suspense>
    }
}

#[component]
fn MyErrorBoundary(children: Children) -> impl IntoView {
    view! {
        <ErrorBoundary
            fallback=|e| {
                logging::error!("{:?}", e);

                view! {
                    <p class="error">Error Occurred! Please check the console.</p>
                }
            }
        >
            {children()}
        </ErrorBoundary>
    }
}

#[component]
fn Menu() -> impl IntoView {
    view! {
        <nav class="menu">
            <A href="/">"All"</A>
            <A href="/sets">"Sets"</A>
        </nav>
    }
}

#[component]
fn All(sounds: Resource<Result<Vec<Sound>, ServerFnError>>) -> impl IntoView {
    view! {
        <MySuspense>
            <MyErrorBoundary>
                <div class="sb">
                    {move || {
                        sounds.and_then(|sounds| {
                            sounds
                                .iter()
                                .map(|sound| {
                                    let path = sound.path.clone();
                                    view! {
                                        <a
                                            class="btn btn-sb"
                                            on:click=move |_| {
                                                let path = path.clone();
                                                leptos::task::spawn_local(async move {
                                                    match send_sound(path.clone()).await {
                                                        Ok(()) => {}
                                                        Err(err) => {
                                                            logging::error!("{:?}", err);
                                                        }
                                                    }
                                                });
                                            }
                                        >
                                            { path.display().to_string() }
                                        </a>
                                    }
                                })
                                .collect_view()
                        })
                    }}
                </div>
            </MyErrorBoundary>
        </MySuspense>
    }
}
