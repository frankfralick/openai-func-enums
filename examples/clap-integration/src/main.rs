use async_openai::{
    types::{
        ChatCompletionFunctionCall, ChatCompletionNamedToolChoice, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequestArgs,
        FunctionCall, FunctionName,
    },
    Client,
};
use async_trait::async_trait;
use clap::{Parser, Subcommand, ValueEnum};
use openai_func_enums::{
    arg_description, generate_enum_info, generate_value_arg_info, get_tool_chat_completion_args,
    CommandError, EnumDescriptor, RunCommand, SubcommandGPT, ToolCallExecutionStrategy,
    VariantDescriptors,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tiktoken_rs::cl100k_base;
use tokio::sync::Mutex;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, SubcommandGPT)]
pub enum Commands {
    /// Adds two numbers
    Add {
        a: f64,
        b: f64,
        rounding_mode: RoundingMode,
    },
    /// Subtracts two numbers
    Subtract {
        a: f64,
        b: f64,
        rounding_mode: RoundingMode,
    },
    /// Multiplies two numbers
    Multiply {
        a: f64,
        b: f64,
        rounding_mode: RoundingMode,
    },
    /// Divides two numbers
    Divide {
        a: f64,
        b: f64,
        rounding_mode: RoundingMode,
    },
    /// CallMultiStep is designed to efficiently process complex, multi-step user requests. It takes an array of text prompts, each detailing a specific step in a sequential task. This function is crucial for handling requests where the output of one step forms the input of the next. When constructing the prompt list, consider the dependency and order of tasks. Independent tasks within the same step should be consolidated into a single prompt to leverage parallel processing capabilities. This function ensures that multi-step tasks are executed in the correct sequence and that all dependencies are respected, thus faithfully representing and fulfilling the user's request."
    CallMultiStep {
        prompt_list: Vec<String>,
    },
    GPT {
        prompt: String,
    },
}

#[async_trait]
impl RunCommand for Commands {
    async fn run(
        &self,
        execution_strategy: ToolCallExecutionStrategy,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let max_response_tokens = 1000_u16;
        let request_token_limit = 4191;
        let model_name = "gpt-4-1106-preview";
        let system_message = "You are an advanced function-calling bot, adept at handling complex, \
                              multi-step user requests. Your role is to discern and articulate \
                              each step of a user's request, especially when it involves sequential \
                              operations. Use the CallMultiStep function for requests that require \
                              sequential processing. Each step should be described in a separate \
                              prompt, with attention to whether the steps are independent or \
                              interdependent. For interdependent steps, ensure each prompt \
                              accurately represents the sequence and dependencies of the tasks. \
                              Remember, a single step may encompass multiple tasks that can be \
                              executed in parallel. Your goal is to capture the entire scope of the \
                              user's request, structuring it into an appropriate sequence of function \
                              calls without omitting any steps. For example, if a user asks to add 8 \
                              and 2 in the first step, and then requests the result to be multiplied \
                              by 7 and 5 in separate tasks of the second step, use CallMultiStep with \
                              two prompts: the first for addition, and the second combining both \
                              multiplication tasks, recognizing their parallel nature.";

        println!();
        match self {
            Commands::Add {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a + b);
                println!(
                    "Result of adding {} and {} with rounding mode {:#?}: {}",
                    a,
                    b,
                    rounding_mode.variant_name_with_token_count().0,
                    result
                );
                return Ok(Some(result.to_string()));
            }
            Commands::Subtract {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a - b);
                println!(
                    "Result of subtracting {} from {} with rounding mode {:#?}: {}",
                    b,
                    a,
                    rounding_mode.variant_name_with_token_count().0,
                    result
                );
                return Ok(Some(result.to_string()));
            }
            Commands::Multiply {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a * b);
                println!(
                    "Result of multiplying {} and {} with rounding mode {:#?}: {}",
                    a,
                    b,
                    rounding_mode.variant_name_with_token_count().0,
                    result
                );
                return Ok(Some(result.to_string()));
            }
            Commands::Divide {
                a,
                b,
                rounding_mode,
            } => {
                if *b != 0.0 {
                    let result = rounding_mode.round(a / b);
                    println!(
                        "Result of dividing {} by {} with rounding mode {:#?}: {}",
                        a,
                        b,
                        rounding_mode.variant_name_with_token_count().0,
                        result
                    );
                    return Ok(Some(result.to_string()));
                } else {
                    return Err(Box::new(CommandError::new("Cannot divide by zero")));
                }
            }
            Commands::CallMultiStep { prompt_list } => {
                let prior_result = Arc::new(Mutex::new(None));
                for (i, prompt) in prompt_list.iter().enumerate() {
                    let prior_result_clone = prior_result.clone();

                    match i {
                        0 => {
                            CommandsGPT::run(
                                &prompt.to_string(),
                                model_name,
                                request_token_limit,
                                max_response_tokens,
                                Some(system_message.to_string()),
                                prior_result_clone,
                                execution_strategy.clone(),
                            )
                            .await?;
                        }

                        _ => {
                            let prior_result_guard = prior_result.lock().await;
                            if let Some(prior) = &*prior_result_guard {
                                let new_prompt =
                                    format!("The prior result was: {}. {}", prior.clone(), prompt);
                                drop(prior_result_guard);

                                CommandsGPT::run(
                                    &new_prompt,
                                    model_name,
                                    request_token_limit,
                                    max_response_tokens,
                                    Some(system_message.to_string()),
                                    prior_result_clone,
                                    execution_strategy.clone(),
                                )
                                .await?;
                            } else {
                                *prior_result.lock().await = None;
                            }
                        }
                    }
                }
                return Ok(None);
            }
            Commands::GPT { prompt } => {
                let prior_result = Arc::new(Mutex::new(None));
                CommandsGPT::run(
                    prompt,
                    model_name,
                    request_token_limit,
                    max_response_tokens,
                    Some(system_message.to_string()),
                    prior_result,
                    execution_strategy.clone(),
                )
                .await?;
            }
        };

        Ok(None)
    }
}

#[derive(Clone, Debug, Deserialize, EnumDescriptor, VariantDescriptors, ValueEnum)]
#[arg_description(description = "Different modes to round a number.")]
pub enum RoundingMode {
    NoRounding,
    Nearest,
    Zero,
    Up,
    Down,
}

impl RoundingMode {
    pub fn round(&self, number: f64) -> f64 {
        match *self {
            RoundingMode::NoRounding => number,
            RoundingMode::Nearest => number.round(),
            RoundingMode::Zero => number.trunc(),
            RoundingMode::Up => number.ceil(),
            RoundingMode::Down => number.floor(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let start_time = Instant::now();

    cli.command
        .run(ToolCallExecutionStrategy::Async)
        .await
        .map_err(|e| {
            Box::new(CommandError::new(&format!(
                "Command failed with error: {}",
                e
            )))
        })?;

    let duration = start_time.elapsed();
    println!("Command completed in {:.2} seconds", duration.as_secs_f64());

    Ok(())
}
