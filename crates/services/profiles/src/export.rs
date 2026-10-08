use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use aivt_database::{MessageRole, MessageStatus, Profile, ProfileIdentity, ProfilePersonality, ProfileVoice};

const FORMAT: &str = "aivtprofile";
const CURRENT_VERSION: u32 = 2;

const PROFILE_ENTRY: &str = "profile.json";
const CHATS_ENTRY: &str = "chats.json";
const ATTACHMENTS_DIR: &str = "attachments/";

/// The `.aivtprofile` file format (sección 9 y 109 del brief). Desde la
/// v2 el archivo es un ZIP, no JSON plano — `write_zip`/`read_zip` abajo
/// son la única forma de armarlo/leerlo; nada fuera de este módulo debería
/// tocar la estructura del ZIP directamente.
///
/// Deliberately excludes: API keys/tokens (never included, full stop) y
/// memorias (Fase 4 — quedará opt-in, no por defecto). Los chats SÍ se
/// incluyen (a diferencia de las memorias) porque son datos del usuario,
/// no algo que la IA "aprendió" — pero son opcionales vía `include_chats`
/// en el momento de exportar (pedido del usuario: por defecto tildado,
/// pero se puede destildar antes de compartir un personaje).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExport {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub nickname: Option<String>,
    pub description: Option<String>,
    pub age_fictional: Option<String>,
    pub pronouns: Option<String>,
    pub occupation: Option<String>,
    pub species: Option<String>,
    pub accent_color: Option<String>,
    /// Added alongside the 15-preset ThemeEngine (ADR-009).
    /// `#[serde(default)]` so a `.aivtprofile` exported before this field
    /// existed still imports — it just falls back to "Personalizado".
    #[serde(default)]
    pub theme: Option<String>,
    pub identity: ProfileIdentity,
    pub personality: ProfilePersonality,
    /// Added in Fase 5. `#[serde(default)]` so a `.aivtprofile` exported
    /// before this field existed still imports — it just gets voice
    /// disabled with the system default, same as a brand new profile.
    #[serde(default)]
    pub voice: ProfileVoice,
    /// Extensión real de cada imagen incluida en el ZIP (sin el punto) —
    /// le dice a `import` exactamente qué entrada buscar
    /// (`avatar.<ext>`/`avatar_idle.<ext>`/etc.) en vez de tener que
    /// adivinar iterando el archivo. `None` = esa imagen no estaba
    /// configurada en el perfil original, no se incluye nada.
    #[serde(default)]
    pub avatar_extension: Option<String>,
    #[serde(default)]
    pub avatar_idle_extension: Option<String>,
    #[serde(default)]
    pub avatar_thinking_extension: Option<String>,
    #[serde(default)]
    pub avatar_talking_extension: Option<String>,
}

impl ProfileExport {
    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: CURRENT_VERSION,
            name: profile.name.clone(),
            nickname: profile.nickname.clone(),
            description: profile.description.clone(),
            age_fictional: profile.age_fictional.clone(),
            pronouns: profile.pronouns.clone(),
            occupation: profile.occupation.clone(),
            species: profile.species.clone(),
            accent_color: profile.accent_color.clone(),
            theme: profile.theme.clone(),
            identity: profile.identity.clone(),
            personality: profile.personality.clone(),
            voice: profile.voice.clone(),
            avatar_extension: None,
            avatar_idle_extension: None,
            avatar_thinking_extension: None,
            avatar_talking_extension: None,
        }
    }
}

