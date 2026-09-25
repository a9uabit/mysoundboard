use std::path::PathBuf;

use leptos::{logging, prelude::*};
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{A, Route, Router, Routes},
    hooks::use_params_map,
    path,
};
use slotmap::{Key, KeyData, SlotMap};

use crate::common_sounds::{Set, SetKey, Sound, SoundKey};

#[cfg(feature = "backend")]
use crate::{bot::SignalFromWeb, sounds::Sounds};

#[cfg(feature = "backend")]
use tokio::sync::mpsc::Sender;

#[cfg(feature = "backend")]
use std::sync::Arc;

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
async fn fetch_sounds() -> Result<SlotMap<SoundKey, Sound>, ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context.sounds.fetch_new_sounds().await?;

    let sounds = context.sounds.data(|data| data.sounds.clone()).await?;

    Ok(sounds)
}

#[server]
async fn fetch_sets() -> Result<SlotMap<SetKey, Set>, ServerFnError> {
    let context = expect_context::<WebAppContext>();

    let sets = context.sounds.data(|data| data.sets.clone()).await?;

    Ok(sets)
}

#[server]
async fn add_set(name: String) -> Result<(), ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context
        .sounds
        .data_mut(|data| {
            data.sets.insert(Set {
                name,
                sounds: Default::default(),
            });
        })
        .await?;

    Ok(())
}

#[server]
async fn delete_set(set_key: SetKey) -> Result<(), ServerFnError> {
    let context = expect_context::<WebAppContext>();

    context
        .sounds
        .data_mut(|data| {
            data.sets.remove(set_key);
        })
        .await?;

    Ok(())
}

