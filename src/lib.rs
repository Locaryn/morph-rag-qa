//! Locaryn RAG & Question-Answering Plugin
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentRequest {
    pub file_path: String,
    pub chunk_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentResult {
    pub chunks_indexed: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryRequest {
    pub query: String,
    pub top_k: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunkCitation {
    pub file_path: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryResponse {
    pub answer: String,
    pub citations: Vec<RagChunkCitation>,
}

/// Non implemente. La signature est conservee pour que l'interface et le
/// serveur MCP gardent leur forme, mais l'appel echoue franchement plutot
/// que de fabriquer un resultat.
pub async fn index_document(_req: IndexDocumentRequest) -> Result<IndexDocumentResult, String> {
    Err("L'indexation n'est pas implementee : ce morph ne calcule aucun plongement et n'ecrit dans aucun index.".into())
}

/// Non implemente. La signature est conservee pour que l'interface et le
/// serveur MCP gardent leur forme, mais l'appel echoue franchement plutot
/// que de fabriquer un resultat.
pub async fn answer_question(_req: RagQueryRequest) -> Result<RagQueryResponse, String> {
    Err("La reponse sur corpus n'est pas implementee : ce morph n'interroge aucun index. Les citations renvoyees auparavant etaient inventees.".into())
}
