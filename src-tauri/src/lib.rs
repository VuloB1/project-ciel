mod commands;
mod state;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Manager, WindowEvent};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use aivt_eventbus::{AppEvent, EventBus};
use aivt_memory::{MemoryManager, MemoryService, PeopleManager, PeopleService};
use aivt_provider_embedding::{EmbeddingModelManager, EmbeddingProvider, LocalEmbeddingProvider};
use aivt_provider_stt::SttModelManager;
use aivt_provider_tts::{AivtTtsProvider, TTSProvider};
use aivt_secrets::{KeyringSecretStore, SecretStore};
use aivt_service_activity::ActivityService;
use aivt_service_avatar::AvatarServerService;
use aivt_service_call::CallService;
use aivt_service_chat::ChatService;
use aivt_service_discord::DiscordService;
use aivt_service_manager::ServiceManager;
use aivt_service_proactive::ProactiveService;
use aivt_service_profiles::ProfileService;
use aivt_service_settings::SettingsService;
use aivt_service_twitch::TwitchService;

use state::AppState;

const EVENT_BRIDGE_CHANNEL: &str = "app://event";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Busca, entre los argumentos de lanzamiento (`argv[1..]`, sea el arranque
/// en frío o los de una segunda instancia que este proceso interceptó vía
/// `tauri-plugin-single-instance`), el primero que termine en
/// `.aivtprofile` (personaje) o `.aivtbackup` (copia de seguridad) — así es
/// como el sistema operativo pasa el archivo con el que se hizo doble clic;
/// el frontend decide por la extensión qué confirmación mostrar. `argv[0]`
/// (el propio ejecutable) nunca termina en ninguna de las dos, así que no
/// hace falta saltearlo explícitamente.
fn find_profile_file_arg<'a, I: IntoIterator<Item = &'a String>>(args: I) -> Option<String> {
    args.into_iter()
        .find(|arg| {
            let lower = arg.to_lowercase();
            lower.ends_with(".aivtprofile") || lower.ends_with(".aivtbackup")
        })
        .cloned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Must be registered before any other plugin — see the plugin's docs.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                // `.show()` first — a window hidden via `.hide()` (minimizada
                // a la bandeja) isn't the same state as OS-minimized, and
                // `unminimize()`/`set_focus()` alone don't bring it back.
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }

            // Doble clic en un `.aivtprofile` con la app YA corriendo: Tauri
            // relanza el ejecutable (que este plugin intercepta) con el path
            // del archivo como argumento. A diferencia del arranque en frío
            // (ver `pending_import_path` en `state.rs`), acá el frontend ya
            // lleva rato corriendo y suscripto — publicar el evento directo
            // es seguro.
            if let Some(path) = find_profile_file_arg(&argv) {
                if let Some(state) = app.try_state::<AppState>() {
                    state.event_bus.publish(AppEvent::ProfileImportRequested { path });
                }
            }
        }));
    }

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_autostart::Builder::new().build());
    }

    builder = builder.plugin(tauri_plugin_dialog::init());
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.plugin(tauri_plugin_notification::init());

    builder
        .setup(|app| {
            let handle = app.handle().clone();

            // Arranque en frío con un `.aivtprofile` (doble clic sin la app
            // corriendo todavía) — ver el doc comment de
            // `AppState::pending_import_path` para por qué esto no usa el
            // `AppEvent` que sí usa el caso de la app ya corriendo, más
            // abajo en `tauri_plugin_single_instance::init`.
            let args: Vec<String> = std::env::args().collect();
            let pending_import_path = find_profile_file_arg(args.iter().skip(1));

            let app_data_dir = handle.path().app_data_dir()?;
            let logs_dir = app_data_dir.join("logs");
            std::fs::create_dir_all(&logs_dir)?;

            let file_appender = tracing_appender::rolling::daily(&logs_dir, "aivt.log");
            let (file_writer, log_guard) = tracing_appender::non_blocking(file_appender);

            let db_path = app_data_dir.join("aivt.db");
            // Si el usuario preparó una restauración, se aplica AHORA: antes de abrir la
            // base, que es cuando ningún archivo está en uso. Ver `aivt_service_backup`.
            let restore_result = aivt_service_backup::apply_pending_restore(&app_data_dir);
            let secrets: Arc<dyn SecretStore> = Arc::new(KeyringSecretStore::new());

            let (pool, settings, profiles, activity, activity_layer, language) = tauri::async_runtime::block_on({
                let secrets = secrets.clone();
                let profiles_data_dir = app_data_dir.clone();
                async move {
                    let pool = aivt_database::connect(&db_path)
                        .await
                        .expect("failed to open database");
                    aivt_database::migrate(&pool)
                        .await
                        .expect("failed to run database migrations");

                    let settings = SettingsService::new(pool.clone(), secrets);
                    let language = settings.language().await.unwrap_or_else(|_| "es".to_string());
                    let profiles = ProfileService::new(pool.clone(), profiles_data_dir);
                    let (activity, activity_layer) = ActivityService::spawn(pool.clone());
                    (pool, settings, profiles, activity, activity_layer, language)
                }
            });

            let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
            let fmt_layer = tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .with(activity_layer)
                .init();

            tracing::info!(
                target: "activity",
                category = "SYSTEM",
                module = "bootstrap",
                action = "startup",
                summary = %startup_success_message(&language),
            );

            match &restore_result {
                Ok(Some(outcome)) => tracing::info!(
                    target: "system",
                    "backup: copia de seguridad restaurada; los datos anteriores quedaron en {}",
                    outcome.previous_data
                ),
                Ok(None) => {}
                Err(err) => tracing::error!(target: "system", "backup: no se pudo aplicar la restauración, se siguen usando los datos actuales: {err}"),
            }

            let event_bus = EventBus::new();

            // Bridge: internal EventBus -> frontend, via Tauri's own event
            // system. The frontend subscribes with `listen(EVENT_BRIDGE_CHANNEL, ...)`.
            {
                let mut rx = event_bus.subscribe();
                let emit_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(event) => {
                                let _ = emit_handle.emit(EVENT_BRIDGE_CHANNEL, &event);
                            }
                            // Fell behind the publisher — a handful of
                            // events were dropped, but the bridge itself
                            // should keep running.
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });
            }

            // Proves the EventBus -> Tauri emit -> frontend bridge works
            // end-to-end before any real producer (Chat/Twitch/Discord/Call)
            // exists. Safe to remove once one does.
            {
                let heartbeat_bus = event_bus.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(HEARTBEAT_INTERVAL).await;
                        heartbeat_bus.publish(AppEvent::Heartbeat {
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                });
            }

            // Constructed before `memory` on purpose — `MemoryService`
            // takes it (wrapped in a `LocalEmbeddingProvider`) as a
            // constructor argument, same relationship `stt_models` has
            // with `call`.
            let embedding_models =
                EmbeddingModelManager::new(app_data_dir.join("models").join("embedding"), event_bus.clone());
            let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(LocalEmbeddingProvider::new(embedding_models.clone()));
            let memory: Arc<dyn MemoryManager> =
                Arc::new(MemoryService::new(pool.clone(), embedding_provider, event_bus.clone()));
            let people: Arc<dyn PeopleManager> = Arc::new(PeopleService::new(pool.clone(), secrets.clone()));
            let chat = ChatService::new(pool.clone(), event_bus.clone(), app_data_dir.clone(), memory.clone());
            // One-shot cleanup of audio clips saved by Chat's optional
            // "guardar audio de los mensajes" feature past their configured
            // retention — see `ChatService::prune_old_audio`. Runs once per
            // launch rather than on a recurring timer: this is a desktop
            // app the user restarts often enough that a startup sweep keeps
            // disk usage bounded without needing a live background
            // scheduler. Best-effort: a settings-read or DB failure here
            // must never block startup.
            {
                let chat = chat.clone();
                let settings = settings.clone();
                tauri::async_runtime::spawn(async move {
                    let retention_days = settings.chat_audio_settings().await.map(|s| s.retention_days).unwrap_or(90);
                    match chat.prune_old_audio(retention_days).await {
                        Ok(0) => {}
                        Ok(deleted) => {
                            tracing::info!(target: "system", "chat: se eliminaron {deleted} audios guardados que superaron la retención configurada");
                        }
                        Err(err) => tracing::warn!(target: "system", "chat: no se pudo limpiar audios de mensajes vencidos: {err}"),
                    }
                });
            }
            // Seeds `AivtTtsProvider`'s in-memory mute flag from the last
            // persisted value — a restart shouldn't un-mute the app behind
            // the user's back. See `SettingsService::voice_muted`.
            let voice_muted = tauri::async_runtime::block_on(settings.voice_muted()).unwrap_or(false);
            // Same reasoning as `voice_muted` above, for the "minimizar a la
            // bandeja"/"reproducir voz en la bandeja" pair — see
            // `SettingsService::background_settings`.
            let background_settings = tauri::async_runtime::block_on(settings.background_settings()).unwrap_or_default();
            // Backfill del flag de onboarding (sección Créditos + wizard de
            // bienvenida): es un flag nuevo, así que una instalación que ya
            // tenía perfiles antes de que existiera nunca lo tendría
            // guardado — sin esto, `onboarding_completed() == false` por
            // defecto forzaría el wizard de "primera vez" a usuarios que ya
            // llevan tiempo usando la app. `ProfileService` descartable,
            // mismo patrón que `avatar_server_profiles` más abajo. Best
            // effort: nunca bloquea el arranque si falla.
            if !tauri::async_runtime::block_on(settings.onboarding_completed()).unwrap_or(false) {
                let existing_profiles = ProfileService::new(pool.clone(), app_data_dir.clone());
                let has_profiles = tauri::async_runtime::block_on(existing_profiles.list())
                    .map(|profiles| !profiles.is_empty())
                    .unwrap_or(false);
                if has_profiles {
                    let _ = tauri::async_runtime::block_on(settings.set_onboarding_completed(true));
                }
            }
            let minimize_to_tray_enabled = Arc::new(AtomicBool::new(background_settings.minimize_to_tray));
            let tts: Arc<dyn TTSProvider> =
                Arc::new(AivtTtsProvider::new(event_bus.clone(), voice_muted, background_settings.voice_while_hidden));
            // La salida de audio elegida se aplica ya al arrancar.
            if let Ok(Some(device)) = tauri::async_runtime::block_on(settings.audio_output_device()) {
                tts.set_output_device(Some(device));
            }
            let stt_models = SttModelManager::new(app_data_dir.join("models").join("whisper"), event_bus.clone());

            let twitch = TwitchService::new(pool.clone(), event_bus.clone(), secrets.clone(), tts.clone(), people.clone(), memory.clone());
            // No `tts` argument — Discord replies are never spoken,
            // regardless of the global voice mute, see
            // `aivt_service_discord::service::Inner::generate_reply`'s doc
            // comment.
            let discord = DiscordService::new(pool.clone(), event_bus.clone(), secrets.clone(), people.clone(), memory.clone());
            let proactive = ProactiveService::new(pool.clone(), secrets.clone(), memory.clone(), chat.clone());
            // Fresh `ProfileService` rather than a shared/cloned handle to
            // the one above — that crate isn't `Clone` (plain `DbPool` +
            // `PathBuf`, no interior caching to worry about), and every
            // other service in this file that needs read-only access to
            // cheap, stateless glue over `pool` already does the same thing
            // (see `CallService::Inner::settings()`/`ProactiveService::Inner::settings()`
            // constructing their own `SettingsService` on demand instead of
            // sharing one).
            let avatar_server_profiles = Arc::new(ProfileService::new(pool.clone(), app_data_dir.clone()));
            let avatar_server = AvatarServerService::new(avatar_server_profiles, event_bus.clone(), settings.clone());
            let call = CallService::new(pool.clone(), event_bus.clone(), secrets, tts.clone(), stt_models.clone(), memory.clone());
            let service_manager = Arc::new(ServiceManager::new(event_bus.clone()));
            tauri::async_runtime::block_on(service_manager.register(Arc::new(twitch.clone())));
            tauri::async_runtime::block_on(service_manager.register(Arc::new(discord.clone())));
            tauri::async_runtime::block_on(service_manager.register(Arc::new(call.clone())));
            tauri::async_runtime::block_on(service_manager.register(Arc::new(proactive.clone())));
            tauri::async_runtime::block_on(service_manager.register(Arc::new(avatar_server.clone())));
            // Opt-in like Twitch/Discord/Call (never silently binds a port),
            // but unlike those, it auto-resumes on every future launch once
            // turned on once — mismo patrón que `voice_muted`/`background_settings`
            // más abajo, leído una vez con `block_on` antes de `app.manage(...)`.
            let avatar_server_settings = tauri::async_runtime::block_on(settings.avatar_server_settings()).unwrap_or_default();
            if avatar_server_settings.enabled {
                if let Err(err) = tauri::async_runtime::block_on(service_manager.start("avatar_server")) {
                    tracing::warn!(target: "system", "no se pudo arrancar el servidor del avatar para OBS: {err}");
                }
            }
            // A diferencia de twitch/discord/call (que esperan una conexión
            // explícita del usuario desde su propia pestaña), el motor de
            // decisión debe correr siempre que la app esté abierta — el
            // on/off real vive en `profile_proactive.enabled` por perfil,
            // no en si este `ManagedService` está arrancado.
            if let Err(err) = tauri::async_runtime::block_on(service_manager.start("proactive")) {
                tracing::warn!(target: "system", "no se pudo arrancar el motor de modo proactivo: {err}");
            }

            // Bandeja del sistema ("Minimizar a la bandeja al cerrar",
            // Configuración > General) — clic izquierdo muestra la ventana
            // principal; clic derecho posiciona y muestra `tray-menu`, una
            // ventanita propia sin bordes con el mismo aspecto que
            // `AppContextMenu.tsx`, en vez del menú nativo del SO (que no
            // puede seguir los colores/redondez del tema de la app) — ver
            // `TrayMenuWindow.tsx` y `commands::window`. El cierre de la
            // ventana principal se intercepta más abajo, que la esconde en
            // vez de cerrarla cuando `minimize_to_tray_enabled` está activo.
            #[cfg(desktop)]
            {
                use tauri::tray::TrayIconBuilder;
                use tauri::{PhysicalPosition, Position};

                // Separación entre el ícono de la bandeja y `tray-menu`, en
                // píxeles físicos — mismo espíritu que `VIEWPORT_MARGIN` en
                // `AppContextMenu.tsx`.
                const TRAY_MENU_MARGIN: i32 = 8;

                TrayIconBuilder::new()
                    .icon(app.default_window_icon().cloned().expect("no default window icon"))
                    .tooltip("Project Ciel")
                    .on_tray_icon_event(|tray, event| {
                        let app = tray.app_handle();
                        match event {
                            tauri::tray::TrayIconEvent::Click {
                                button: tauri::tray::MouseButton::Left,
                                button_state: tauri::tray::MouseButtonState::Up,
                                ..
                            } => {
                                if let Some(menu_window) = app.get_webview_window("tray-menu") {
                                    let _ = menu_window.hide();
                                }
                                commands::window::tray_show_main_window(app.clone());
                            }
                            tauri::tray::TrayIconEvent::Click {
                                button: tauri::tray::MouseButton::Right,
                                button_state: tauri::tray::MouseButtonState::Up,
                                position,
                                ..
                            } => {
                                let Some(menu_window) = app.get_webview_window("tray-menu") else {
                                    return;
                                };
                                let Ok(size) = menu_window.outer_size() else {
                                    return;
                                };
                                // Ancla el borde superior de `tray-menu` justo
                                // arriba del ícono, centrado horizontalmente
                                // en el punto del clic — recortado contra el
                                // área de trabajo del monitor bajo el cursor
                                // (excluye la barra de tareas) para no
                                // dibujarse fuera de pantalla ni debajo de
                                // ella.
                                let mut x = position.x as i32 - size.width as i32 / 2;
                                let mut y = position.y as i32 - size.height as i32 - TRAY_MENU_MARGIN;
                                if let Ok(Some(monitor)) = menu_window.monitor_from_point(position.x, position.y) {
                                    let work = monitor.work_area();
                                    let min_x = work.position.x;
                                    let max_x = (work.position.x + work.size.width as i32 - size.width as i32).max(min_x);
                                    x = x.clamp(min_x, max_x);
                                    y = y.max(work.position.y);
                                }
                                let _ = menu_window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                                let _ = menu_window.show();
                                let _ = menu_window.set_focus();
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;
            }

            if let Some(window) = app.get_webview_window("main") {
                let minimize_to_tray_enabled = minimize_to_tray_enabled.clone();
                let tts_for_window = tts.clone();
                let window_to_hide = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        if minimize_to_tray_enabled.load(Ordering::Relaxed) {
                            api.prevent_close();
                            let _ = window_to_hide.hide();
                            tts_for_window.set_window_hidden(true);
                        }
                    }
                    WindowEvent::Focused(true) => {
                        // Cubre las tres formas de volver a mostrar la
                        // ventana (menú de la bandeja, click en el ícono,
                        // relanzar una segunda instancia) sin repetir este
                        // aviso en cada una — todas terminan en
                        // `set_focus()`, que dispara este mismo evento.
                        tts_for_window.set_window_hidden(false);
                    }
                    _ => {}
                });
            }

            // `tray-menu` se esconde sola al perder el foco (clic afuera,
            // Alt+Tab, etc.) — mismo dismiss-on-blur que `AppContextMenu.tsx`
            // hace con `window.addEventListener("blur", ...)`, pero acá es a
            // nivel de ventana del SO porque `tray-menu` es una ventana
            // Tauri propia, no un overlay dentro de `main`. Escape lo cubre
            // `commands::window::hide_window`, invocado desde
            // `TrayMenuWindow.tsx`.
            if let Some(menu_window) = app.get_webview_window("tray-menu") {
                let menu_window_to_hide = menu_window.clone();
                menu_window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = menu_window_to_hide.hide();
                    }
                });
            }

            app.manage(AppState {
                pool,
                data_dir: app_data_dir.clone(),
                settings,
                profiles,
                chat,
                memory,
                people,
                tts,
                stt_models,
                embedding_models,
                twitch,
                discord,
                call,
                proactive,
                _avatar_server: avatar_server,
                service_manager,
                event_bus,
                pending_import_path: std::sync::Mutex::new(pending_import_path),
                activity,
                minimize_to_tray_enabled,
                _log_guard: log_guard,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::settings_get_language,
            commands::settings::settings_set_language,
            commands::settings::settings_get_user_name,
            commands::settings::settings_set_user_name,
            commands::settings::settings_get_background,
            commands::settings::settings_set_background,
            commands::settings::settings_get_appearance,
            commands::settings::settings_set_appearance,
            commands::settings::settings_get_onboarding_completed,
            commands::settings::settings_set_onboarding_completed,
            commands::settings::open_external_url,
            commands::settings::open_legal_document,
            commands::settings::open_devtools,
            commands::settings::settings_list_llm_providers,
            commands::settings::settings_get_llm_usage,
            commands::settings::settings_reset_llm_usage,
            commands::settings::settings_add_llm_provider,
            commands::settings::settings_update_llm_provider,
            commands::settings::settings_remove_llm_provider,
            commands::settings::settings_set_llm_provider_enabled,
            commands::settings::settings_set_llm_provider_pinned,
            commands::settings::settings_set_current_llm_provider,
            commands::settings::settings_test_llm_provider,
            commands::settings::settings_list_tts_providers,
            commands::settings::settings_add_tts_provider,
            commands::settings::settings_update_tts_provider,
            commands::settings::settings_remove_tts_provider,
            commands::settings::settings_set_tts_provider_enabled,
            commands::settings::settings_set_current_tts_provider,
            commands::settings::settings_test_tts_provider,
            commands::stt::dictation_get_settings,
            commands::stt::dictation_set_settings,
            commands::stt::dictation_start,
            commands::stt::dictation_stop,
            commands::stt::dictation_cancel,
            commands::stt::stt_get_model,
            commands::stt::stt_set_model,
            commands::stt::stt_list_model_status,
            commands::stt::stt_download_model,
            commands::stt::stt_delete_model,
            commands::embedding::embedding_get_model_status,
            commands::embedding::embedding_download_model,
            commands::embedding::embedding_delete_model,
            commands::profiles::profiles_list,
            commands::profiles::profiles_create,
            commands::profiles::profiles_update,
            commands::profiles::profiles_duplicate,
            commands::profiles::profiles_delete,
            commands::profiles::profiles_set_avatar,
            commands::profiles::profiles_get_avatar,
            commands::profiles::user_avatar_set,
            commands::profiles::user_avatar_get,
            commands::profiles::profiles_export_to_path,
            commands::profiles::profiles_import_from_path,
            commands::profiles::profiles_take_pending_import_path,
            commands::profiles::profiles_preview_prompt,
            commands::chat::chat_list_conversations,
            commands::chat::chat_create_conversation,
            commands::chat::chat_get_messages,
            commands::chat::chat_get_attachment_base64,
            commands::chat::chat_rename_conversation,
            commands::chat::chat_archive_conversation,
            commands::chat::chat_delete_conversation,
            commands::chat::chat_cancel_generation,
            commands::chat::chat_send_message,
            commands::chat::chat_extract_memories,
            commands::chat::chat_import_call_transcript,
            commands::chat::chat_set_conversation_pinned,
            commands::chat::chat_reorder_pinned_conversations,
            commands::chat::chat_get_audio_settings,
            commands::chat::chat_set_audio_settings,
            commands::memory::memory_list,
            commands::memory::memory_search,
            commands::memory::memory_create,
            commands::diagnostics::diagnostics_run,
            commands::diagnostics::diagnostics_text,
            commands::diagnostics::diagnostics_save,
            commands::backup::backup_create,
            commands::backup::backup_prepare_restore,
            commands::backup::backup_cancel_restore,
            commands::backup::backup_restart_app,
            commands::memory::memory_set_pinned,
            commands::memory::memory_set_scope,
            commands::memory::memory_update,
            commands::memory::memory_delete,
            commands::memory::memory_backfill_embeddings,
            commands::memory::memory_propose_fusion,
            commands::memory::memory_apply_fusion,
            commands::mood::mood_get,
            commands::mood::mood_reset,
            commands::proactive::proactive_get_settings,
            commands::proactive::proactive_set_enabled,
            commands::proactive::proactive_set_active_profile,
            commands::people::people_list,
            commands::people::people_create,
            commands::people::people_delete,
            commands::people::people_get_policy,
            commands::people::people_set_policy,
            commands::voice::voice_list_voices,
            commands::voice::voice_search_voices,
            commands::voice::voice_get_settings,
            commands::voice::voice_set_settings,
            commands::voice::voice_speak,
            commands::voice::voice_stop,
            commands::voice::voice_get_muted,
            commands::voice::voice_set_muted,
            commands::voice::voice_list_output_devices,
            commands::voice::voice_get_output_device,
            commands::voice::voice_set_output_device,
            commands::web_search::web_search_get_settings,
            commands::web_search::web_search_set_settings,
            commands::activity::activity_recent,
            commands::activity::activity_clear,
            commands::notifications::notifications_send_proactive,
            commands::avatar::avatar_set_state_image,
            commands::avatar::avatar_get_state_image,
            commands::avatar::avatar_server_get_status,
            commands::avatar::avatar_server_set_enabled,
            commands::twitch::twitch_get_connection,
            commands::twitch::twitch_get_policy,
            commands::twitch::twitch_set_policy,
            commands::twitch::twitch_connect_account,
            commands::twitch::twitch_disconnect_account,
            commands::twitch::twitch_start,
            commands::twitch::twitch_stop,
            commands::twitch::twitch_get_status,
            commands::twitch::twitch_set_active_profile,
            commands::twitch::twitch_send_test_message,
            commands::twitch::twitch_set_target_channel,
            commands::twitch::twitch_get_emote_settings,
            commands::twitch::twitch_set_emote_settings,
            commands::twitch::twitch_refresh_emotes,
            commands::twitch::twitch_get_emote_status,
            commands::twitch::twitch_get_emotes,
            commands::twitch::twitch_start_emote_captioning,
            commands::discord::discord_get_connection,
            commands::discord::discord_get_policy,
            commands::discord::discord_set_policy,
            commands::discord::discord_connect_bot,
            commands::discord::discord_disconnect_account,
            commands::discord::discord_start,
            commands::discord::discord_stop,
            commands::discord::discord_get_status,
            commands::discord::discord_set_active_profile,
            commands::discord::discord_send_test_message,
            commands::call::call_get_policy,
            commands::call::call_set_policy,
            commands::call::call_list_microphones,
            commands::call::call_set_muted,
            commands::call::call_mic_test_start,
            commands::call::call_mic_test_stop,
            commands::call::call_is_muted,
            commands::call::call_start,
            commands::call::call_stop,
            commands::call::call_get_status,
            commands::call::call_set_active_profile,
            commands::window::tray_show_main_window,
            commands::window::tray_quit_app,
            commands::window::tray_reload_main_window,
            commands::window::tray_resize_menu_window,
            commands::window::hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn startup_success_message(lang: &str) -> &'static str {
    match lang {
        "en" => "Project Ciel started successfully",
        "pt" => "Project Ciel iniciado com sucesso",
        "ja" => "Project Ciel が正常に起動しました",
        "ko" => "Project Ciel이 정상적으로 시작되었습니다",
        "zh" => "Project Ciel 已成功启动",
        _ => "Project Ciel se inició correctamente",
    }
}
