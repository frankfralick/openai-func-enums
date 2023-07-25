use async_openai::{
    types::{
        ChatCompletionFunctionCall, ChatCompletionRequestMessageArgs,
        CreateChatCompletionRequestArgs, FunctionCall, Role,
    },
    Client,
};
use clap::{Parser, Subcommand, ValueEnum};
use openai_func_enums::{
    generate_value_arg_info, get_function_chat_completion_args, SubcommandGPT,
};
use serde_json::{json, Value};
use std::error::Error;

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
    },
    /// Subtracts two numbers
    Subtract {
        a: f64,
        b: f64,
    },
    /// Multiplies two numbers
    Multiply {
        a: f64,
        b: f64,
    },
    /// Divides two numbers
    Divide {
        a: f64,
        b: f64,
    },
    GPT {
        a: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { a, b } => {
            println!("Result: {}", a + b);
        }
        Commands::Subtract { a, b } => println!("Result: {}", a - b),
        Commands::Multiply { a, b } => println!("Result: {}", a * b),
        Commands::Divide { a, b } => {
            if b != 0.0 {
                println!("Result: {}", a / b)
            } else {
                panic!("Cannot divide by zero");
            }
        }
        Commands::GPT { a } => {
            let function_args = get_function_chat_completion_args(CommandsGPT::all_function_jsons)?;
            let request = CreateChatCompletionRequestArgs::default()
                .max_tokens(512u16)
                .model("gpt-4-0613")
                .messages([ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(a)
                    .build()?])
                .functions(function_args.0)
                .function_call("auto")
                .build()?;

            let client = Client::new();
            let response_message = client
                .chat()
                .create(request)
                .await?
                .choices
                .get(0)
                .unwrap()
                .message
                .clone();

            // println!("This is the response message returned:");
            // println!("{:#?}", response_message);

            if let Some(function_call) = response_message.function_call {
                match CommandsGPT::parse_gpt_function_call(&function_call) {
                    Ok(FunctionResponse::AddResponse(response)) => {
                        let result = response.execute_command();
                        match result {
                            Commands::Add { a, b } => {
                                println!("Result: {}", a + b);
                            }
                            _ => {}
                        }
                    }
                    Ok(FunctionResponse::SubtractResponse(response)) => {
                        let result = response.execute_command();
                        match result {
                            Commands::Subtract { a, b } => {
                                println!("Result: {}", a - b);
                            }
                            _ => {}
                        }
                    }
                    Ok(FunctionResponse::DivideResponse(response)) => {
                        let result = response.execute_command();
                        println!("Result: {:#?}", result);
                        match result {
                            Commands::Divide { a, b } => {
                                if b != 0.0 {
                                    println!("Result: {}", a / b)
                                } else {
                                    panic!("Cannot divide by zero");
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(FunctionResponse::MultiplyResponse(response)) => {
                        let result = response.execute_command();
                        println!("Result: {:#?}", result);
                        match result {
                            Commands::Multiply { a, b } => {
                                println!("Result: {}", a * b);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        println!("There was an error:  {:#?}", e)
                    }
                    _ => {}
                }
            }
        }
    };
    Ok(())
}