#[server]
async fn fetch_set(id: SetKey) -> Result<Option<Vec<Sound>>, ServerFnError> {
    let context = expect_context::<WebAppContext>();

    let sounds = context
        .sounds
        .data(|data| {
            data.sets.get(id).map(|set| {
                set.sounds
                    .iter()
                    .filter_map(|sound_key| data.sounds.get(*sound_key))
                    .cloned()
                    .collect::<Vec<Sound>>()
            })
        })
        .await?;

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

#[server]
async fn set_sound_name(id: SoundKey, name: String) -> Result<(), ServerFnError> {
    let context = expect_context::<WebAppContext>();

    let name = name.trim();
    let res = context
        .sounds
        .data_mut::<Result<(), String>>(move |data| {
            let sound = data
                .sounds
                .get_mut(id)
                .ok_or("sound not found".to_owned())?;
            if !name.is_empty() {
                sound.name = Some(name.to_owned());
            } else {
                sound.name = None;
            }

            Ok(())
        })
        .await?;

    if let Err(e) = res {
        return Err(ServerFnError::ServerError(e));
    }

    Ok(())
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let sounds_res = Resource::new(|| (), |_| async { fetch_sounds().await });
    let sets = Resource::new(|| (), |_| async { fetch_sets().await });
    let set_reload = RwSignal::new(None::<Callback<()>>);

    view! {
        <Title text="myサウンドボード" />
        <Stylesheet href="/assets/styles.css" />

        <div style="display: flex; gap: 6px;">
            <a
                class="btn"
                on:click=move |_| {
                    sounds_res.refetch();
                    sets.refetch();

                    if let Some(reload) = set_reload.get_untracked() {
                        reload.run(());
                    }
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
            <Menu sets />

            <Routes
                fallback=|| {
                    view! {
                        <p class="error">存在しないページです</p>
                    }
                }
            >
                <Route path=path!() view=move || view! { <All sounds_res /> } />
                <Route path=path!("sets") view=move || view! { <Sets sets /> } />
                <Route path=path!("set/:id") view=move || view! { <Set set_reload /> } />
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
                e.with(|errors| {
                    for (_, error) in errors.iter() {
                        logging::error!("{}", error);
                    }
                });

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
fn Menu(sets: Resource<Result<SlotMap<SetKey, Set>, ServerFnError>>) -> impl IntoView {
    let show_sets = move || {
        sets.and_then(|sets| {
            sets.iter()
                .map(|set| {
                    let id = set.0.data().as_ffi();
                    let name = set.1.name.clone();

                    view! {
                        <A href=format!("/set/{}", id)>{name}</A>
                    }
                })
                .collect_view()
        })
    };

    view! {
        <nav class="menu">
            <A href="/">"すべて"</A>
            <A href="/sets">"セットの設定"</A>

            <Transition>
                {show_sets}
            </Transition>
        </nav>
    }
}

fn get_name(path: PathBuf, name: Option<String>) -> String {
    if let Some(name) = name {
        name
    } else {
        path.display().to_string()
    }
}

#[component]
fn All(sounds_res: Resource<Result<SlotMap<SoundKey, Sound>, ServerFnError>>) -> impl IntoView {
    let select = RwSignal::new(false);

    let click_sound = move |id: SoundKey, path: PathBuf, name: Option<String>| {
        if select.get() {
            match window().prompt_with_message_and_default(
                format!(
                    "ファイルパス: {}\nこのサウンドの名前を入れてください\n空欄にした場合はファイルパスが表示されます",
                    path.display()
                )
                .as_str(),
                &name.clone().unwrap_or_default(),
            ) {
                Ok(Some(name)) => {
                    let name_cloned = name.clone();
                    leptos::task::spawn_local(async move {
                        match set_sound_name(id, name_cloned).await {
                            Ok(()) => {}
                            Err(e) => {
                                logging::error!("{:?}", e);
                            }
                        }
                    });

                    return Some(name);
                }

                Ok(None) => {}

                Err(e) => {
                    logging::error!("{:?}", e);
                }
            }
        } else {
            leptos::task::spawn_local(async move {
                match send_sound(path.clone()).await {
                    Ok(()) => {}
                    Err(e) => {
                        logging::error!("{:?}", e);
                    }
                }
            });
        }

        name
    };

    let show_sounds = move || {
        sounds_res.and_then(|sounds| {
            sounds
                .iter()
                .map(|(id, sound)| {
                    let name = RwSignal::new(sound.name.clone());
                    let path = sound.path.clone();
                    view! {
                        <a
                            class="btn btn-sb"
                            on:click={
                                let path = path.clone();
                                move |_| {
                                    let path = path.clone();
                                    *name.write() = click_sound(id, path, name.get_untracked());
                                }
                            }
                        >
                            {
                                let path = path.clone();
                                move || get_name(path.clone(), name.get())
                            }
                        </a>
                    }
                })
                .collect_view()
        })
    };

    view! {
        <MySuspense>
            <MyErrorBoundary>
                <SoundsMenu select />

                <p />

                <div class="sb">
                    {show_sounds}
                </div>
            </MyErrorBoundary>
        </MySuspense>
    }
}

#[component]
fn SoundsMenu(select: RwSignal<bool>) -> impl IntoView {
    view! {
        <a
            class="btn"
            on:click=move |_| {
                let mut select = select.write();
                *select = !*select;
            }
        >
            {move || {
                if select.get() {
                    "編集取り消し"
                } else {
                    "編集"
                }
            }}
        </a>
        <Show when=move || { select.get() }>
            <span style="margin-left: 10px;">"編集するサウンドを選んでください"</span>
        </Show>
    }
}

#[component]
fn Sets(sets: Resource<Result<SlotMap<SetKey, Set>, ServerFnError>>) -> impl IntoView {
    let show_sets = move || {
        sets.and_then(|sets_el| {
            sets_el
                .iter()
                .map(|set| {
                    view! {
                        <p class="set">
                            {set.1.name.clone()}
                            <a
                                class="btn set-btn"
                                on:click=move |_| {
                                    match window().confirm_with_message("本当に削除しますか?") {
                                        Ok(delete) => {
                                            if delete {
                                                leptos::task::spawn_local(async move {
                                                    match delete_set(set.0).await {
                                                        Ok(()) => {
                                                            sets.refetch();
                                                        }
                                                        Err(e) => {
                                                            logging::error!("{:?}", e);
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            logging::error!("{:?}", e);
                                        }
                                    }
                                }
                            >
                                "削除"
                            </a>
                        </p>
                    }
                })
                .collect_view()
        })
    };

    view! {
        <MySuspense>
            <MyErrorBoundary>
                <SetsMenu sets />

                {show_sets}
            </MyErrorBoundary>
        </MySuspense>
    }
}

#[component]
fn SetsMenu(sets: Resource<Result<SlotMap<SetKey, Set>, ServerFnError>>) -> impl IntoView {
    view! {
        <a
            class="btn"
            on:click=move |_| {
                match window().prompt_with_message("追加するセットの名前を入力してください") {
                    Ok(Some(name)) => {
                        let name = name.trim().to_string();
                        if !name.is_empty() {
                            leptos::task::spawn_local(async move {
                                match add_set(name).await {
                                    Ok(()) => {
                                        sets.refetch();
                                    }
                                    Err(e) => {
                                        logging::error!("{:?}", e)
                                    }
                                }
                            });
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        logging::error!("{:?}", e);
                    }
                }
            }
        >
            "追加"
        </a>
    }
}

#[component]
fn Set(set_reload: RwSignal<Option<Callback<()>>>) -> impl IntoView {
    let params = use_params_map();

    let id_err = {
        view! {
            <p class="error">不適切なIDです</p>
        }
        .into_any()
    };

    let id = params
        .read_untracked()
        .get("id")
        .and_then(|id| id.parse::<u64>().ok())
        .map(|id| SetKey::from(KeyData::from_ffi(id)));

    let id = match id {
        Some(id) => id,
        None => {
            return id_err;
        }
    };

    let set = Resource::new(|| (), move |_| async move { fetch_set(id).await });

    *set_reload.write() = Some(Callback::new(move |()| {
        set.refetch();
    }));

    on_cleanup(move || {
        *set_reload.write() = None;
    });

    let show_set = move || {
        set.and_then(|set| {
            set.as_ref().map(|set| {
                set.iter()
                    .map(|sound| {
                        let name = sound.name.clone();
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
                                { get_name(path.clone(), name) }
                            </a>
                        }
                    })
                    .collect_view()
            })
        })
    };

    view! {
        <MySuspense>
            <MyErrorBoundary>
                <div class="sb">
                    {show_set}
                </div>
            </MyErrorBoundary>
        </MySuspense>
    }
    .into_any()
}
