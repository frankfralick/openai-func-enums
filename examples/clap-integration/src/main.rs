use clap::{Parser, Subcommand, ValueEnum};
use openai_func_enums::{
    logger_task, rank_functions, single_embedding, CommandError, EnumDescriptor, FuncEmbedding,
    RunCommand, ToolCallExecutionStrategy, ToolSet, VariantDescriptors,
};
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;
use std::{fs::File, path::Path};
use tokio::spawn;
use tokio::sync::Mutex;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, ToolSet)]
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
        _arguments: Option<Vec<String>>,
        logger: Arc<Logger>,
    ) -> Result<
        (Option<String>, Option<Vec<String>>),
        Box<dyn std::error::Error + Send + Sync + 'static>,
    > {
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
                return Ok((Some(result.to_string()), None));
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
                return Ok((Some(result.to_string()), None));
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
                return Ok((Some(result.to_string()), None));
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
                    return Ok((Some(result.to_string()), None));
                } else {
                    return Err(Box::new(CommandError::new("Cannot divide by zero")));
                }
            }
            Commands::CallMultiStep { prompt_list } => {
                let _ = logger
                    .sender
                    .send(String::from("this is the prompt list"))
                    .await;
                let message = format!("{:#?}", prompt_list);
                let _ = logger.sender.send(message).await;

                let prior_result = Arc::new(Mutex::new(None));

                let command_args_list: Vec<String> = Vec::new();
                let command_args = Arc::new(Mutex::new(Some(command_args_list)));
                for (i, prompt) in prompt_list.iter().enumerate() {
                    let prior_result_clone = prior_result.clone();
                    let command_args_clone = command_args.clone();
                    let logger_clone = logger.clone();

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
                                command_args_clone,
                                None,
                                None,
                                logger_clone,
                            )
                            .await?
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
                                    command_args_clone,
                                    None,
                                    None,
                                    logger_clone,
                                )
                                .await?
                            } else {
                                *prior_result.lock().await = None;
                            }
                        }
                    }
                }
                let result = String::from("Ok.");
                return Ok((Some(result), None));
            }
            Commands::GPT { prompt } => {
                let prompt_embedding = single_embedding(prompt, FUNC_ENUMS_EMBED_MODEL).await?;

                let prior_result = Arc::new(Mutex::new(None));
                let command_args = Arc::new(Mutex::new(None));
                let embed_path = Path::new(FUNC_ENUMS_EMBED_PATH);
                let mut ranked_func_names = vec![];
                let logger_clone = logger.clone();

                if embed_path.exists() {
                    let mut file = File::open(embed_path).unwrap();
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes).unwrap();

                    let archived_funcs =
                        rkyv::check_archived_root::<Vec<FuncEmbedding>>(&bytes).unwrap();
                    ranked_func_names = rank_functions(archived_funcs, prompt_embedding).await;
                }

                let required_funcs = vec![String::from("CallMultiStep")];

                CommandsGPT::run(
                    prompt,
                    model_name,
                    request_token_limit,
                    max_response_tokens,
                    Some(system_message.to_string()),
                    prior_result,
                    execution_strategy.clone(),
                    command_args,
                    Some(ranked_func_names),
                    Some(required_funcs),
                    logger_clone,
                )
                .await?;
            }
        };

        Ok((None, None))
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
    let (sender, receiver) = mpsc::channel(100);
    let logger = Arc::new(Logger { sender });
    spawn(logger_task(receiver));
    let logger_clone = logger.clone();

    let cli = Cli::parse();

    let start_time = Instant::now();

    cli.command
        .run(ToolCallExecutionStrategy::Async, None, logger_clone)
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
