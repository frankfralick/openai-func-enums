use openai_func_enums::{
    arg_description, logger_task, CommandError, EnumDescriptor, Logger, RunCommand,
    ToolCallExecutionStrategy, ToolSet, VariantDescriptors,
};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(100);
    let logger = Arc::new(Logger { sender });
    spawn(logger_task(receiver));
    let logger_clone = logger.clone();

    let system_message = Some((String::from("You are an advanced function-calling bot."), 9));

    (FunctionDef::GPT {
        prompt: "What's the weather like in Swainsboro, GA, Nashville, TN, Los Angeles, CA?"
            .to_string(),
    })
    .run(
        ToolCallExecutionStrategy::Async,
        None,
        logger_clone,
        system_message,
    )
    .await
    .map_err(|e| {
        Box::new(CommandError::new(&format!(
            "Command failed with error: {}",
            e
        )))
    })?;

    Ok(())
}

#[derive(Debug, ToolSet)]
pub enum FunctionDef {
    /// "Get the current weather in the location closest to the one provided location"
    GetCurrentWeather {
        location: Location,
        temperature_units: TemperatureUnits,
    },

    GPT {
        prompt: String,
    },
}

#[async_trait]
impl RunCommand for FunctionDef {
    async fn run(
        &self,
        execution_strategy: ToolCallExecutionStrategy,
        _arguments: Option<Vec<String>>,
        logger: Arc<Logger>,
        system_message: Option<(String, usize)>,
    ) -> Result<
        (Option<String>, Option<Vec<String>>),
        Box<dyn std::error::Error + Send + Sync + 'static>,
    > {
        // These two arguments are optional. If None is passed, the
        // values are taken from environment variables. Look at
        // the build.rs file for this example.
        let max_response_tokens = 1000_u16;
        let request_token_limit = 4191;

        let model_name = "gpt-4-1106-preview";

        match self {
            FunctionDef::GetCurrentWeather {
                location,
                temperature_units,
            } => {
                println!("Called GetCurrentWeather function with argument:");
                println!("{:#?}", location);
                println!("{:#?}", temperature_units);
            }
            FunctionDef::GPT { prompt } => {
                let prior_result = Arc::new(Mutex::new(None));
                let command_args = Arc::new(Mutex::new(None));
                let logger_clone = logger.clone();

                // If you want to see an example of limiting presentation of function calls based
                // on a token budget, look at the clap integration example.
                CommandsGPT::run(
                    prompt,
                    model_name,
                    Some(request_token_limit),
                    Some(max_response_tokens),
                    system_message,
                    prior_result,
                    execution_strategy.clone(),
                    command_args,
                    None,
                    None,
                    logger_clone,
                )
                .await?;
            }
        }

        Ok((None, None))
    }
}

#[derive(Clone, Debug, Deserialize, EnumDescriptor, VariantDescriptors)]
#[arg_description(description = "The only valid locations that can be passed.")]
pub enum Location {
    Atlanta,
    Boston,
    Chicago,
    Dallas,
    Denver,
    LosAngeles,
    Miami,
    Nashville,
    NewYork,
    Philadelphia,
    Seattle,
    StLouis,
    Washington,
}

#[derive(Clone, Debug, Deserialize, EnumDescriptor, VariantDescriptors)]
#[arg_description(description = "A temperature unit chosen from the enum.")]
pub enum TemperatureUnits {
    Celcius,
    Fahrenheit,
}
