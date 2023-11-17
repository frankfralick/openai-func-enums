use async_openai::{
    types::{
        ChatCompletionFunctionCall, ChatCompletionNamedToolChoice,
        ChatCompletionRequestUserMessageArgs, ChatCompletionToolChoiceOption,
        ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionName,
    },
    Client,
};
use openai_func_enums::{
    arg_description, func_description, generate_enum_info, get_tool_chat_completion_args,
    parse_function_call, EnumDescriptor, FunctionCallResponse, VariantDescriptors,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    let tool_args = get_tool_chat_completion_args(GetCurrentWeatherResponse::get_function_json)?;

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(512u16)
        .model("gpt-4-1106-preview")
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content("What's the weather like in Swainsboro, GA, Nashville, TN, Los Angeles, CA?")
            .build()?
            .into()])
        .tools(tool_args.0)
        // Only one function call will be returned if tool_choice is passed.
        // .tool_choice(GetCurrentWeatherResponse::to_tool_choice())
        .build()?;

    let response_message = client
        .chat()
        .create(request)
        .await?
        .choices
        .get(0)
        .unwrap()
        .message
        .clone();

    if let Some(tool_calls) = response_message.tool_calls {
        println!("These are the tool calls returned:");
        println!("{:#?}", tool_calls);
        println!("");

        for tool_call in tool_calls.iter() {
            match tool_call.r#type {
                ChatCompletionToolType::Function => {
                    let current_weather_response =
                        parse_function_call!(tool_call.function, GetCurrentWeatherResponse);

                    if let Some(current_weather_response) = current_weather_response {
                        println!(
                            "Function called with location: {:#?}",
                            current_weather_response.location
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, FunctionCallResponse)]
pub enum FunctionDef {
    #[func_description(
        description = "Get the current weather in the location closest to the one provided location"
    )]
    GetCurrentWeather(Location, TemperatureUnits),
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
