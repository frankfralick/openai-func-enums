use async_openai::{types::CreateEmbeddingRequestArgs, Client};
use rkyv::{vec::ArchivedVec, Archive, Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Archive, Deserialize, Serialize)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug))]
pub struct FuncEmbedding {
    pub name: String,
    pub description: String,
    pub embedding: Vec<f32>,
}

pub async fn single_embedding(
    text: &String,
    model: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let request = CreateEmbeddingRequestArgs::default()
        .model(model)
        .input([text])
        .build()?;

    let response = client.embeddings().create(request).await?;

    match response.data.first() {
        Some(data) => Ok(data.embedding.to_owned()),
        None => {
            let embedding_error =
                FuncEnumsError::OpenAIError(String::from("Didn't get embedding vector back."));
            let boxed_error: Box<dyn std::error::Error + Send + Sync> = Box::new(embedding_error);
            Err(boxed_error)
        }
    }
}

pub fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> f32 {
    let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(&x1, &x2)| x1 * x2).sum();
    let magnitude1: f32 = vec1.iter().map(|&x| x.powf(2.0)).sum::<f32>().sqrt();
    let magnitude2: f32 = vec2.iter().map(|&x| x.powf(2.0)).sum::<f32>().sqrt();

    if magnitude1 == 0.0 || magnitude2 == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude1 * magnitude2)
}

pub async fn rank_functions(
    archived_embeddings: &ArchivedVec<ArchivedFuncEmbedding>,
    input_vector: Vec<f32>,
) -> Vec<String> {
    let mut name_similarity_pairs: Vec<(String, f32)> = archived_embeddings
        .iter()
        .map(|archived_embedding| {
            let archived_embedding_vec: &ArchivedVec<f32> = &archived_embedding.embedding;
            let similarity = cosine_similarity(archived_embedding_vec, &input_vector);
            (archived_embedding.name.to_string(), similarity)
        })
        .collect();

    name_similarity_pairs
        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    name_similarity_pairs
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

pub async fn get_ranked_function_names(
    prompt_embedding: Vec<f32>,
    embed_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    if embed_path.exists() {
        let mut file = match File::open(embed_path) {
            Ok(f) => f,
            Err(e) => return Err(Box::new(e)),
        };
        let mut bytes = Vec::new();
        if let Err(e) = file.read_to_end(&mut bytes) {
            return Err(Box::new(e));
        }

        let archived_funcs =
            rkyv::check_archived_root::<Vec<FuncEmbedding>>(&bytes).map_err(|e| {
                Box::new(FuncEnumsError::RkyvError(format!(
                    "Archive processing failed: {}",
                    e
                ))) as Box<dyn std::error::Error + Send + Sync>
            })?;

        Ok(rank_functions(archived_funcs, prompt_embedding).await)
    } else {
        Ok(vec![])
    }
}

#[derive(Debug)]
pub enum FuncEnumsError {
    OpenAIError(String),
    RkyvError(String),
}

impl std::fmt::Display for FuncEnumsError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for FuncEnumsError {}
