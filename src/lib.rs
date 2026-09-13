pub mod app;

#[cfg(feature = "backend")]
pub mod bot;

#[cfg(feature = "backend")]
pub mod sounds;

pub mod common_sounds {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};
    use slotmap::SlotMap;

    slotmap::new_key_type! {
        pub struct SetKey;
    }

    #[derive(Default, Serialize, Deserialize)]
    pub struct SoundsFile {
        pub sounds: Vec<Sound>,
        pub sets: SlotMap<SetKey, Set>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Set {
        pub name: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Sound {
        pub path: PathBuf,
        pub name: Option<String>,
        pub set_id: Option<SetKey>,
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;

    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(App);
}

