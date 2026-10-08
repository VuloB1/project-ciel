use aivt_core::AppError;
use aivt_database::{NewPerson, Person};
use aivt_memory::MemoryError;
use aivt_service_settings::PeoplePolicy;

use crate::state::AppState;

fn map_err(err: MemoryError) -> AppError {
    let code = match &err {
        MemoryError::Database(_) => "PEOPLE_DATABASE_ERROR",
        MemoryError::Llm(_) => "PEOPLE_LLM_ERROR",
        MemoryError::ExtractionParseFailed => "PEOPLE_EXTRACTION_PARSE_FAILED",
    };
    AppError::from_display("people", code, err)
}

#[tauri::command]
pub async fn people_list(state: tauri::State<'_, AppState>) -> Result<Vec<Person>, AppError> {
    state.people.list_all().await.map_err(map_err)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonInput {
    pub canonical_name: String,
    pub role: String,
}

#[tauri::command]
pub async fn people_create(state: tauri::State<'_, AppState>, input: PersonInput) -> Result<Person, AppError> {
    state.people.create_manual(NewPerson { canonical_name: input.canonical_name, role: input.role }).await.map_err(map_err)
}

#[tauri::command]
pub async fn people_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.people.delete(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn people_get_policy(state: tauri::State<'_, AppState>) -> Result<PeoplePolicy, AppError> {
    state.settings.people_policy().await.map_err(|err| AppError::from_display("people", "PEOPLE_POLICY_READ_FAILED", err))
}

#[tauri::command]
pub async fn people_set_policy(state: tauri::State<'_, AppState>, policy: PeoplePolicy) -> Result<PeoplePolicy, AppError> {
    state.settings.set_people_policy(policy).await.map_err(|err| AppError::from_display("people", "PEOPLE_POLICY_WRITE_FAILED", err))
}
