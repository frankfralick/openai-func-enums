use std::error::Error;
use std::fmt::{self, Debug};

use async_openai::error::OpenAIError;
use async_openai::types::{ChatCompletionFunctions, ChatCompletionFunctionsArgs};
use async_trait::async_trait;
pub use openai_func_enums_macros::*;
use serde_json::Value;

/// A trait to provide a descriptor for an enumeration.
/// This includes the name of the enum and the count of tokens in its name.
pub trait EnumDescriptor {
    /// Returns the name of the enum and the count of tokens in its name.
    ///
    /// # Returns
    ///
    /// A tuple where the first element is a `String` representing the name of the enum,
    /// and the second element is a `usize` representing the count of tokens in the enum's name.
    fn name_with_token_count() -> (String, usize);
    fn arg_description_with_token_count() -> (String, usize);
}

pub trait SubcommandGPT {
    // fn name_with_token_count() -> (String, usize);
    // fn arg_description_with_token_count() -> (String, usize);
}

/// A trait to provide descriptors for the variants of an enumeration.
/// This includes the names of the variants and the count of tokens in their names.
pub trait VariantDescriptors {
    /// Returns the names of the variants of the enum and the count of tokens in each variant's name.
    ///
    /// # Returns
    ///
    /// A `Vec` of tuples where each tuple's first element is a `String` representing the name of a variant,
    /// and the second element is a `usize` representing the count of tokens in the variant's name.
    fn variant_names_with_token_counts() -> Vec<(String, usize)>;

    /// Returns the name of a variant and the count of tokens in its name.
    ///
    /// # Returns
    ///
    /// A tuple where the first element is a `String` representing the name of the variant,
    /// and the second element is a `usize` representing the count of tokens in the variant's name.
    fn variant_name_with_token_count(&self) -> (String, usize);
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
        CommandError::new(&format!("OpenAI Error: {}", error.to_string()))
    }
}

#[async_trait]
pub trait RunCommand: Sync + Send {
    async fn run(
        &self,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync + 'static>>;
}

/// A trait for responses from function calls.
/// This includes a method to generate a JSON representation of the function.
pub trait FunctionCallResponse {
    /// Returns a JSON representation of the function and the count of tokens in the representation.
    ///
    /// # Returns
    ///
    /// A `Vec` of tuples where each tuple's first element is a `Value` representing a JSON object of the function,
    /// and the second element is a `usize` representing the count of tokens in the function's JSON representation.
    fn get_function_json(&self) -> Vec<(Value, usize)>;
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
                eprintln!("Failed to parse function call: {}", e);
                None
            }
        }
    };
}
// #[macro_export]
// macro_rules! parse_function_call {
//     ($func_call:expr) => {
//         match serde_json::from_str::<$type>($func_call.arguments.as_str()) {
//             Ok(response) => Some(response),
//             Err(e) => {
//                 eprintln!("Failed to parse function call: {}", e);
//                 None
//             }
//         }
//     };
// }

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
) -> Result<(Vec<ChatCompletionFunctions>, usize), OpenAIError> {
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
        let parameters = match value.get("parameters") {
            Some(parameters) => Some(parameters.clone()),
            None => None,
        };

        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = value.get("name").unwrap().as_str().unwrap().to_string();
        let chat_completion_args = match description {
            Some(desc) => ChatCompletionFunctionsArgs::default()
                .name(name)
                .description(desc)
                .parameters(parameters)
                .build()?,
            None => ChatCompletionFunctionsArgs::default()
                .name(name)
                .parameters(parameters)
                .build()?,
        };
        chat_completion_functions_vec.push(chat_completion_args);
    }

    Ok((chat_completion_functions_vec, total_tokens))
}
