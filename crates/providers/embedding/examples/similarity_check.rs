//! One-off diagnostic, not part of the app: measures real cosine
//! similarity between a stored memory's text and a handful of unrelated
//! queries, using the actual downloaded local embedding model — to check
//! whether `MIN_SEMANTIC_SIMILARITY` (aivt-database's
//! `MemoriesRepository::search_by_embedding`) is actually discriminating
//! for this model, instead of guessing a threshold value from theory.
//!
//! Run with: cargo run -p aivt-provider-embedding --example similarity_check

use aivt_eventbus::EventBus;
use aivt_provider_embedding::{EmbeddingModelManager, EmbeddingProvider, LocalEmbeddingProvider};

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let models_dir = std::env::var("AIVT_EMBEDDING_MODELS_DIR")
        .unwrap_or_else(|_| "C:/Users/Vulito/AppData/Roaming/com.projectciel.app/models/embedding".to_string());
    let manager = EmbeddingModelManager::new(models_dir.into(), EventBus::new());
    let provider = LocalEmbeddingProvider::new(manager);

    // The real stored memory (from the live database) — same
    // `"{title}\n{summary}"` shape `embedding_text` builds in
    // crates/memory/src/service.rs, so this is exactly what's actually
    // sitting in `memory_embeddings` for it today.
    let memory_text = "Solicitud de configuración de video generación…\nEl usuario quiere configurar un sistema local para generar videos con su RX 6750 XT usando lenguaje natural.";
    let memory_vector = provider.embed(memory_text).await.expect("embed memory text");

    let queries = [
        "hablame de mi placa de video",
        "quiero armar algo para generar videos localmente",
        "hola, ¿qué tal?",
        "¿qué día es hoy?",
        "cuéntame un chiste",
        "estoy cansado hoy",
        "me gusta el senderismo",
        "¿cómo te sientes?",
    ];

    println!("Memoria: {memory_text}\n");
    for query in queries {
        let query_vector = provider.embed(query).await.expect("embed query");
        let score = dot(&memory_vector, &query_vector);
        println!("{score:.4}  {query}");
    }
}
