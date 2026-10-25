use crate::error::AvatarError;
use crate::model::{CharacterId, CharacterProfile, Model3D};
use crate::validator::ModelValidator;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegistryData {
    active_character_id: Option<CharacterId>,
    characters: Vec<CharacterProfile>,
}

#[derive(Debug, Clone)]
pub struct CharacterRegistry {
    storage_dir: PathBuf,
    validator: Arc<ModelValidator>,
    data: Arc<Mutex<RegistryData>>,
}

impl CharacterRegistry {
    pub fn new(storage_dir: PathBuf) -> Result<Self, AvatarError> {
        fs::create_dir_all(&storage_dir)
            .map_err(|e| AvatarError::Io(format!("Failed to create avatar directory: {e}")))?;

        let registry_path = storage_dir.join("registry.json");
        let data = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path)
                .map_err(|e| AvatarError::Io(format!("Failed to read registry.json: {e}")))?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            RegistryData::default()
        };

        Ok(Self {
            storage_dir,
            validator: Arc::new(ModelValidator::default()),
            data: Arc::new(Mutex::new(data)),
        })
    }

    pub fn in_memory() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("wowie_avatar_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        Self {
            storage_dir: temp_dir,
            validator: Arc::new(ModelValidator::default()),
            data: Arc::new(Mutex::new(RegistryData::default())),
        }
    }

    fn persist(&self, data: &RegistryData) -> Result<(), AvatarError> {
        let registry_path = self.storage_dir.join("registry.json");
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| AvatarError::Serialization(e.to_string()))?;
        fs::write(&registry_path, json)
            .map_err(|e| AvatarError::Io(format!("Failed to write registry.json: {e}")))?;
        Ok(())
    }

    pub fn import_character(
        &self,
        source_path: &Path,
        display_name: &str,
    ) -> Result<CharacterProfile, AvatarError> {
        let bytes = fs::read(source_path)
            .map_err(|e| AvatarError::Io(format!("Failed to read model file: {e}")))?;

        let (metadata, expressions) = self.validator.validate_and_extract(&bytes)?;

        let char_id = CharacterId::new();
        let target_filename = format!("{char_id}.vrm");
        let target_path = self.storage_dir.join(&target_filename);

        fs::write(&target_path, &bytes)
            .map_err(|e| AvatarError::Io(format!("Failed to copy model to storage: {e}")))?;

        let profile = CharacterProfile {
            id: char_id,
            display_name: if display_name.trim().is_empty() {
                metadata.title.clone()
            } else {
                display_name.trim().to_string()
            },
            file_name: target_filename,
            file_size_bytes: bytes.len() as u64,
            metadata,
            available_expressions: expressions,
            created_at: Utc::now(),
        };

        let mut data = self.data.lock().unwrap();
        data.characters.push(profile.clone());
        if data.active_character_id.is_none() {
            data.active_character_id = Some(char_id);
        }

        self.persist(&data)?;
        Ok(profile)
    }

    pub fn list_characters(&self) -> Vec<CharacterProfile> {
        self.data.lock().unwrap().characters.clone()
    }

    pub fn get_character(&self, id: CharacterId) -> Option<CharacterProfile> {
        self.data
            .lock()
            .unwrap()
            .characters
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }

    pub fn get_active_character(&self) -> Option<CharacterProfile> {
        let data = self.data.lock().unwrap();
        let active_id = data.active_character_id?;
        data.characters.iter().find(|c| c.id == active_id).cloned()
    }

    pub fn set_active_character(&self, id: CharacterId) -> Result<(), AvatarError> {
        let mut data = self.data.lock().unwrap();
        if !data.characters.iter().any(|c| c.id == id) {
            return Err(AvatarError::NotFound(format!(
                "Character with id {id} not found"
            )));
        }
        data.active_character_id = Some(id);
        self.persist(&data)
    }

    pub fn remove_character(&self, id: CharacterId) -> Result<(), AvatarError> {
        let mut data = self.data.lock().unwrap();
        let pos = data
            .characters
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| AvatarError::NotFound(format!("Character {id} not found")))?;

        let removed = data.characters.remove(pos);
        let file_path = self.storage_dir.join(&removed.file_name);
        let _ = fs::remove_file(file_path);

        if data.active_character_id == Some(id) {
            data.active_character_id = data.characters.first().map(|c| c.id);
        }

        self.persist(&data)
    }

    /// Load the full 3D geometry model for a given character ID.
    pub fn load_character_model(&self, id: CharacterId) -> Result<Model3D, AvatarError> {
        let profile = self
            .get_character(id)
            .ok_or_else(|| AvatarError::NotFound(format!("Character {id} not found")))?;

        let file_path = self.storage_dir.join(&profile.file_name);
        let bytes = fs::read(&file_path)
            .map_err(|e| AvatarError::Io(format!("Failed to read character file: {e}")))?;

        self.validator.load_3d_model(&bytes)
    }

    /// Load the 3D model for the currently active character, or None if no character is active.
    pub fn load_active_character_model(&self) -> Result<Option<Model3D>, AvatarError> {
        let active_profile = self.get_active_character();
        if let Some(profile) = active_profile {
            self.load_character_model(profile.id).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Ensure at least one default authentic 3D companion character is present in the registry.
    pub fn seed_default_character_if_empty(&self) -> Result<CharacterProfile, AvatarError> {
        if let Some(active) = self.get_active_character() {
            return Ok(active);
        }

        let sample_bytes = crate::sample::create_sample_vrm1_bytes();
        let temp_sample_file = self.storage_dir.join("wowiebot_seed.vrm");
        fs::write(&temp_sample_file, &sample_bytes)
            .map_err(|e| AvatarError::Io(format!("Failed to write seed model: {e}")))?;

        let profile = self.import_character(&temp_sample_file, "WowieBot 3D")?;
        let _ = fs::remove_file(temp_sample_file);
        Ok(profile)
    }
}
