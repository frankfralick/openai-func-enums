use async_openai::error::OpenAIError;
use async_openai::types::{ChatCompletionFunctions, ChatCompletionFunctionsArgs};
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

/// A trait to provide a description for a function argument.
/// This includes an optional description and the count of tokens in the description.
pub trait FunctionArgument {
    /// Returns the description of the function argument and the count of tokens in the description.
    ///
    /// # Returns
    ///
    /// A tuple where the first element is an `Option<String>` representing the description of the function argument,
    /// and the second element is a `usize` representing the count of tokens in the description.
    fn argument_description_with_token_count() -> (Option<String>, usize);
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
) -> Result<(ChatCompletionFunctions, usize), OpenAIError> {
    let (func_json, total_tokens) = func();

    let parameters = match func_json.get("parameters") {
        Some(parameters) => Some(parameters.clone()),
        None => None,
    };

    let description = func_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let name = func_json.get("name").unwrap().as_str().unwrap().to_string();
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
    Ok((chat_completion_args, total_tokens))
}
