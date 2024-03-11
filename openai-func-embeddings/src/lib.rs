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

/// Asynchronously generates a single embedding vector for the given text using a specified model.
///
/// This function creates an embedding for the input text by calling an external service (e.g., OpenAI's
/// API) with the specified model. It returns the embedding vector as a `Vec<f32>`.
///
/// # Parameters
/// - `text`: A reference to a `String` containing the text to be embedded.
/// - `model`: A string slice (`&str`) specifying the model to use for generating the embedding.
///
/// # Returns
/// A `Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>>`:
/// - `Ok(Vec<f32>)` containing the embedding vector if the operation is successful.
/// - `Err(Box<dyn std::error::Error + Send + Sync>)` if there is an error during the operation,
///     including issues with creating the request, network errors, or if the response does not contain an embedding.
///
/// # Errors
/// This function can return an error in several cases, including:
/// - Failure to build the embedding request.
/// - Network or API errors when contacting the external service.
/// - The response from the external service does not include an embedding vector.
///
/// # Example
/// ```rust
/// use std::path::Path;
/// use your_module::{single_embedding, FuncEnumsError, Client, CreateEmbeddingRequestArgs};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let text = String::from("Your sample text here");
///     let model = "your-model-name";
///     
///     let embedding = single_embedding(&text, model).await?;
///     println!("Embedding vector: {:?}", embedding);
///
///     Ok(())
/// }
/// ```
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

/// Asynchronously retrieves and ranks function names based on their similarity to a given prompt embedding.
///
/// This function searches a specified file for function embeddings, compares them to the provided prompt embedding, and returns a ranked list of function names based on their similarity to the prompt.
///
/// # Parameters
/// - `prompt_embedding`: A `Vec<f32>` representing the embedding of the prompt. This embedding is used to compare against the function embeddings stored in the file located at `embed_path`.
/// - `embed_path`: A reference to a `Path` where the function embeddings are stored. This file should contain a serialized `Vec<FuncEmbedding>` where `FuncEmbedding` is a structure representing the function name and its embedding.
///
/// # Returns
/// - `Ok(Vec<String>)`: A vector of function names ranked by their similarity to the `prompt_embedding`. The most similar function's name is first.
/// - `Err(Box<dyn std::error::Error + Send + Sync>)`: An error if the file at `embed_path` cannot be opened, read, or if the embeddings cannot be deserialized and compared successfully.
///
/// # Errors
/// - File opening failure due to `embed_path` not existing or being inaccessible.
/// - File reading failure if the file cannot be read to the end.
/// - Archive processing failure if deserialization of the stored embeddings encounters errors.
///
/// # Examples
/// ```
/// async fn run() -> Result<(), Box<dyn std::error::Error>> {
///     let prompt_embedding = vec![0.1, 0.2, 0.3];
///     let embed_path = Path::new("function_embeddings.bin");
///     let ranked_function_names = get_ranked_function_names(prompt_embedding, embed_path).await?;
///     println!("Ranked functions: {:?}", ranked_function_names);
///     Ok(())
/// }
/// ```
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

        // TODO: Would be nice to check how much faster unsafe version of this is.
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
