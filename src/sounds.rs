use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock};

use crate::common_sounds::Sound;
use crate::common_sounds::SoundsFile;

const SOUNDS_FILE_PATH: &str = "./sounds.json";

pub struct Sounds {
    sounds_folder: PathBuf,
    cached_file: RwLock<Option<Arc<RwLock<SoundsFile>>>>,
    load_locks: Arc<Mutex<()>>,
}

impl Sounds {
    pub fn new(sounds_folder: PathBuf) -> std::io::Result<Self> {
        if sounds_folder.exists() && sounds_folder.is_file() {
            std::fs::remove_file(&sounds_folder)?;
        }
        if !sounds_folder.exists() {
            std::fs::create_dir(&sounds_folder)?;
        }

        Ok(Self {
            sounds_folder,
            cached_file: Default::default(),
            load_locks: Default::default(),
        })
    }

    pub fn sounds_folder(&self) -> &PathBuf {
        &self.sounds_folder
    }

    #[cfg(feature = "backend")]
    async fn load_data(&self) -> std::io::Result<Arc<RwLock<SoundsFile>>> {
        let load_lock = Arc::clone(&self.load_locks);

        let _lock = match load_lock.try_lock() {
            Ok(lock) => lock,
            Err(_) => {
                if let Ok(cached_file) = self.cached_file.try_read()
                    && let Some(cached_file) = &*cached_file
                {
                    let cached_file = Arc::clone(cached_file);

                    return Ok(cached_file);
                }

                let lock = load_lock.lock().await;

                if let Ok(cached_file) = self.cached_file.try_read()
                    && let Some(cached_file) = &*cached_file
                {
                    let cached_file = Arc::clone(cached_file);

                    return Ok(cached_file);
                }

                lock
            }
        };

        let sounds_file = match tokio::fs::read_to_string(SOUNDS_FILE_PATH).await {
            Ok(contents) => {
                let sounds_file: SoundsFile = serde_json::from_str(&contents)?;

                sounds_file
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let data = SoundsFile::default();
                let json = serde_json::to_string_pretty(&data).unwrap();

                tokio::fs::write(SOUNDS_FILE_PATH, json).await?;

                data
            }
            Err(e) => {
                return Err(e);
            }
        };

        let sounds_file = Arc::new(RwLock::new(sounds_file));

        *self.cached_file.write().await = Some(Arc::clone(&sounds_file));

        Ok(sounds_file)
    }

    #[cfg(feature = "backend")]
    pub async fn data<R>(&self, reader: impl FnOnce(&SoundsFile) -> R) -> std::io::Result<R> {
        let data = self.load_data().await?;
        let data = data.read().await;

        Ok(reader(&data))
    }

    #[cfg(feature = "backend")]
    pub async fn data_mut<R>(
        &self,
        writer: impl FnOnce(&mut SoundsFile) -> R,
    ) -> std::io::Result<R> {
        let data = self.load_data().await?;
        let mut data = data.write().await;

        let res = writer(&mut data);

        let json = serde_json::to_string_pretty(&*data).unwrap();

        tokio::fs::write(SOUNDS_FILE_PATH, json).await?;

        Ok(res)
    }

    #[cfg(feature = "backend")]
    pub async fn fetch_new_sounds(&self) -> std::io::Result<()> {
        let sounds = self.data(|file| file.sounds.clone()).await?;

        let mut index = Vec::new();
        for sound in sounds {
            index.push(sound.1.path);
        }

        let mut dirs = tokio::fs::read_dir(self.sounds_folder.clone()).await?;

        let mut add_things = Vec::new();
        while let Ok(Some(entry)) = dirs.next_entry().await {
            if !index.contains(&entry.path()) {
                add_things.push(Sound {
                    path: entry.path(),
                    name: Default::default(),
                });
            }
        }

        self.data_mut(|data| {
            for add_thing in add_things {
                data.sounds.insert(add_thing);
            }
        })
        .await?;

        Ok(())
    }
}
