# openai-func-enums:

openai-func-enums is a set of procedural macros and other functions, to be used in conjunction with async-openai, that make it easy to use enums to compose "functions" that can be passed to OpenAIs chat completions api. 

### Why?

The motivation for this was the need to leverage OpenAI function calls for logic control flow. If you have a lot of "function calls" to deal with, especially if they share argument types, the out-of-the-box way of doing this is unwieldy with async-openai. This library allows returns to be deserialized as instances of structs, the types of which the macros produce, so that you can easily take the response and match on the variants selected by the model.

## Features

- **Enums are the greatest:** openai-func-enums asks you to define an enum to represent possible "functions" to be passed to the OpenAI API, with each variant representing a function, with the fields on these variants indicating the required arguments. Each field is an enum, with the variants of these fields determining the allowed choices that can be passed to the OpenAI API.

- **Token Tallying:** The library keeps a tally of the token count associated with each "function" defined through the enums. This would allow for precise control over the token limit if there was better documentation, but it should work in most cases. There is a limit on function descriptions that I can but haven't determined a value for. At some point I will put in guards for description length (the function description seems to make a big difference on performance where nuance exists).

- **clap-gpt:** This library provides macros and traits to allow you to turn an existing clap application into a clap-gpt application without a ton of extra ceremony required. See the usage section for an example.

## Usage

First, define an enum to hold the possible functions, with each variant being a function. The fields on these variants indicate the required arguments, and each field must also be an enum. The variants of these fields determine the allowed choices that can be passed to OpenAI's API. For example, here's a function definition for getting current weather:

```rust
#[derive(Debug, FunctionCallResponse)]
pub enum FunctionDef {
    #[func_description(
        description = "Get the current weather in the location closest to the one provided location"
    )]
    GetCurrentWeather(Location, TemperatureUnits),
}
```

Each argument must derive EnumDescriptor and VariantDescriptor, and must have the attribute macro arg_description. For example, a `Location` argument might look like this:

```rust
#[derive(Clone, Debug, Deserialize, EnumDescriptor, VariantDescriptors)]
#[arg_description(description = "The only valid locations that can be passed.")]
pub enum Location {
    Atlanta,
    Boston,
    // ...
}
```

Then, you can use these definitions to construct a request to the OpenAI API. The thing to note here is that the user prompt asks about the weather at the center of the universe, Swainsboro, GA, which isn't a variant we are giving it:

```rust
let function_args =
    get_function_chat_completion_args(GetCurrentWeatherResponse::get_function_json)?;
let request = CreateChatCompletionRequestArgs::default()
    .max_tokens(512u16)
    .model("gpt-4-0613")
    .messages([ChatCompletionRequestMessageArgs::default()
        .role(Role::User)
        .content("What's the weather like in Swainsboro, Georgia?")
        .build()?])
    .functions(vec![function_args.0])
    .function_call(GetCurrentWeatherResponse::to_function_call())
    .build()?;
```

This creates a request with the `GetCurrentWeather` function, and two arguments: `Location` and `TemperatureUnits`.

After sending the chat request, you can use `parse_function_call!` macro to parse the function call response into an instance of GetCurrentWeatherResponse, which is a struct type that the FunctionCallResponse derive macro generates. The properties of this struct type will correspond to the argument type enums. In this example GetCurrentWeatherResponse will have properties location: Location, and temperature_units: TemperatureUnits. Once you have this you can match on the variants and be on your way:

```rust
    let response_message = client
        .chat()
        .create(request)
        .await?
        .choices
        .get(0)
        .unwrap()
        .message
        .clone();

    if let Some(function_call) = response_message.function_call {
        println!("This is the function call returned:");
        println!("{:#?}", function_call);

        let current_weather_response =
            parse_function_call!(function_call, GetCurrentWeatherResponse);

        if let Some(current_weather_response) = current_weather_response {
            match current_weather_response.location {
                Location::Atlanta => {
                    println!("Function called with location: Atlanta");
                }
                _ => {
                    println!("Function call with a location other than Atlanta.");
                }
            }
        }
    }
```

### Integration with clap:
Depending on how your existing clap application is structured, this library can provide an easy mechanism to allow use of your command line tool with natural language instructions. It supports value type arguments and enums. How well it performs will depend on which model you use, the system messages, and function descriptions. A word of caution: this example demonstrates how to have one instruction call multiple commands in order. That involves the model planning what to do, and if your instructions include some step that your commands don't cover, it can run away (for now). Despite aggressive system cards warning it to only ever call functions that exist, sometimes it returns make-me-ups.


If your application follows the pattern where you have an enum that derives clap's ```Subcommand```, you will also want to derive ```SubcommandGPT```. Additionally, you will want to add a new magical variant to handle the natural language commands. In this example it is the "GPT" variant. Note that I don't give it a description, and you do want to omit it. There's another variant in this example that isn't necessary to have, "CallMultiStep", that is there just to demonstrate doing multiple steps at once.

```rust
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
```

The library provides a trait called "RunCommand", which makes you implement a "run" function. This function returns a result of Option<String>, and this is only for cases where you have more than one step.  In this example I'm showing how you can have value type arguments, as well as enums. If you want to define an enum that will serve as an argument to function calls, they need to derive clap's ```ValueEnum```, as well as the other ```EnumDescriptor``` and ```VariantDescriptors``` provided by this library. 

![Clap Example](./openai-func-enums/assets/clap_example.png)


```rust
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
```
