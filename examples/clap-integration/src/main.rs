use async_openai::{
    types::{
        ChatCompletionFunctionCall, ChatCompletionRequestMessageArgs,
        CreateChatCompletionRequestArgs, FunctionCall, Role,
    },
    Client,
};
use async_trait::async_trait;
use clap::{Parser, Subcommand, ValueEnum};
use openai_func_enums::{
    arg_description, generate_enum_info, generate_value_arg_info,
    get_function_chat_completion_args, CommandError, EnumDescriptor, RunCommand, SubcommandGPT,
    VariantDescriptors,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tiktoken_rs::cl100k_base;

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
    /// A string with plain english prompts on separate lines describing the steps to accomplish a multistep request.
    CallMultiStep {
        prompt_list: String,
    },
    GPT {
        prompt: String,
    },
}

#[async_trait]
impl RunCommand for Commands {
    async fn run(
        &self,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let max_response_tokens = 250_u16;
        let request_token_limit = 4191;
        let model_name = "gpt-4-0613";
        let system_message = "You are a helpful function-calling bot. If the user prompt \
                              involves multiple steps, use the CallMultiStep function with \
                              a new-line-separated string with each line descripbing a step.";

        match self {
            Commands::Add {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a + b);
                println!("Result: {}", result);
                return Ok(Some(result.to_string()));
            }
            Commands::Subtract {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a - b);
                println!("Result: {}", result);
                return Ok(Some(result.to_string()));
            }
            Commands::Multiply {
                a,
                b,
                rounding_mode,
            } => {
                let result = rounding_mode.round(a * b);
                println!("Result: {}", result);
                return Ok(Some(result.to_string()));
            }
            Commands::Divide {
                a,
                b,
                rounding_mode,
            } => {
                if *b != 0.0 {
                    let result = rounding_mode.round(a / b);
                    println!("Result: {}", result);
                    return Ok(Some(result.to_string()));
                } else {
                    return Err(Box::new(CommandError::new("Cannot divide by zero")));
                }
            }
            Commands::CallMultiStep { prompt_list } => {
                let prompts: Vec<_> = prompt_list.split('\n').collect();

                let mut prior_result: Option<String> = None;
                for (i, prompt) in prompts.iter().enumerate() {
                    match i {
                        0 => {
                            println!("This is the first step: {}", prompt);
                            prior_result = CommandsGPT::run(
                                &prompt.to_string(),
                                model_name,
                                request_token_limit,
                                max_response_tokens,
                                Some(system_message.to_string()),
                            )
                            .await?
                        }

                        _ => {
                            if i == prompts.len() - 1 {
                                println!("This is the last prompt: {}", prompt);
                            } else {
                                println!("This is the next prompt: {}", prompt);
                            }
                            if let Some(prior) = prior_result {
                                let new_prompt =
                                    format!("The prior result was: {}. {}", prior, prompt);
                                prior_result = CommandsGPT::run(
                                    &new_prompt,
                                    model_name,
                                    request_token_limit,
                                    max_response_tokens,
                                    Some(system_message.to_string()),
                                )
                                .await?
                            } else {
                                prior_result = None;
                            }
                        }
                    }
                }
            }
            Commands::GPT { prompt } => {
                CommandsGPT::run(
                    prompt,
                    model_name,
                    request_token_limit,
                    max_response_tokens,
                    Some(system_message.to_string()),
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

    cli.command.run().await.map_err(|e| {
        Box::new(CommandError::new(&format!(
            "Command failed with error: {}",
            e
        )))
    })?;

    Ok(())
}
