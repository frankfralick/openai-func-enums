use async_openai::error::OpenAIError;
use async_openai::types::{
    ChatCompletionTool, ChatCompletionToolArgs, ChatCompletionToolType, FunctionObject,
    FunctionObjectArgs,
};
use async_trait::async_trait;
pub use openai_func_embeddings::*;
pub use openai_func_enums_macros::*;
use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Debug};
use std::sync::Arc;
use tokio::sync::mpsc;

/// A trait to provide a descriptor for an enumeration.
/// This includes the name of the enum and the count of tokens in its name.
pub trait EnumDescriptor {
    /// Returns the name of the enum and the count of tokens in its name.
    ///
    /// # Returns
    ///
    /// A tuple where the first element is a `String` representing the name of the enum,
    /// and the second element is a `usize` representing the count of tokens in the enum's name.
    fn name_with_token_count() -> &'static (&'static str, usize);

    fn arg_description_with_token_count() -> &'static (&'static str, usize);
}

pub trait ToolSet {}

/// A trait to provide descriptors for the variants of an enumeration.
/// This includes the names of the variants and the count of tokens in their names.
pub trait VariantDescriptors {
    /// Returns the names of the variants of the enum and the count of tokens in each variant's name.
    ///
    /// # Returns
    ///
    /// A `Vec` of tuples where each tuple's first element is a `String` representing the name of a variant,
    /// and the second element is a `usize` representing the count of tokens in the variant's name.
    fn variant_names_with_token_counts(
    ) -> &'static (&'static [&'static str], &'static [usize], usize, usize);