/// Un adjunto de mensaje dentro del ZIP — apunta al archivo real en
/// `attachments/<hash>.<extension>`, nunca repite bytes (varios mensajes
/// pueden compartir el mismo `hash` si subieron la misma imagen, mismo
/// criterio content-addressed que `ChatService::save_attachment` ya usa
/// en disco).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedAttachment {
    pub hash: String,
    pub extension: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedMessage {
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub attachments: Vec<ExportedAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedConversation {
    pub title: String,
    pub source: String,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ExportedMessage>,
}

/// Bytes ya leídos del filesystem, listos para empaquetar — `write_zip`
/// nunca toca el disco él mismo, solo arma el ZIP en memoria (misma
/// separación que el resto del crate: la I/O real vive en `service.rs`).
#[derive(Default)]
pub struct ExportedFiles {
    pub avatar: Option<Vec<u8>>,
    pub avatar_idle: Option<Vec<u8>>,
    pub avatar_thinking: Option<Vec<u8>>,
    pub avatar_talking: Option<Vec<u8>>,
    pub chats: Option<Vec<ExportedConversation>>,
    /// Un adjunto único por hash (deduplicado) — `(hash, extension, bytes)`.
    pub attachments: Vec<(String, String, Vec<u8>)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportFormatError {
    #[error("no se pudo armar el archivo .aivtprofile: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("no se pudo leer el archivo .aivtprofile: {0}")]
    Io(#[from] std::io::Error),
    #[error("el archivo .aivtprofile está dañado o no es válido: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("el archivo .aivtprofile trae un adjunto con un nombre no permitido")]
    UnsafeAttachmentName,
    #[error("el archivo .aivtprofile es demasiado grande")]
    TooLarge,
}

/// Tope por entrada del ZIP y para el total descomprimido. Un `.aivtprofile`
/// llega de fuera (se comparte, se descarga): sin tope, una "bomba zip" de pocos
/// KB podría descomprimirse a gigabytes en memoria. Las imágenes de avatar y los
/// adjuntos reales pesan una fracción de esto.
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

/// `hash` llega del `chats.json` de un archivo ajeno y termina formando una ruta
/// en disco (`attachments/<hash[..2]>/<hash>.<ext>`): solo se acepta lo que de
/// verdad es un hash (hexadecimal) y una extensión simple. Cualquier otra cosa
/// (`../`, `\\`, `/`, espacios, no ASCII) permitiría escribir fuera de la carpeta
/// de adjuntos.
pub(crate) fn is_safe_attachment_name(hash: &str, extension: &str) -> bool {
    (1..=128).contains(&hash.len())
        && hash.bytes().all(|b| b.is_ascii_hexdigit())
        && (1..=8).contains(&extension.len())
        && extension.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Arma los bytes completos del `.aivtprofile` (un ZIP comprimido) a
/// partir de los datos de texto ya resueltos (`profile`) y los archivos ya
/// leídos (`files`).
pub fn write_zip(profile: &ProfileExport, files: &ExportedFiles) -> Result<Vec<u8>, ExportFormatError> {
    let mut buffer = Vec::new();
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buffer));

        zip.start_file(PROFILE_ENTRY, options)?;
        zip.write_all(&serde_json::to_vec_pretty(profile)?)?;

        if let (Some(bytes), Some(ext)) = (&files.avatar, &profile.avatar_extension) {
            zip.start_file(format!("avatar.{ext}"), options)?;
            zip.write_all(bytes)?;
        }
        if let (Some(bytes), Some(ext)) = (&files.avatar_idle, &profile.avatar_idle_extension) {
            zip.start_file(format!("avatar_idle.{ext}"), options)?;
            zip.write_all(bytes)?;
        }
        if let (Some(bytes), Some(ext)) = (&files.avatar_thinking, &profile.avatar_thinking_extension) {
            zip.start_file(format!("avatar_thinking.{ext}"), options)?;
            zip.write_all(bytes)?;
        }
        if let (Some(bytes), Some(ext)) = (&files.avatar_talking, &profile.avatar_talking_extension) {
            zip.start_file(format!("avatar_talking.{ext}"), options)?;
            zip.write_all(bytes)?;
        }

        if let Some(chats) = &files.chats {
            zip.start_file(CHATS_ENTRY, options)?;
            zip.write_all(&serde_json::to_vec_pretty(chats)?)?;

            for (hash, ext, bytes) in &files.attachments {
                zip.start_file(format!("{ATTACHMENTS_DIR}{hash}.{ext}"), options)?;
                zip.write_all(bytes)?;
            }
        }

        zip.finish()?;
    }

    Ok(buffer)
}

/// Lo que `read_zip` devuelve — mismo shape que `ExportedFiles` pero del
/// lado de lectura, más el `profile.json` ya parseado.
#[derive(Debug)]
pub struct ImportedZip {
    pub profile: ProfileExport,
    pub avatar: Option<Vec<u8>>,
    pub avatar_idle: Option<Vec<u8>>,
    pub avatar_thinking: Option<Vec<u8>>,
    pub avatar_talking: Option<Vec<u8>>,
    pub chats: Option<Vec<ExportedConversation>>,
    /// `"<hash>.<extension>" -> bytes` — un lookup directo, ya que
    /// `ExportedAttachment` referencia sus archivos por esa misma clave.
    pub attachments: HashMap<String, Vec<u8>>,
}

fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str, budget: &mut u64) -> Result<Option<Vec<u8>>, ExportFormatError> {
    match archive.by_name(name) {
        Ok(file) => {
            let limit = MAX_ENTRY_BYTES.min(*budget);
            let mut bytes = Vec::new();
            // `take(limit + 1)` lee como mucho un byte de más: si llega, se pasó.
            file.take(limit + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                return Err(ExportFormatError::TooLarge);
            }
            *budget -= bytes.len() as u64;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn read_zip(bytes: &[u8]) -> Result<ImportedZip, ExportFormatError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let mut budget = MAX_TOTAL_BYTES;

    let profile_bytes = read_entry(&mut archive, PROFILE_ENTRY, &mut budget)?
        .ok_or_else(|| ExportFormatError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "falta profile.json")))?;
    let profile: ProfileExport = serde_json::from_slice(&profile_bytes)?;

    let avatar = match &profile.avatar_extension {
        Some(ext) => read_entry(&mut archive, &format!("avatar.{ext}"), &mut budget)?,
        None => None,
    };
    let avatar_idle = match &profile.avatar_idle_extension {
        Some(ext) => read_entry(&mut archive, &format!("avatar_idle.{ext}"), &mut budget)?,
        None => None,
    };
    let avatar_thinking = match &profile.avatar_thinking_extension {
        Some(ext) => read_entry(&mut archive, &format!("avatar_thinking.{ext}"), &mut budget)?,
        None => None,
    };
    let avatar_talking = match &profile.avatar_talking_extension {
        Some(ext) => read_entry(&mut archive, &format!("avatar_talking.{ext}"), &mut budget)?,
        None => None,
    };

    let chats: Option<Vec<ExportedConversation>> = match read_entry(&mut archive, CHATS_ENTRY, &mut budget)? {
        Some(bytes) => Some(serde_json::from_slice(&bytes)?),
        None => None,
    };

    let mut attachments = HashMap::new();
    if let Some(chats) = &chats {
        for conversation in chats {
            for message in &conversation.messages {
                for attachment in &message.attachments {
                    if !is_safe_attachment_name(&attachment.hash, &attachment.extension) {
                        return Err(ExportFormatError::UnsafeAttachmentName);
                    }
                    let key = format!("{}.{}", attachment.hash, attachment.extension);
                    if attachments.contains_key(&key) {
                        continue;
                    }
                    if let Some(bytes) = read_entry(&mut archive, &format!("{ATTACHMENTS_DIR}{key}"), &mut budget)? {
                        attachments.insert(key, bytes);
                    }
                }
            }
        }
    }

    Ok(ImportedZip { profile, avatar, avatar_idle, avatar_thinking, avatar_talking, chats, attachments })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_export() -> ProfileExport {
        ProfileExport {
            format: FORMAT.to_string(),
            version: CURRENT_VERSION,
            name: "Ciel".to_string(),
            nickname: Some("Ci".to_string()),
            description: None,
            age_fictional: None,
            pronouns: None,
            occupation: None,
            species: None,
            accent_color: None,
            theme: None,
            identity: ProfileIdentity::default(),
            personality: ProfilePersonality::default(),
            voice: ProfileVoice::default(),
            avatar_extension: None,
            avatar_idle_extension: None,
            avatar_thinking_extension: None,
            avatar_talking_extension: None,
        }
    }

    #[test]
    fn round_trips_through_json() {
        let export = sample_export();
        let json = serde_json::to_string(&export).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("token"));

        let parsed: ProfileExport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Ciel");
        assert_eq!(parsed.format, FORMAT);
    }

    /// A `.aivtprofile` exported before Fase 5 has no `voice` key at all —
    /// must still import instead of erroring (see the `#[serde(default)]`
    /// on `ProfileExport::voice`).
    #[test]
    fn a_profile_exported_before_voice_existed_still_imports() {
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
            "identity": {"lore":"","relationshipToUser":"","userRole":"","speakingStyle":"","specialInstructions":""},
            "personality": {"freeDescription":"","warmth":50,"empathy":50,"humor":50,"sarcasm":20,"formality":30,"curiosity":50,"energy":50,"spontaneity":50,"initiative":40,"affection":50,"shyness":30,"confidence":50,"expressiveness":50,"verbosity":50,"flirtiness":0,"emotionalIntensity":50,"teasingTendency":30,"questionTendency":40,"initiateConversationTendency":30}
        }"#;

        let parsed: ProfileExport = serde_json::from_str(json).unwrap();
        assert!(!parsed.voice.enabled);
        assert_eq!(parsed.voice.rate, 1.0);
    }

    /// A `.aivtprofile` exported before the theme-presets feature has no
    /// `theme` key at all — must still import as "Personalizado" instead
    /// of erroring (see the `#[serde(default)]` on `ProfileExport::theme`).
    #[test]
    fn a_profile_exported_before_theme_existed_still_imports() {
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
            "identity": {"lore":"","relationshipToUser":"","userRole":"","speakingStyle":"","specialInstructions":""},
            "personality": {"freeDescription":"","warmth":50,"empathy":50,"humor":50,"sarcasm":20,"formality":30,"curiosity":50,"energy":50,"spontaneity":50,"initiative":40,"affection":50,"shyness":30,"confidence":50,"expressiveness":50,"verbosity":50,"flirtiness":0,"emotionalIntensity":50,"teasingTendency":30,"questionTendency":40,"initiateConversationTendency":30}
        }"#;

        let parsed: ProfileExport = serde_json::from_str(json).unwrap();
        assert!(parsed.theme.is_none());
    }

    #[test]
    fn write_then_read_zip_round_trips_profile_and_avatar() {
        let mut export = sample_export();
        export.avatar_extension = Some("png".to_string());
        let files = ExportedFiles { avatar: Some(vec![1, 2, 3, 4]), ..Default::default() };

        let zip_bytes = write_zip(&export, &files).unwrap();
        let imported = read_zip(&zip_bytes).unwrap();

        assert_eq!(imported.profile.name, "Ciel");
        assert_eq!(imported.avatar, Some(vec![1, 2, 3, 4]));
        assert!(imported.avatar_idle.is_none());
        assert!(imported.chats.is_none());
    }

    #[test]
    fn write_then_read_zip_round_trips_chats_and_dedupes_shared_attachments() {
        let export = sample_export();
        let shared_attachment =
            ExportedAttachment { hash: "abc123".to_string(), extension: "png".to_string(), mime_type: "image/png".to_string(), size_bytes: 4 };
        let chats = vec![ExportedConversation {
            title: "Chat 1".to_string(),
            source: "chat".to_string(),
            model: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:05:00Z".to_string(),
            messages: vec![
                ExportedMessage {
                    role: MessageRole::User,
                    content: "hola".to_string(),
                    status: MessageStatus::Complete,
                    provider_id: None,
                    model: None,
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                    attachments: vec![shared_attachment.clone()],
                },
                ExportedMessage {
                    role: MessageRole::Assistant,
                    content: "de nuevo".to_string(),
                    status: MessageStatus::Complete,
                    provider_id: None,
                    model: None,
                    created_at: "2026-01-01T00:01:00Z".to_string(),
                    attachments: vec![shared_attachment],
                },
            ],
        }];
        let files = ExportedFiles {
            chats: Some(chats),
            attachments: vec![("abc123".to_string(), "png".to_string(), vec![9, 9, 9, 9])],
            ..Default::default()
        };

        let zip_bytes = write_zip(&export, &files).unwrap();
        let imported = read_zip(&zip_bytes).unwrap();

        let chats = imported.chats.unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].messages.len(), 2);
        assert_eq!(imported.attachments.len(), 1, "el mismo hash+ext no debe duplicarse en el ZIP");
        assert_eq!(imported.attachments.get("abc123.png"), Some(&vec![9, 9, 9, 9]));
    }

    #[test]
    fn read_zip_rejects_a_plain_non_zip_file() {
        let err = read_zip(b"{\"format\":\"aivtprofile\"}").unwrap_err();
        assert!(matches!(err, ExportFormatError::Zip(_)));
    }
    #[test]
    fn attachment_names_must_be_a_hex_hash_and_a_plain_extension() {
        let hash = "a".repeat(64);
        assert!(is_safe_attachment_name(&hash, "png"));
        assert!(is_safe_attachment_name("0123abcdEF", "mp3"));

        for bad_hash in ["../../evil", "..\\..\\evil", "ab/cd1234", "abcd 1234", "ééééééééé", "xyz", ""] {
            assert!(!is_safe_attachment_name(bad_hash, "png"), "{bad_hash:?} no debería aceptarse");
        }
        for bad_ext in ["", "p/ng", "..", "png.exe", "verylongext", "pñg"] {
            assert!(!is_safe_attachment_name(&hash, bad_ext), "{bad_ext:?} no debería aceptarse");
        }
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
            for (name, data) in entries {
                writer.start_file(*name, SimpleFileOptions::default()).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer
    }

    #[test]
    fn an_entry_larger_than_the_budget_is_rejected() {
        let bytes = zip_with(&[("profile.json", &[b'x'; 100])]);
        let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut budget = 10;
        assert!(matches!(read_entry(&mut archive, "profile.json", &mut budget), Err(ExportFormatError::TooLarge)));
    }

    #[test]
    fn reading_spends_the_budget() {
        let bytes = zip_with(&[("a.png", &[1u8; 30]), ("b.png", &[2u8; 30])]);
        let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut budget = 50;
        assert_eq!(read_entry(&mut archive, "a.png", &mut budget).unwrap().unwrap().len(), 30);
        assert_eq!(budget, 20);
        assert!(matches!(read_entry(&mut archive, "b.png", &mut budget), Err(ExportFormatError::TooLarge)));
    }

    #[test]
    fn a_profile_with_a_path_traversal_attachment_is_rejected_before_anything_is_written() {
        let mut export = sample_export();
        export.version = CURRENT_VERSION;
        let profile_json = serde_json::to_vec(&export).unwrap();
        let chats = serde_json::json!([{
            "title": "t", "source": "chat", "model": null, "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
            "messages": [{
                "role": "assistant", "content": "x", "status": "complete", "providerId": null, "model": null, "createdAt": "2026-01-01T00:00:00Z",
                "attachments": [{ "hash": "../../evil", "extension": "bat", "mimeType": "image/png", "sizeBytes": 1 }]
            }]
        }]);
        let bytes = zip_with(&[("profile.json", &profile_json), ("chats.json", chats.to_string().as_bytes()), ("attachments/../../evil.bat", b"MZ")]);
        assert!(matches!(read_zip(&bytes), Err(ExportFormatError::UnsafeAttachmentName)));
    }
}
