use std::collections::HashMap;
use std::path::PathBuf;

use aivt_database::{
    AttachmentsRepository, AvatarState, ConversationsRepository, DbPool, MessagesRepository, NewAttachment,
    NewProfile, PeopleRepository, Profile, ProfileUpdate, ProfileVoice, ProfileVoiceUpdate, ProfilesRepository,
    MAX_ALIASES_PER_PERSON,
};

use crate::export::{self, ExportedAttachment, ExportedConversation, ExportedFiles, ExportedMessage, ProfileExport};
use crate::ProfileError;

/// Application-level view over profile CRUD, avatar storage, and
/// import/export — everything Fase 1's Identidad tab needs.
///
/// `data_dir` is the app's OS-resolved data directory (same root as the
/// SQLite file); avatars live at `<data_dir>/avatars/<profile_id>/`, never
/// as base64 inside SQLite (sección 75 del brief).
pub struct ProfileService {
    pool: DbPool,
    data_dir: PathBuf,
}

impl ProfileService {
    pub fn new(pool: DbPool, data_dir: PathBuf) -> Self {
        Self { pool, data_dir }
    }

    fn repo(&self) -> ProfilesRepository<'_> {
        ProfilesRepository::new(&self.pool)
    }

    fn people_repo(&self) -> PeopleRepository<'_> {
        PeopleRepository::new(&self.pool)
    }

    fn avatars_dir(&self) -> PathBuf {
        self.data_dir.join("avatars")
    }

    /// Mismo árbol que `ChatService` ya usa para adjuntos de chat
    /// (`<data_dir>/attachments/<hash[..2]>/<hash>.<ext>`) — se reconstruye
    /// acá en vez de depender de `aivt-service-chat` (evitaría un ciclo:
    /// `chat` ya depende de `profiles` para resolver el perfil activo).
    fn attachments_dir(&self) -> PathBuf {
        self.data_dir.join("attachments")
    }

    /// Escribe un adjunto importado si todavía no existe en disco — mismo
    /// criterio content-addressed que `ChatService::save_attachment`: si el
    /// hash ya está (otro perfil, u otra importación anterior, ya lo
    /// trajo), no se reescribe.
    async fn write_attachment_file_if_missing(&self, hash: &str, extension: &str, bytes: &[u8]) -> Result<String, ProfileError> {
        // Defensa en profundidad: `read_zip` ya lo valida, pero esto arma una ruta a
        // partir de datos de un archivo ajeno y no debe depender de que el llamador
        // se acordara de comprobarlo.
        if !crate::export::is_safe_attachment_name(hash, extension) {
            return Err(crate::export::ExportFormatError::UnsafeAttachmentName.into());
        }
        let prefix = &hash[..hash.len().min(2)];
        let dir = self.attachments_dir().join(prefix);
        tokio::fs::create_dir_all(&dir).await?;

        let file_name = format!("{hash}.{extension}");
        let file_path = dir.join(&file_name);
        if tokio::fs::metadata(&file_path).await.is_err() {
            tokio::fs::write(&file_path, bytes).await?;
        }
        Ok(format!("attachments/{prefix}/{file_name}"))
    }

    /// Registers/updates a character's name/nickname in the registro de
    /// personas (`role = "ai"`, keyed on `external_key = "profile:<id>"`
    /// so a rename updates the *same* row rather than leaving a stale one
    /// behind — see `PeopleRepository::upsert_by_external_key`) so Whisper
    /// recognizes it by voice even when this profile *isn't* the one
    /// currently active during a call — `CallService::transcription_prompt`
    /// already primes the *active* profile's own name unconditionally, but
    /// that leaves every other AI character the user has created
    /// unrecognized unless they're mentioned by name and happen to pass
    /// conversation extraction (which deliberately excludes AI characters
    /// — they aren't "real people" to extract). Called from every path
    /// that results in a usable character (`create`, `update`, `duplicate`,
    /// `import`) — pedido del usuario: que esto sea automático, en vez de
    /// tener que agregarlo a mano en Configuración > Privacidad cada vez.
    /// `update` matters just as much as `create`: a freshly-created
    /// profile starts out named "Nuevo personaje" (see
    /// `ProfilesRepository::create`) and only gets its real name once the
    /// user fills in the form and saves — a live-testing checkpoint hit
    /// exactly this, the placeholder name was the only one ever
    /// registered. Best-effort: a failure here shouldn't fail the profile
    /// operation over what's ultimately a secondary benefit.
    async fn register_as_ai_person(&self, profile: &Profile) {
        let nickname = profile.nickname.as_deref().filter(|n| !n.trim().is_empty());
        let external_key = format!("profile:{}", profile.id);
        if let Err(err) =
            self.people_repo().upsert_by_external_key(&external_key, &profile.name, nickname, "ai", "profile", MAX_ALIASES_PER_PERSON).await
        {
            tracing::warn!(target: "system", "profiles: no se pudo registrar '{}' en Personas: {err}", profile.name);
        }
    }

    pub async fn create(&self, input: NewProfile) -> Result<Profile, ProfileError> {
        let profile = self.repo().create(input).await?;
        self.register_as_ai_person(&profile).await;
        Ok(profile)
    }

    pub async fn get(&self, id: &str) -> Result<Option<Profile>, ProfileError> {
        Ok(self.repo().get(id).await?)
    }

    pub async fn list(&self) -> Result<Vec<Profile>, ProfileError> {
        Ok(self.repo().list().await?)
    }

    pub async fn update(&self, id: &str, input: ProfileUpdate) -> Result<Profile, ProfileError> {
        let profile = self.repo().update(id, input).await?;
        self.register_as_ai_person(&profile).await;
        Ok(profile)
    }

    pub async fn duplicate(&self, id: &str, new_name: &str) -> Result<Profile, ProfileError> {
        let profile = self.repo().duplicate(id, new_name).await?;
        self.register_as_ai_person(&profile).await;
        Ok(profile)
    }

    /// Fase 5 — edited from its own Voz tab, not bundled into `update`.
    pub async fn set_voice(&self, id: &str, input: ProfileVoiceUpdate) -> Result<ProfileVoice, ProfileError> {
        Ok(self.repo().set_voice(id, input).await?)
    }

    pub async fn delete(&self, id: &str) -> Result<(), ProfileError> {
        let existing = self.repo().get(id).await?;
        self.repo().delete(id).await?;

        // Best-effort cleanup: a stray avatar directory or registro-de-
        // personas entry left behind is not a correctness problem worth
        // failing the delete over.
        if existing.is_some() {
            let profile_avatar_dir = self.avatars_dir().join(id);
            let _ = tokio::fs::remove_dir_all(profile_avatar_dir).await;
            if let Err(err) = self.people_repo().delete_by_external_key(&format!("profile:{id}")).await {
                tracing::warn!(target: "system", "profiles: no se pudo limpiar el registro de Personas: {err}");
            }
        }

        Ok(())
    }

    /// Shared replace-on-write logic behind `set_avatar`/`set_avatar_state`/
    /// `set_user_avatar`: create `<data_dir>/avatars/<subdir>/` if needed,
    /// delete whatever previously lived there (or just files starting with
    /// `stem_filter.` when the directory is shared with other stems, as
    /// `set_avatar_state`'s three reactive states are), write the new file,
    /// and return its path relative to `data_dir`.
    async fn write_avatar_file(
        &self,
        subdir: &str,
        stem_filter: Option<&str>,
        file_stem: &str,
        bytes: &[u8],
        extension: &str,
    ) -> Result<String, ProfileError> {
        let dir = self.avatars_dir().join(subdir);
        tokio::fs::create_dir_all(&dir).await?;

        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let should_remove = match stem_filter {
                Some(stem) => entry.file_name().to_string_lossy().starts_with(&format!("{stem}.")),
                None => true,
            };
            if should_remove {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }

        let file_name = format!("{file_stem}.{}", sanitize_extension(extension));
        tokio::fs::write(dir.join(&file_name), bytes).await?;

        Ok(format!("avatars/{subdir}/{file_name}"))
    }

    /// Saves `bytes` as the profile's avatar, replacing any previous file,
    /// and records the path (relative to `data_dir`) on the profile row.
    /// Returns that relative path.
    pub async fn set_avatar(&self, id: &str, bytes: &[u8], extension: &str) -> Result<String, ProfileError> {
        let relative_path = self.write_avatar_file(id, None, "avatar", bytes, extension).await?;
        self.repo().set_avatar_path(id, Some(&relative_path)).await?;
        Ok(relative_path)
    }

    /// Fase 7 — same replace-on-write pattern as `set_avatar`, but for one
    /// of the three reactive-avatar states instead of the single profile
    /// picture. Files live at `<data_dir>/avatars/<id>/<state>.<ext>`.
    pub async fn set_avatar_state(&self, id: &str, state: AvatarState, bytes: &[u8], extension: &str) -> Result<String, ProfileError> {
        let state_stem = match state {
            AvatarState::Idle => "idle",
            AvatarState::Thinking => "thinking",
            AvatarState::Talking => "talking",
        };

        let relative_path = self.write_avatar_file(id, Some(state_stem), state_stem, bytes, extension).await?;
        self.repo().set_avatar_state_path(id, state, Some(&relative_path)).await?;
        Ok(relative_path)
    }

    /// Fase 11 Polish — foto global del usuario (no de un personaje), para
    /// mostrarla junto a sus propios mensajes en el chat. Vive en
    /// `avatars/user/` en vez de `avatars/<profile_id>/` porque no hay
    /// ninguna fila de `profiles` a la que asociarla — es la única razón de
    /// que este método esté en `ProfileService` (dueño de la lógica de
    /// archivos de avatar) en vez de tocar la tabla `profiles`. El path se
    /// guarda aparte, en `SettingsService` (un setting global, no un campo
    /// de perfil).
    pub async fn set_user_avatar(&self, bytes: &[u8], extension: &str) -> Result<String, ProfileError> {
        self.write_avatar_file("user", None, "avatar", bytes, extension).await
    }

    /// Reads a stored avatar back into memory — the frontend renders it as
    /// a `data:` URI rather than through Tauri's asset protocol, to avoid
    /// the extra asset-scope configuration for what's a handful of small
    /// images (see docs/architecture.md).
    pub async fn read_avatar(&self, relative_path: &str) -> Result<Vec<u8>, ProfileError> {
        Ok(tokio::fs::read(self.data_dir.join(relative_path)).await?)
    }

    /// Downscales the profile's avatar into a small thumbnail for the
    /// Windows toast notification's circular icon override, written under
    /// the OS temp dir rather than next to the avatar itself — confirmed
    /// live that WinRT's `appLogoOverride` silently renders no icon at all
    /// for *any* file under `<data_dir>/avatars/...` (tried multiple sizes
    /// and both a Rust- and a GDI+-encoded PNG, all identically blank),
    /// while the exact same bytes copied to the temp dir render fine —
    /// almost certainly an ACL/AppContainer restriction on the app's own
    /// data folder that the notification host process can't read through.
    /// Regenerated on every call (cheap at this size) so it always
    /// reflects the current avatar; `None` when the profile has no avatar
    /// set.
    pub async fn notification_icon_path(&self, id: &str) -> Result<Option<PathBuf>, ProfileError> {
        let Some(profile) = self.repo().get(id).await? else { return Ok(None) };
        let Some(avatar_path) = profile.avatar_path else { return Ok(None) };

        let source = self.data_dir.join(avatar_path);
        let dest = std::env::temp_dir().join(format!("aivt_notification_icon_{id}.png"));
        let dest_for_task = dest.clone();

        tokio::task::spawn_blocking(move || -> Result<(), ProfileError> {
            let image = image::open(source)?;
            image.thumbnail(128, 128).save(dest_for_task)?;
            Ok(())
        })
        .await
        .expect("notification icon resize task panicked")?;

        Ok(Some(dest))
    }

    /// Arma los bytes completos del `.aivtprofile` (ZIP comprimido) — texto
    /// del perfil siempre, más avatar/avatar reactivo si el perfil tiene, más
    /// el historial de chats completo (conversaciones + mensajes + adjuntos
    /// únicos) si `include_chats` — pedido explícito del usuario tras perder
    /// todo eso en un export/import de prueba: antes de este cambio
    /// "Exportar" solo guardaba texto.
    pub async fn export(&self, id: &str, include_chats: bool) -> Result<Vec<u8>, ProfileError> {
        let profile = self.repo().get(id).await?.ok_or(ProfileError::NotFound)?;
        let mut profile_export = ProfileExport::from_profile(&profile);
        let mut files = ExportedFiles::default();

        if let Some(path) = &profile.avatar_path {
            files.avatar = Some(tokio::fs::read(self.data_dir.join(path)).await?);
            profile_export.avatar_extension = Some(extension_of(path));
        }
        if let Some(path) = &profile.avatar_idle_path {
            files.avatar_idle = Some(tokio::fs::read(self.data_dir.join(path)).await?);
            profile_export.avatar_idle_extension = Some(extension_of(path));
        }
        if let Some(path) = &profile.avatar_thinking_path {
            files.avatar_thinking = Some(tokio::fs::read(self.data_dir.join(path)).await?);
            profile_export.avatar_thinking_extension = Some(extension_of(path));
        }
        if let Some(path) = &profile.avatar_talking_path {
            files.avatar_talking = Some(tokio::fs::read(self.data_dir.join(path)).await?);
            profile_export.avatar_talking_extension = Some(extension_of(path));
        }

        if include_chats {
            let (chats, attachments) = self.collect_chats_for_export(id).await?;
            files.chats = Some(chats);
            files.attachments = attachments;
        }

        Ok(export::write_zip(&profile_export, &files)?)
    }

    /// Junta todas las conversaciones de un perfil en `ExportedConversation`
    /// + los bytes de cada adjunto único (deduplicado por `hash`+extensión,
    /// para no repetir el mismo archivo si dos mensajes comparten una
    /// imagen). Separado de `export` solo por legibilidad — es la parte más
    /// larga del método.
    async fn collect_chats_for_export(&self, profile_id: &str) -> Result<(Vec<ExportedConversation>, Vec<(String, String, Vec<u8>)>), ProfileError> {
        let conversations = ConversationsRepository::new(&self.pool).list_for_profile(profile_id).await?;
        let mut exported_conversations = Vec::with_capacity(conversations.len());
        let mut seen_attachments = std::collections::HashSet::new();
        let mut attachment_files = Vec::new();

        for conversation in conversations {
            let messages = MessagesRepository::new(&self.pool).list_for_conversation(&conversation.id).await?;
            let attachments = AttachmentsRepository::new(&self.pool).list_for_conversation(&conversation.id).await?;
            let mut attachments_by_message: HashMap<String, Vec<_>> = HashMap::new();
            for attachment in attachments {
                attachments_by_message.entry(attachment.message_id.clone()).or_default().push(attachment);
            }

            let mut exported_messages = Vec::with_capacity(messages.len());
            for message in messages {
                let message_attachments = attachments_by_message.remove(&message.id).unwrap_or_default();
                let mut exported_attachments = Vec::with_capacity(message_attachments.len());
                for attachment in message_attachments {
                    let ext = extension_of(&attachment.path);
                    if seen_attachments.insert((attachment.hash.clone(), ext.clone())) {
                        let bytes = tokio::fs::read(self.data_dir.join(&attachment.path)).await?;
                        attachment_files.push((attachment.hash.clone(), ext.clone(), bytes));
                    }
                    exported_attachments.push(ExportedAttachment {
                        hash: attachment.hash,
                        extension: ext,
                        mime_type: attachment.mime_type,
                        size_bytes: attachment.size_bytes,
                    });
                }
                exported_messages.push(ExportedMessage {
                    role: message.role,
                    content: message.content,
                    status: message.status,
                    provider_id: message.provider_id,
                    model: message.model,
                    created_at: message.created_at,
                    attachments: exported_attachments,
                });
            }

            exported_conversations.push(ExportedConversation {
                title: conversation.title,
                source: conversation.source,
                model: conversation.model,
                created_at: conversation.created_at,
                updated_at: conversation.updated_at,
                messages: exported_messages,
            });
        }

        Ok((exported_conversations, attachment_files))
    }

    /// Crea un perfil nuevo (siempre con id fresco, nunca reutiliza el
    /// original) a partir de un `.aivtprofile` — texto, avatar/avatar
    /// reactivo y chats, todo lo que `export` haya incluido. Si algo falla
    /// a mitad de camino (después de que el perfil ya existe), lo borra en
    /// vez de dejar un personaje a medio importar dando vueltas.
    ///
    /// Compatibilidad hacia atrás: un `.aivtprofile` exportado antes de
    /// este cambio es JSON plano, no un ZIP — si `read_zip` no lo reconoce
    /// como ZIP, se lo intenta leer directo como el `ProfileExport` de
    /// siempre (sin avatar/chats, que ese formato nunca tuvo).
    pub async fn import(&self, file_bytes: &[u8]) -> Result<Profile, ProfileError> {
        let imported = match export::read_zip(file_bytes) {
            Ok(imported) => imported,
            Err(_) => {
                let profile: ProfileExport = serde_json::from_slice(file_bytes)?;
                export::ImportedZip {
                    profile,
                    avatar: None,
                    avatar_idle: None,
                    avatar_thinking: None,
                    avatar_talking: None,
                    chats: None,
                    attachments: HashMap::new(),
                }
            }
        };

        let created = self
            .repo()
            .create(NewProfile {
                name: imported.profile.name.clone(),
                nickname: imported.profile.nickname.clone(),
                description: imported.profile.description.clone(),
            })
            .await?;

        if let Err(err) = self.import_rest(&created.id, imported).await {
            let _ = self.delete(&created.id).await;
            return Err(err);
        }

        let final_profile = self.repo().get(&created.id).await?.ok_or(ProfileError::NotFound)?;
        self.register_as_ai_person(&final_profile).await;
        Ok(final_profile)
    }

    /// Todo lo que `import` hace después de crear el perfil — separado para
    /// que `import` pueda envolverlo en un solo `if let Err(...)` de
    /// limpieza best-effort (ver su doc comment).
    async fn import_rest(&self, new_id: &str, imported: export::ImportedZip) -> Result<(), ProfileError> {
        let export::ImportedZip { profile: export, avatar, avatar_idle, avatar_thinking, avatar_talking, chats, attachments } = imported;

        let update = ProfileUpdate {
            name: export.name,
            nickname: export.nickname,
            description: export.description,
            age_fictional: export.age_fictional,
            pronouns: export.pronouns,
            occupation: export.occupation,
            species: export.species,
            accent_color: export.accent_color,
            theme: export.theme,
            identity: export.identity,
            personality: export.personality,
        };
        self.repo().update(new_id, update).await?;

        self.repo()
            .set_voice(
                new_id,
                ProfileVoiceUpdate {
                    enabled: export.voice.enabled,
                    voice_id: export.voice.voice_id,
                    rate: export.voice.rate,
                    pitch: export.voice.pitch,
                    volume: export.voice.volume,
                },
            )
            .await?;

        if let (Some(bytes), Some(ext)) = (&avatar, &export.avatar_extension) {
            self.set_avatar(new_id, bytes, ext).await?;
        }
        if let (Some(bytes), Some(ext)) = (&avatar_idle, &export.avatar_idle_extension) {
            self.set_avatar_state(new_id, AvatarState::Idle, bytes, ext).await?;
        }
        if let (Some(bytes), Some(ext)) = (&avatar_thinking, &export.avatar_thinking_extension) {
            self.set_avatar_state(new_id, AvatarState::Thinking, bytes, ext).await?;
        }
        if let (Some(bytes), Some(ext)) = (&avatar_talking, &export.avatar_talking_extension) {
            self.set_avatar_state(new_id, AvatarState::Talking, bytes, ext).await?;
        }

        if let Some(chats) = chats {
            for conversation in chats {
                let created_conversation = ConversationsRepository::new(&self.pool)
                    .create_imported(
                        new_id,
                        &conversation.title,
                        &conversation.source,
                        conversation.model.as_deref(),
                        &conversation.created_at,
                        &conversation.updated_at,
                    )
                    .await?;

                for message in conversation.messages {
                    let created_message = MessagesRepository::new(&self.pool)
                        .create_imported(
                            &created_conversation.id,
                            message.role,
                            &message.content,
                            message.status,
                            message.provider_id.as_deref(),
                            message.model.as_deref(),
                            &message.created_at,
                        )
                        .await?;

                    for attachment in message.attachments {
                        let key = format!("{}.{}", attachment.hash, attachment.extension);
                        let Some(bytes) = attachments.get(&key) else { continue };
                        let relative_path = self.write_attachment_file_if_missing(&attachment.hash, &attachment.extension, bytes).await?;
                        AttachmentsRepository::new(&self.pool)
                            .create(NewAttachment {
                                message_id: created_message.id.clone(),
                                kind: "image".to_string(),
                                path: relative_path,
                                mime_type: attachment.mime_type,
                                size_bytes: attachment.size_bytes,
                                hash: attachment.hash,
                            })
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// La extensión (sin punto) de un path relativo guardado en DB, ej.
/// `"avatars/<id>/avatar.png"` -> `"png"` — todos los archivos que este
/// servicio escribe ya pasan por `sanitize_extension` al guardarse, así
/// que siempre hay algo razonable acá; el fallback solo cubre un path
/// vacío o sin punto, que no debería darse en la práctica.
fn extension_of(path: &str) -> String {
    std::path::Path::new(path).extension().and_then(|ext| ext.to_str()).unwrap_or("png").to_lowercase()
}

fn sanitize_extension(extension: &str) -> String {
    let cleaned: String = extension.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if cleaned.is_empty() {
        "png".to_string()
    } else {
        cleaned.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aivt_database::ProfileIdentity;

    async fn test_service() -> (ProfileService, tempfile::TempDir) {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let service = ProfileService::new(pool, dir.path().to_path_buf());
        (service, dir)
    }

    fn sample_new_profile(name: &str) -> NewProfile {
        NewProfile { name: name.to_string(), nickname: None, description: None }
    }

    #[tokio::test]
    async fn create_get_list_delete_round_trip() {
        let (service, _dir) = test_service().await;

        let created = service.create(sample_new_profile("Ciel")).await.unwrap();
        assert_eq!(service.list().await.unwrap().len(), 1);

        let fetched = service.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Ciel");

        service.delete(&created.id).await.unwrap();
        assert!(service.get(&created.id).await.unwrap().is_none());
        assert_eq!(service.list().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn create_also_registers_the_character_as_an_ai_person() {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let service = ProfileService::new(pool.clone(), dir.path().to_path_buf());

        service
            .create(NewProfile { name: "Ciel".to_string(), nickname: Some("Luciel".to_string()), description: None })
            .await
            .unwrap();

        let people = PeopleRepository::new(&pool).list_all().await.unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].canonical_name, "Ciel");
        assert_eq!(people[0].role, "ai");
        assert_eq!(people[0].source, "profile");
        assert_eq!(people[0].aliases, vec!["Luciel".to_string()]);
    }

    #[tokio::test]
    async fn renaming_a_profile_updates_its_person_entry_instead_of_leaving_the_placeholder() {
        // Regression: profiles start out named "Nuevo personaje" (see
        // `ProfilesRepository::create`) — `update` must correct the
        // *same* registro-de-personas row once the user saves the real
        // name, not leave the placeholder stuck there forever alongside a
        // second row (pedido del usuario, encontrado en vivo).
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let service = ProfileService::new(pool.clone(), dir.path().to_path_buf());

        let created = service.create(sample_new_profile("Nuevo personaje")).await.unwrap();
        service
            .update(
                &created.id,
                ProfileUpdate {
                    name: "Ciel".to_string(),
                    nickname: Some("Luciel".to_string()),
                    description: None,
                    age_fictional: None,
                    pronouns: None,
                    occupation: None,
                    species: None,
                    accent_color: None,
                    theme: None,
                    identity: ProfileIdentity::default(),
                    personality: Default::default(),
                },
            )
            .await
            .unwrap();

        let people = PeopleRepository::new(&pool).list_all().await.unwrap();
        assert_eq!(people.len(), 1, "the placeholder name must not linger as a separate row");
        assert_eq!(people[0].canonical_name, "Ciel");
        assert_eq!(people[0].aliases, vec!["Luciel".to_string()]);
    }

    #[tokio::test]
    async fn deleting_a_profile_removes_its_person_entry() {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let service = ProfileService::new(pool.clone(), dir.path().to_path_buf());
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        service.delete(&created.id).await.unwrap();

        assert!(PeopleRepository::new(&pool).list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn set_avatar_writes_file_and_updates_profile() {
        let (service, dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        let bytes = vec![137, 80, 78, 71]; // fake PNG header, contents don't matter here
        let relative_path = service.set_avatar(&created.id, &bytes, "PNG").await.unwrap();
        assert_eq!(relative_path, format!("avatars/{}/avatar.png", created.id));
        assert!(dir.path().join(&relative_path).exists());

        let fetched = service.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.avatar_path.as_deref(), Some(relative_path.as_str()));

        let read_back = service.read_avatar(&relative_path).await.unwrap();
        assert_eq!(read_back, bytes);
    }

    #[tokio::test]
    async fn set_avatar_replaces_previous_file() {
        let (service, dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        service.set_avatar(&created.id, &[1, 2, 3], "png").await.unwrap();
        service.set_avatar(&created.id, &[4, 5, 6], "jpg").await.unwrap();

        let profile_dir = dir.path().join("avatars").join(&created.id);
        let files: Vec<_> = std::fs::read_dir(&profile_dir).unwrap().collect();
        assert_eq!(files.len(), 1, "old avatar file should have been removed");
    }

    #[tokio::test]
    async fn set_avatar_state_writes_file_and_updates_profile() {
        let (service, dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        let bytes = vec![137, 80, 78, 71];
        let relative_path = service.set_avatar_state(&created.id, AvatarState::Talking, &bytes, "PNG").await.unwrap();
        assert_eq!(relative_path, format!("avatars/{}/talking.png", created.id));
        assert!(dir.path().join(&relative_path).exists());

        let fetched = service.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.avatar_talking_path.as_deref(), Some(relative_path.as_str()));
        // Other states untouched.
        assert!(fetched.avatar_idle_path.is_none());
    }

    #[tokio::test]
    async fn set_avatar_state_replaces_only_that_states_previous_file() {
        let (service, dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        service.set_avatar_state(&created.id, AvatarState::Idle, &[1, 2, 3], "png").await.unwrap();
        service.set_avatar_state(&created.id, AvatarState::Idle, &[4, 5, 6], "jpg").await.unwrap();
        service.set_avatar_state(&created.id, AvatarState::Talking, &[7, 8, 9], "png").await.unwrap();

        let profile_dir = dir.path().join("avatars").join(&created.id);
        let mut files: Vec<_> = std::fs::read_dir(&profile_dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        files.sort();
        // Old idle.png replaced by idle.jpg, talking.png untouched alongside it.
        assert_eq!(files, vec!["idle.jpg".to_string(), "talking.png".to_string()]);
    }

    #[tokio::test]
    async fn export_then_import_creates_an_independent_copy() {
        let (service, _dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();
        service
            .update(
                &created.id,
                ProfileUpdate {
                    name: "Ciel".to_string(),
                    nickname: None,
                    description: None,
                    age_fictional: None,
                    pronouns: None,
                    occupation: None,
                    species: None,
                    accent_color: None,
                    theme: Some("ocean".to_string()),
                    identity: ProfileIdentity { lore: "Lore original".to_string(), ..ProfileIdentity::default() },
                    personality: Default::default(),
                },
            )
            .await
            .unwrap();

        let zip_bytes = service.export(&created.id, false).await.unwrap();

        let imported = service.import(&zip_bytes).await.unwrap();
        assert_ne!(imported.id, created.id);
        assert_eq!(imported.name, "Ciel");
        assert_eq!(imported.identity.lore, "Lore original");
        assert_eq!(imported.theme.as_deref(), Some("ocean"));
    }

    #[tokio::test]
    async fn export_missing_profile_errors() {
        let (service, _dir) = test_service().await;
        assert!(matches!(service.export("missing", false).await, Err(ProfileError::NotFound)));
    }

    #[tokio::test]
    async fn export_then_import_restores_avatar_and_reactive_states() {
        let (service, _dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();
        service.set_avatar(&created.id, &[1, 2, 3], "png").await.unwrap();
        service.set_avatar_state(&created.id, AvatarState::Idle, &[4, 5, 6], "jpg").await.unwrap();

        let zip_bytes = service.export(&created.id, false).await.unwrap();
        let imported = service.import(&zip_bytes).await.unwrap();

        assert!(imported.avatar_path.is_some());
        assert_eq!(service.read_avatar(imported.avatar_path.as_deref().unwrap()).await.unwrap(), vec![1, 2, 3]);
        assert!(imported.avatar_idle_path.is_some());
        assert_eq!(service.read_avatar(imported.avatar_idle_path.as_deref().unwrap()).await.unwrap(), vec![4, 5, 6]);
    }

    #[tokio::test]
    async fn export_with_include_chats_restores_conversations_and_messages() {
        let (service, _dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();
        let conversation = ConversationsRepository::new(&service.pool).create(&created.id, "Charla", "chat").await.unwrap();
        MessagesRepository::new(&service.pool)
            .create(aivt_database::NewMessage {
                conversation_id: conversation.id.clone(),
                role: aivt_database::MessageRole::User,
                content: "Hola!".to_string(),
                status: aivt_database::MessageStatus::Complete,
                provider_id: None,
                model: None,
            })
            .await
            .unwrap();

        let zip_bytes = service.export(&created.id, true).await.unwrap();
        let imported = service.import(&zip_bytes).await.unwrap();

        let conversations = ConversationsRepository::new(&service.pool).list_for_profile(&imported.id).await.unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].title, "Charla");
        assert_ne!(conversations[0].id, conversation.id, "debe tener un id nuevo, no reusar el original");

        let messages = MessagesRepository::new(&service.pool).list_for_conversation(&conversations[0].id).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hola!");
    }

    #[tokio::test]
    async fn export_without_include_chats_omits_conversations() {
        let (service, _dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();
        ConversationsRepository::new(&service.pool).create(&created.id, "Charla", "chat").await.unwrap();

        let zip_bytes = service.export(&created.id, false).await.unwrap();
        let imported = service.import(&zip_bytes).await.unwrap();

        assert!(ConversationsRepository::new(&service.pool).list_for_profile(&imported.id).await.unwrap().is_empty());
    }

    /// Un `.aivtprofile` exportado antes de este cambio es JSON plano, no
    /// un ZIP — debe seguir importando (sin avatar/chats, que ese formato
    /// nunca tuvo) en vez de fallar.
    #[tokio::test]
    async fn importing_a_pre_v2_plain_json_export_still_works() {
        let (service, _dir) = test_service().await;
        let json = r#"{
            "format": "aivtprofile",
            "version": 1,
            "name": "Ciel",
            "nickname": null,
            "description": null,
            "ageFictional": null,
            "pronouns": null,
            "occupation": null,
            "species": null,
            "accentColor": null,
            "identity": {"lore":"Lore vieja","relationshipToUser":"","userRole":"","speakingStyle":"","specialInstructions":""},
            "personality": {"freeDescription":"","warmth":50,"empathy":50,"humor":50,"sarcasm":20,"formality":30,"curiosity":50,"energy":50,"spontaneity":50,"initiative":40,"affection":50,"shyness":30,"confidence":50,"expressiveness":50,"verbosity":50,"flirtiness":0,"emotionalIntensity":50,"teasingTendency":30,"questionTendency":40,"initiateConversationTendency":30}
        }"#;

        let imported = service.import(json.as_bytes()).await.unwrap();
        assert_eq!(imported.name, "Ciel");
        assert_eq!(imported.identity.lore, "Lore vieja");
        assert!(imported.avatar_path.is_none());
    }

    #[tokio::test]
    async fn set_user_avatar_writes_file_without_touching_any_profile() {
        let (service, dir) = test_service().await;
        let created = service.create(sample_new_profile("Ciel")).await.unwrap();

        let bytes = vec![137, 80, 78, 71];
        let relative_path = service.set_user_avatar(&bytes, "PNG").await.unwrap();
        assert_eq!(relative_path, "avatars/user/avatar.png");
        assert!(dir.path().join(&relative_path).exists());

        let read_back = service.read_avatar(&relative_path).await.unwrap();
        assert_eq!(read_back, bytes);

        // The unrelated profile's own avatar is untouched.
        let fetched = service.get(&created.id).await.unwrap().unwrap();
        assert!(fetched.avatar_path.is_none());
    }

    #[tokio::test]
    async fn set_user_avatar_replaces_previous_file() {
        let (service, dir) = test_service().await;

        service.set_user_avatar(&[1, 2, 3], "png").await.unwrap();
        service.set_user_avatar(&[4, 5, 6], "jpg").await.unwrap();

        let user_avatar_dir = dir.path().join("avatars").join("user");
        let files: Vec<_> = std::fs::read_dir(&user_avatar_dir).unwrap().collect();
        assert_eq!(files.len(), 1, "old user avatar file should have been removed");
    }
    #[tokio::test]
    async fn writing_an_attachment_with_a_traversal_name_is_refused_and_creates_nothing() {
        let (service, dir) = test_service().await;
        let outside = dir.path().parent().unwrap().join("evil.bat");

        let result = service.write_attachment_file_if_missing("../../evil", "bat", b"MZ").await;

        assert!(result.is_err());
        assert!(!outside.exists());
        assert!(!dir.path().join("attachments").exists(), "no debería ni crear carpetas");
    }

    #[tokio::test]
    async fn a_legitimate_attachment_is_still_written_under_its_hash_prefix() {
        let (service, dir) = test_service().await;
        let hash = "ab".to_string() + &"c".repeat(62);

        let path = service.write_attachment_file_if_missing(&hash, "png", b"data").await.unwrap();

        assert_eq!(path, format!("attachments/ab/{hash}.png"));
        assert!(dir.path().join("attachments").join("ab").join(format!("{hash}.png")).exists());
    }
}