    /// Returns the name of a variant and the count of tokens in its name.
    ///
    /// # Returns
    ///
    /// A tuple where the first element is a `String` representing the name of the variant,
    /// and the second element is a `usize` representing the count of tokens in the variant's name.
    fn variant_name_with_token_count(&self) -> (&'static str, usize);
}

#[derive(Clone, Debug)]
pub enum ToolCallExecutionStrategy {
    Parallel,
    Async,
    Synchronous,
}

#[derive(Debug)]
pub struct CommandError {
    details: String,
}

impl CommandError {
    pub fn new(msg: &str) -> CommandError {
        CommandError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for CommandError {}

impl From<OpenAIError> for CommandError {
    fn from(error: OpenAIError) -> Self {
        CommandError::new(&format!("OpenAI Error: {}", error))
    }
}

pub struct Logger {
    pub sender: mpsc::Sender<String>,
}

impl Logger {
    pub async fn log(&self, message: String) {
        let _ = self.sender.send(message).await;
    }
}

pub async fn logger_task(mut receiver: mpsc::Receiver<String>) {
    while let Some(message) = receiver.recv().await {
        println!("{}", message);
    }
}

// There is a better way than to keep adding return types.
// Trying to determine which road to go down on other issues first.
#[async_trait]
pub trait RunCommand: Sync + Send {
    async fn run(
        &self,
        execution_strategy: ToolCallExecutionStrategy,
        arguments: Option<Vec<String>>,
        logger: Arc<Logger>,
        system_message: Option<(String, usize)>,
    ) -> Result<
        (Option<String>, Option<Vec<String>>),
        Box<dyn std::error::Error + Send + Sync + 'static>,
    >;
}

/// A macro to parse a function call into a specified type.
/// If the parsing fails, it prints an error message and returns `None`.
///
/// # Arguments
///
/// * `$func_call` - An expression representing the function call to parse.
/// * `$type` - The target type to parse the function call into.
#[macro_export]
macro_rules! parse_function_call {
    ($func_call:expr, $type:ty) => {
        match serde_json::from_str::<$type>($func_call.arguments.as_str()) {
            Ok(response) => Some(response),
            Err(e) => {
                println!("Failed to parse function call: {}", e);
                None
            }
        }
    };
}

/// A function to get the chat completion arguments for a function.
///
/// # Arguments
///
/// * `func` - A function that returns a JSON representation of a function and the count of tokens in the representation.
///
/// # Returns
///
/// * A `Result` which is `Ok` if the function chat completion arguments were successfully obtained, and `Err` otherwise.
///   The `Ok` variant contains a tuple where the first element is a `ChatCompletionFunctions` representing the chat completion arguments for the function,
///   and the second element is a `usize` representing the total count of tokens in the function's JSON representation.
pub fn get_function_chat_completion_args(
    func: impl Fn() -> (Value, usize),
) -> Result<(Vec<FunctionObject>, usize), OpenAIError> {
    let (func_json, total_tokens) = func();

    let mut chat_completion_functions_vec = Vec::new();

    let values = match func_json {
        Value::Object(_) => vec![func_json],
        Value::Array(arr) => arr,
        _ => {
            return Err(OpenAIError::InvalidArgument(String::from(
                "Something went wrong parsing the json",
            )))
        }
    };

    for value in values {
        let parameters = value.get("parameters").cloned();

        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = value.get("name").unwrap().as_str().unwrap().to_string();
        let chat_completion_args = match description {
            Some(desc) => FunctionObjectArgs::default()
                .name(name)
                .description(desc)
                .parameters(parameters)
                .build()?,
            None => FunctionObjectArgs::default()
                .name(name)
                .parameters(parameters)
                .build()?,
        };
        chat_completion_functions_vec.push(chat_completion_args);
    }

    Ok((chat_completion_functions_vec, total_tokens))
}

/// A function to get the chat completion arguments for a tool.
///
/// # Arguments
///
/// * `tool_func` - A function that returns a JSON representation of a tool and the count of tokens in the representation.
///
/// # Returns
///
/// * A `Result` which is `Ok` if the tool chat completion arguments were successfully obtained, and `Err` otherwise.
///   The `Ok` variant contains a tuple where the first element is a `ChatCompletionTool` representing the chat completion arguments for the tool,
///   and the second element is a `usize` representing the total count of tokens in the tool's JSON representation.
pub fn get_tool_chat_completion_args(
    tool_func: impl Fn() -> (Value, usize),
) -> Result<(Vec<ChatCompletionTool>, usize), OpenAIError> {
    let (tool_json, total_tokens) = tool_func();

    let mut chat_completion_tool_vec = Vec::new();

    let values = match tool_json {
        Value::Object(_) => vec![tool_json],
        Value::Array(arr) => arr,
        _ => {
            return Err(OpenAIError::InvalidArgument(String::from(
                "Something went wrong parsing the json",
            )))
        }
    };

    for value in values {
        let parameters = value.get("parameters").cloned();

        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = value.get("name").unwrap().as_str().unwrap().to_string();

        if name != "GPT" {
            let chat_completion_functions_args = match description {
                Some(desc) => FunctionObjectArgs::default()
                    .name(name)
                    .description(desc)
                    .parameters(parameters)
                    .build()?,
                None => FunctionObjectArgs::default()
                    .name(name)
                    .parameters(parameters)
                    .build()?,
            };

            let chat_completion_tool = ChatCompletionToolArgs::default()
                .r#type(ChatCompletionToolType::Function)
                .function(chat_completion_functions_args)
                .build()?;

            chat_completion_tool_vec.push(chat_completion_tool);
        }
    }

    Ok((chat_completion_tool_vec, total_tokens))
}

/// This function will get called if an "allowed_functions" argument is passed to the
/// run function. If it is passed, then the presense or absence of the function_filtering
/// feature flag will dictate what happens. If function_filtering is on, then the required
/// functions (if some) will get included, then your ranked functions will get added until the
/// token limit is reached. Without function_filtering feature enabled, all functions listed in
/// allowed_func_names and required_func_names will get sent.

/// Performs selective inclusion of tools based on the provided `allowed_func_names` and the state
/// of the `function_filtering` feature flag. When `function_filtering` is enabled and `required_func_names`
/// is specified, required functions are prioritized, followed by ranked functions until a token limit is reached.
/// Without the `function_filtering` feature, all functions in `allowed_func_names` and `required_func_names`
/// are included, irrespective of the token limit.
///
/// # Arguments
/// - `tool_func`: A function that takes allowed and required function names, and returns tool JSON and total token count.
/// - `allowed_func_names`: A list of function names allowed for inclusion.
/// - `required_func_names`: An optional list of function names required for inclusion.
///
/// # Returns
/// A result containing a vector of `ChatCompletionTool` objects and the total token count, or an `OpenAIError` on failure.
///
/// # Errors
/// Returns an `OpenAIError::InvalidArgument` if there's an issue parsing the tool JSON.
pub fn get_tools_limited(
    tool_func: impl Fn(Vec<String>, Option<Vec<String>>) -> (Value, usize),
    allowed_func_names: Vec<String>,
    required_func_names: Option<Vec<String>>,
) -> Result<(Vec<ChatCompletionTool>, usize), OpenAIError> {
    let (tool_json, total_tokens) = tool_func(allowed_func_names, required_func_names);

    let mut chat_completion_tool_vec = Vec::new();

    let values = match tool_json {
        Value::Object(_) => vec![tool_json],
        Value::Array(arr) => arr,
        _ => {
            return Err(OpenAIError::InvalidArgument(String::from(
                "Something went wrong parsing the json",
            )))
        }
    };

    for value in values {
        let parameters = value.get("parameters").cloned();

        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = value.get("name").unwrap().as_str().unwrap().to_string();

        if name != "GPT" {
            let chat_completion_functions_args = match description {
                Some(desc) => FunctionObjectArgs::default()
                    .name(name)
                    .description(desc)
                    .parameters(parameters)
                    .build()?,
                None => FunctionObjectArgs::default()
                    .name(name)
                    .parameters(parameters)
                    .build()?,
            };

            let chat_completion_tool = ChatCompletionToolArgs::default()
                .r#type(ChatCompletionToolType::Function)
                .function(chat_completion_functions_args)
                .build()?;

            chat_completion_tool_vec.push(chat_completion_tool);
        }
    }

    Ok((chat_completion_tool_vec, total_tokens))
}
