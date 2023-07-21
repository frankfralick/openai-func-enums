# openai-func-enums:

openai-func-enums is a set of procedural macros and other functions, to be used in conjunction with async-openai, that make it easy to use enums to compose "functions" that can be passed to OpenAIs chat completions api. 

### Why?

The motivation for this was the need to leverage OpenAI function calls for logic control flow. If you have a lot of "function calls" to deal with, especially if they share argument types, the out-of-the-box way of doing this is unwieldy with async-openai. This library allows returns to be deserialized as instances of structs, the types of which the macros produce, so that you can easily take the response and match on the variants selected by the model.

## Features

- **Enums are the greatest:** openai-func-enums asks you to define an enum to represent possible "functions" to be passed to the OpenAI API, with each variant representing a "function, with the fields on these variants indicating the required arguments. Each field is an enum, with the variants of these fields determining the allowed choices that can be passed to the OpenAI API.

- **Token Tallying:** The library keeps a tally of the token count associated with each "function" defined through the enums. This would allow for precise control over the token limit if there was better documentation, but it should work in most cases. There is a limit on function descriptions that I can but haven't determined a value for. At some point I will put in guards for description length (the function description seems to make a big difference on performance where nuance exists).

## Usage

First, define an enum to hold the possible functions, with each variant being a function. The fields on these variants indicate the required arguments, and each field must also be an enum. The variants of these fields determine the allowed choices that can be passed to OpenAI's API. For example, here's a function definition for getting current weather:

```rust
#[derive(Debug, FunctionCallResponse)]
pub enum FunctionDef {
    #[func_description(
        description = "Get the current weather in the location closest to the one provided location",
        tokens = 14
    )]
    GetCurrentWeather(Location, TemperatureUnits),
}
```

Each argument must implement the `FunctionArgument` trait, which provides a description and token count for the argument. For example, a `Location` argument might look like this:

```rust
#[derive(Clone, Debug, Deserialize, EnumDescriptor, VariantDescriptors)]
pub enum Location {
    Atlanta,
    Boston,
    // ...
}

impl FunctionArgument for Location {
    fn argument_description_with_token_count() -> (Option<String>, usize) {
        (
            Some(String::from(" The only valid locations that can be passed")),
            9,
        )
    }
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

This way, you can provide more structured and strict interfaces for OpenAI's API.
