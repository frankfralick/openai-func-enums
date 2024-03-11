use proc_macro::{TokenStream, TokenTree};
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Expr, Ident, Lit, Meta};

#[cfg(any(
    feature = "compile_embeddings_all",
    feature = "compile_embeddings_update"
))]
use async_openai::{types::CreateEmbeddingRequestArgs, Client};

#[cfg(any(
    feature = "compile_embeddings_all",
    feature = "compile_embeddings_update"
))]
use std::io::Write;

/// The `arg_description` attribute is a procedural macro used to provide additional description for an enum.
///
/// This attribute does not modify the code it annotates but instead attaches metadata in the form of a description.
/// This isn't required. It will be passed as a description if present, which can result in better
/// argument selection.
///
/// # Usage
///
/// ```rust
/// #[arg_description(description = "This is a sample enum.", tokens = 5)]
/// #[derive(EnumDescriptor)]
/// pub enum SampleEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// Note: The actual usage of the description and tokens provided through this attribute happens
/// in the `EnumDescriptor` derive macro and is retrieved in the `enum_descriptor_derive` function.
///
/// The `arg_description` attribute takes one argument, `description`, which is a string literal.
#[proc_macro_attribute]
pub fn arg_description(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// A derive procedural macro for the `EnumDescriptor` trait.
///
/// The `EnumDescriptor` trait should have a function `name_with_token_count`
/// that returns a tuple with the name of the enum type as a string and the
/// token count for the name as an `usize`.
///
/// This procedural macro generates an implementation of `EnumDescriptor` for
/// the type on which it's applied. The `name_with_token_count` function, in the
/// generated implementation, returns the name of the type and its token count.
///
/// # Usage
///
/// Use the `#[derive(EnumDescriptor)]` attribute on an enum to derive the
/// `EnumDescriptor` trait for it.
///
/// ```
/// #[derive(EnumDescriptor)]
/// enum MyEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// This will generate:
///
/// ```
/// impl EnumDescriptor for MyEnum {
///     fn name_with_token_count() -> (String, usize) {
///         (String::from("MyEnum"), /* token count of "MyEnum" */)
///     }
/// }
/// ```
///
/// The actual token count is computed during compile time using the
/// `calculate_token_count` function.
#[proc_macro_derive(EnumDescriptor, attributes(arg_description))]
pub fn enum_descriptor_derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, attrs, .. } = parse_macro_input!(input as DeriveInput);

    let name_str = ident.to_string();
    let name_token_count = calculate_token_count(&name_str);

    let mut description: &'static str = "";
    let mut desc_tokens = 0_usize;

    for attr in &attrs {
        if attr.path().is_ident("arg_description") {
            let _result = attr.parse_nested_meta(|meta| {
                let content = meta.input;

                if !content.is_empty() {
                    if meta.path.is_ident("description") {
                        let value = meta.value()?;
                        if let Ok(Lit::Str(value)) = value.parse() {
                            description = Box::leak(value.value().into_boxed_str());
                            desc_tokens = calculate_token_count(description);
                        }
                    }
                    return Ok(());
                }

                Err(meta.error("unrecognized my_attribute"))
            });

            if _result.is_err() {
                println!("Error parsing attribute:   {:#?}", _result);
            }
        }
    }

    let expanded = quote! {
        impl openai_func_enums::EnumDescriptor for #ident {
            fn name_with_token_count() -> &'static (&'static str, usize) {
                static NAME_DATA: (&'static str, usize) = (stringify!(#ident), #name_token_count);
                &NAME_DATA
            }

            fn arg_description_with_token_count() -> &'static (&'static str, usize) {
                static DESC_DATA: (&'static str, usize) = (#description, #desc_tokens);
                &DESC_DATA
            }
        }
    };

    TokenStream::from(expanded)
}

/// A derive procedural macro for the `VariantDescriptors` trait.
///
/// This macro generates an implementation of the `VariantDescriptors` trait for
/// an enum. The trait provides two methods:
///
/// 1. `variant_names_with_token_counts`: Returns a `Vec` containing tuples,
/// each with a string representation of a variant's name and its token count.
///
/// 2. `variant_name_with_token_count`: Takes an enum variant as input and
/// returns a tuple with the variant's name as a string and its token count.
///
/// Note: This macro will panic if it is used on anything other than an enum.
///
/// # Usage
///
/// ```
/// #[derive(VariantDescriptors)]
/// enum MyEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// This will generate the following:
///
/// ```
/// impl VariantDescriptors for MyEnum {
///     fn variant_names_with_token_counts() -> Vec<(String, usize)> {
///         vec![
///             (String::from("Variant1"), /* token count of "Variant1" */),
///             (String::from("Variant2"), /* token count of "Variant2" */),
///         ]
///     }
///
///     fn variant_name_with_token_count(&self) -> (String, usize) {
///         match self {
///             Self::Variant1 => (String::from("Variant1"), /* token count of "Variant1" */),
///             Self::Variant2 => (String::from("Variant2"), /* token count of "Variant2" */),
///         }
///     }
/// }
/// ```
///
/// The actual token count is computed during compile time using the
/// `calculate_token_count` function.
#[proc_macro_derive(VariantDescriptors)]
pub fn variant_descriptors_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let enum_name = &ast.ident;

    let variants = if let syn::Data::Enum(ref e) = ast.data {
        e.variants
            .iter()
            .map(|v| {
                let variant_name = &v.ident;
                let token_count = calculate_token_count(&variant_name.to_string());

                (variant_name, token_count)
            })
            .collect::<Vec<_>>()
    } else {
        panic!("VariantDescriptors can only be used with enums");
    };

    let variant_name_with_token_count: Vec<_> = variants
        .iter()
        .map(|(variant_name, token_count)| {
            quote! { Self::#variant_name => (stringify!(#variant_name), #token_count) }
        })
        .collect();

    let variant_names: Vec<_> = variants
        .iter()
        .map(|(variant_name, _)| quote! { stringify!(#variant_name) })
        .collect();

    let variant_name_additional_tokens = variant_names.len() * 3;

    let token_counts: Vec<_> = variants
        .iter()
        .map(|(_, token_count)| quote! { #token_count })
        .collect();

    let total_token_count = variants
        .iter()
        .map(|(_, token_count)| *token_count)
        .sum::<usize>();

    let expanded = quote! {
        impl VariantDescriptors for #enum_name {
            fn variant_names_with_token_counts() -> &'static (&'static [&'static str], &'static [usize], usize, usize) {
                static VARIANT_DATA: (&'static [&'static str], &'static [usize], usize, usize) = (
                    &[#(#variant_names),*],
                    &[#(#token_counts),*],
                    #total_token_count,
                    #variant_name_additional_tokens
                );
                &VARIANT_DATA
            }

            fn variant_name_with_token_count(&self) -> (&'static str, usize) {
                match self {
                    #(#variant_name_with_token_count,)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// A procedural macro to generate JSON information about an enum, including its name,
/// variant names, and descriptions, along with a total token count.
///
/// This macro leverages the `EnumDescriptor` and `VariantDescriptors` traits to extract
/// details about an enum. It compiles these details into a JSON format and calculates
/// an associated token count based on the structure of the generated JSON. The token count
/// is an estimation of how many tokens are needed to represent the enum information in a
/// serialized format, considering the syntax and spacing of JSON.
///
/// The macro returns a tuple containing the generated JSON object and the estimated total
/// token count.
///
/// # Usage
///
/// When applied to an enum, the macro generates code similar to the following example:
///
/// ```rust
/// {
///     use serde_json::Value;
///     let mut token_count = 0;
///
///     // Description and token count for the enum's argument (if applicable)
///     let (arg_desc, arg_tokens) = <MyEnum as EnumDescriptor>::arg_description_with_token_count();
///     token_count += 6; // Base tokens for argument declaration
///     token_count += arg_tokens; // Tokens for the argument description
///
///     // Enum name and its token count
///     let enum_name = <MyEnum as EnumDescriptor>::name_with_token_count();
///     token_count += 6; // Base tokens for enum name declaration
///     token_count += enum_name.1; // Tokens for the enum name
///
///     // Base tokens for enum and type declarations
///     token_count += 7; // Enum declaration
///     token_count += 7; // Type declaration
///
///     // Variant names and their token counts
///     let enum_variants = <MyEnum as VariantDescriptors>::variant_names_with_token_counts();
///     // Adding 3 tokens for each variant for proper JSON formatting
///     token_count += enum_variants.iter().map(|(_, token_count_i)| *token_count_i + 3).sum::<usize>();
///
///     // Constructing the JSON object with enum details
///     let json_enum = serde_json::json!({
///         enum_name.0: {
///             "type": "string",
///             "enum": enum_variants.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(),
///             "description": arg_desc,
///         }
///     });
///
///     (json_enum, token_count)
/// }
/// ```
///
/// ## Token Count Estimation Details
///
/// The estimation of tokens for the generated JSON includes:
/// - **Base tokens for argument and enum declarations**: A fixed count to account for the JSON structure around the enum and its arguments.
/// - **Dynamic tokens for the enum name and argument descriptions**: Calculated based on the length and structure of the enum name and argument descriptions.
/// - **Tokens for each enum variant**: Includes a fixed addition for JSON formatting alongside the variant names.
///
/// This approach ensures a precise estimation of the token count required to represent the enum information in JSON, facilitating accurate serialization.
///
/// Note: The enum must implement the `EnumDescriptor` and `VariantDescriptors` traits for the macro to function correctly. The actual token count is computed at compile time using these traits' methods.
#[proc_macro]
pub fn generate_enum_info(input: TokenStream) -> TokenStream {
    let enum_ident = parse_macro_input!(input as Ident);

    let output = quote! {
        {
            let ARG_DESC_AND_TOKENS: &'static (&'static str, usize) = <#enum_ident as openai_func_enums::EnumDescriptor>::arg_description_with_token_count();
            let ENUM_NAME_AND_TOKENS: &'static (&'static str, usize) = <#enum_ident as openai_func_enums::EnumDescriptor>::name_with_token_count();
            let ENUM_VARIANTS_INFO: &'static (&'static [&'static str], &'static [usize], usize, usize) = <#enum_ident as openai_func_enums::VariantDescriptors>::variant_names_with_token_counts();

            let token_count = 6 + ARG_DESC_AND_TOKENS.1 + 6 + ENUM_NAME_AND_TOKENS.1 + 7 + 7 + ENUM_VARIANTS_INFO.2 + ENUM_VARIANTS_INFO.3;

            let json_enum: Value = serde_json::json!({
                ENUM_NAME_AND_TOKENS.0: {
                    "type": "string",
                    "enum": ENUM_VARIANTS_INFO.0.iter().map(|name| *name).collect::<Vec<_>>(),
                    "description": ARG_DESC_AND_TOKENS.0,
                }
            });

            (json_enum, token_count)
        }
    };

    output.into()
}

#[proc_macro]
pub fn generate_value_arg_info(input: TokenStream) -> TokenStream {
    let mut type_and_name_values = Vec::new();

    let tokens = input.into_iter().collect::<Vec<TokenTree>>();
    for token in tokens {
        if let TokenTree::Ident(ident) = &token {
            type_and_name_values.push(ident.to_string());
        }
    }

    let output = if type_and_name_values.len() == 2 {
        let name = &type_and_name_values[1];
        let type_name = &type_and_name_values[0];

        let name_tokens = calculate_token_count(name);
        let type_name_tokens = calculate_token_count(type_name);
        let mut total_tokens = name_tokens + type_name_tokens;

        let json_string = if type_name == "array" {
            total_tokens += 22;
            format!(
                r#"{{"{}": {{"type": "array", "items": {{"type": "string"}}}}}}"#,
                name
            )
        } else {
            total_tokens += 11;
            format!(r#"{{"{}": {{"type": "{}"}}}}"#, name, type_name)
        };

        quote! {
            {
                static JSON_STR: &str = #json_string;
                let json_enum: serde_json::Value = serde_json::from_str(JSON_STR).unwrap();
                (json_enum, #total_tokens)
            }
        }
    } else {
        quote! {}
    };

    output.into()
}

/// This procedural macro attribute is used to specify a description for an enum variant.
///
/// The `func_description` attribute does not modify the input it is given.
/// It's only used to attach metadata (i.e., a description) to enum variants.
///
/// # Usage
///
/// ```rust
/// enum MyEnum {
///     #[func_description(description="This function does a thing.")]
///     DoAThing,
///     #[func_description(description="This function does another thing.")]
///     DoAnotherThing,
/// }
/// ```
///
/// Note: The actual usage of the description provided through this attribute happens
/// in the `FunctionCallResponse` derive macro and is retrieved in the `impl_function_call_response` function.
#[deprecated(since = "0.3.0", note = "Use a doc string --> '///'.")]
#[proc_macro_attribute]
pub fn func_description(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// The `ToolSet` procedural macro is used to derive a structure
/// which encapsulates various chat completion commands.
///
/// This macro should be applied to an enum. It generates various supporting
/// structures and methods, including structures representing the command arguments,
/// methods for converting between the argument structures and the original enum,
/// JSON conversion methods, and an implementation of the original enum that provides
/// methods for executing the commands and dealing with the responses.
///
/// Each variant of the original enum will be converted into a corresponding structure,
/// and each field in the variant will become a field in the generated structure.
/// The generated structures will derive `serde::Deserialize` and `Debug` automatically.
///
/// This macro also generates methods for calculating the token count of a string and
/// for executing commands based on function calls received from the chat API.
///
/// The types of fields in the enum variants determine how the corresponding fields in the
/// generated structures are treated. For example, fields of type `String` or `&str` are
/// converted to JSON value arguments with type `"string"`, while fields of type `u8`, `u16`,
/// `u32`, `u64`, `usize`, `i8`, `i16`, `i32`, `i64`, `isize`, `f32` or `f64` are converted
/// to JSON value arguments with type `"integer"` or `"number"` respectively.
/// For fields with a tuple type, currently this macro simply prints that the field is of a tuple type.
/// For fields with an array type, they are converted to JSON value arguments with type `"array"`.
///
/// When running the chat command, a custom system message can be optionally provided.
/// If provided, this message will be used as the system message in the chat request.
/// If not provided, a default system message will be used.
///
/// If the total token count of the request exceeds a specified limit, an error will be returned.
///
/// The `derive_subcommand_gpt` function consumes a `TokenStream` representing the enum
/// to which the macro is applied and produces a `TokenStream` representing the generated code.
///
/// # Panics
/// This macro will panic (only at compile time) if it is applied to a non-enum item.
#[proc_macro_derive(ToolSet)]
pub fn derive_subcommand_gpt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let data = match input.data {
        Data::Enum(data) => data,
        _ => panic!("ToolSet can only be implemented for enums"),
    };

    let mut generated_structs = Vec::new();
    let mut json_generator_functions = Vec::new();

    let mut generated_clap_gpt_enum: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut generated_struct_names = Vec::new();

    #[cfg(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update"
    ))]
    let rt = tokio::runtime::Runtime::new().unwrap();

    #[cfg(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update",
        feature = "function_filtering"
    ))]
    let embed_path = std::env::var("FUNC_ENUMS_EMBED_PATH")
        .expect("Functionality for embeddings requires environment variable FUNC_ENUMS_EMBED_PATH to be set.");

    #[cfg(not(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update",
        feature = "function_filtering"
    )))]
    let embed_path = "";

    #[cfg(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update",
        feature = "function_filtering"
    ))]
    let embed_model = std::env::var("FUNC_ENUMS_EMBED_MODEL")
        .expect("Functionality for embeddings requires environment variable FUNC_ENUMS_EMBED_MODEL to be set.");

    // We can set const values, we can have feature flags, but
    // the compiler will not allow us to maybe set a const behind
    // a feature flag.
    #[cfg(not(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update",
        feature = "function_filtering"
    )))]
    let embed_model = "";

    let max_response_tokens: u16 = std::env::var("FUNC_ENUMS_MAX_RESPONSE_TOKENS")
        .expect("Environment variable FUNC_ENUMS_MAX_RESPONSE_TOKENS is required. See build.rs files in the examples.")
        .parse()
        .expect("Failed to parse u16 value from FUNC_ENUMS_MAX_RESPONSE_TOKENS");

    let max_request_tokens: usize = std::env::var("FUNC_ENUMS_MAX_REQUEST_TOKENS")
        .expect("Environment variable FUNC_ENUMS_MAX_REQUEST_TOKENS is required. See build.rs files in the examples.")
        .parse()
        .expect("Failed to parse usize value from FUNC_ENUMS_MAX_REQUEST_TOKENS");

    let max_func_tokens: u16 =std::env::var("FUNC_ENUMS_MAX_FUNC_TOKENS")
        .expect("Environment variable FUNC_ENUMS_MAX_FUNC_TOKENS is required. See build.rs files in the examples.")
        .parse()
        .expect("Failed to parse u16 value from FUNC_ENUMS_MAX_FUNC_TOKENS");

    let max_single_arg_tokens: u16 = std::env::var("FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS") 
        .expect("Environment variable FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS is required. See build.rs files in the examples.")
        .parse()
        .expect("Failed to parse u16 value from FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS");

    #[cfg(any(
        feature = "compile_embeddings_all",
        feature = "compile_embeddings_update"
    ))]
    let mut embeddings: Vec<openai_func_embeddings::FuncEmbedding> = Vec::new();

    #[cfg(feature = "compile_embeddings_update")]
    {
        if Path::new(&embed_path).exists() {
            let mut file = std::fs::File::open(&embed_path).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            let archived_data = rkyv::check_archived_root::<Vec<FuncEmbedding>>(&bytes).unwrap();
            embeddings = archived_data.deserialize(&mut rkyv::Infallible).unwrap();
        }
    }

    let mut has_gpt_variant = false;
    // TODO: make this setable:
    let gpt_variant_name = "GPT";
    for variant in data.variants.iter() {
        let variant_name = &variant.ident;
        if variant_name.to_string() == gpt_variant_name {
            has_gpt_variant = true;
        }

        let struct_name = format_ident!("{}", variant_name);
        let struct_name_tokens = calculate_token_count(struct_name.to_string().as_str());
        generated_struct_names.push(struct_name.clone());
        let mut variant_desc = String::new();
        let mut variant_desc_tokens = 0_usize;

        for variant_attrs in &variant.attrs {
            let description = get_comment_from_attr(variant_attrs);
            if let Some(description) = description {
                variant_desc = description;
                variant_desc_tokens = calculate_token_count(variant_desc.as_str());

                // TODO: Do a default, show a helpful error message, do something, you will forget
                #[cfg(feature = "compile_embeddings_all")]
                {
                    println!("Writing embeddings");
                    let mut name_and_desc = variant_name.to_string();
                    name_and_desc.push(':');
                    name_and_desc.push_str(&variant_desc);

                    rt.block_on(async {
                        let embedding = get_single_embedding(&name_and_desc, &embed_model).await;
                        if let Ok(embedding) = embedding {
                            let data = openai_func_embeddings::FuncEmbedding {
                                name: variant_name.to_string(),
                                description: variant_desc.clone(),
                                embedding,
                            };

                            embeddings.push(data);
                        }
                    });
                }

                #[cfg(feature = "compile_embeddings_update")]
                {
                    let mut name_and_desc = variant_name.to_string();
                    name_and_desc.push(':');
                    name_and_desc.push_str(&variant_desc);

                    rt.block_on(async {
                        let mut existing = embeddings.iter().find(|x| x.name == name);

                        if let Some(existing) = existing {
                            if existing.description != variant_desc {
                                let embedding =
                                    get_single_embedding(&name_and_desc, &embed_model).await;

                                if let Ok(embedding) = embedding {
                                    existing.description = variant_desc.clone();
                                    existing.embedding = embedding;
                                }
                            }
                        } else {
                            let embedding =
                                get_single_embedding(&name_and_desc, &embed_model).await;
                            if let Ok(embedding) = embedding {
                                let data = FuncEmbedding {
                                    name: variant_name.to_string(),
                                    description: variant_desc.clone(),
                                    embedding,
                                };

                                embeddings.push(data);
                            }
                        }
                    });
                }
            }
        }

        #[cfg(any(
            feature = "compile_embeddings_all",
            feature = "compile_embeddings_update"
        ))]
        {
            let serialized_data = rkyv::to_bytes::<_, 256>(&embeddings).unwrap();
            let mut file = std::fs::File::create(&embed_path).unwrap();
            file.write_all(&serialized_data).unwrap();
        }

        let fields: Vec<_> = variant
            .fields
            .iter()
            .map(|f| {
                // If the field has an identifier (i.e., it is a named field),
                // use it. Otherwise, use the type as the name.
                let field_name = if let Some(ident) = &f.ident {
                    format_ident!("{}", ident)
                } else {
                    format_ident!("{}", to_snake_case(&f.ty.to_token_stream().to_string()))
                };
                let field_type = &f.ty;
                quote! {
                    pub #field_name: #field_type,
                }
            })
            .collect();

        let execute_command_parameters: Vec<_> = variant
            .fields
            .iter()
            .map(|field| {
                let field_name = &field.ident;
                quote! { #field_name: self.#field_name.clone() }
            })
            .collect();

        let number_type = "number";
        let number_ident = format_ident!("{}", number_type);
        let integer_type = "integer";
        let integer_ident = format_ident!("{}", integer_type);
        let string_type = "string";
        let string_ident = format_ident!("{}", string_type);
        let array_type = "array";
        let array_ident = format_ident!("{}", array_type);

        let field_info: Vec<_> = variant
            .fields
            .iter()
            .map(|f| {
                let field_name = if let Some(ident) = &f.ident {
                    format_ident!("{}", ident)
                } else {
                    format_ident!("{}", to_snake_case(&f.ty.to_token_stream().to_string()))
                };
                let field_type = &f.ty;

                match field_type {
                    syn::Type::Path(typepath) if typepath.qself.is_none() => {
                        let type_ident = &typepath.path.segments.last().unwrap().ident;

                        match type_ident.to_string().as_str() {
                            "f32" | "f64" => {
                                return quote! {
                                    generate_value_arg_info!(#number_ident, #field_name)
                                };
                            }
                            "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16"
                            | "i32" | "i64" | "i128" | "isize" => {
                                return quote! {
                                    generate_value_arg_info!(#integer_ident, #field_name)
                                };
                            }
                            "String" | "&str" => {
                                return quote! {
                                    generate_value_arg_info!(#string_ident, #field_name)
                                };
                            }
                            "Vec" => {
                                return quote! {
                                    generate_value_arg_info!(#array_ident, #field_name)
                                };
                            }
                            _ => {
                                return quote! {
                                    openai_func_enums::generate_enum_info!(#field_type)
                                };
                            }
                        }
                    }
                    syn::Type::Tuple(_) => {
                        println!("Field {} is of tuple type", field_name);
                    }
                    syn::Type::Array(_) => {
                        println!("Field {} is of array type", field_name);
                        return quote! {
                            generate_value_arg_info!(#array_ident, #field_name)
                        };
                    }
                    _ => {
                        println!("Field {} is of another type.", field_name);
                    }
                }
                quote! {}
            })
            .collect();

        json_generator_functions.push(quote! {
            impl #struct_name {
                pub fn name() -> String {
                    stringify!(#struct_name).to_string()
                }

                pub fn to_function_call() -> ChatCompletionFunctionCall {
                    ChatCompletionFunctionCall::Function {
                        name: stringify!(#struct_name).to_string(),
                    }
                }

                pub fn to_tool_choice() -> ChatCompletionToolChoiceOption {
                    ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionName { name: stringify!(#struct_name).to_string() }
                    })
                }

                pub fn execute_command(&self) -> #name {
                    #name::#variant_name {
                        #(#execute_command_parameters),*
                    }
                }

                // Bake this in. Can be much faster.
                pub fn get_function_json() -> (serde_json::Value, usize) {
                    let mut parameters = serde_json::Map::new();
                    let mut total_tokens = 0;

                    for (arg_json, arg_tokens) in vec![#(#field_info),*] {
                        total_tokens += arg_tokens;
                        total_tokens += 3;

                        parameters.insert(
                            arg_json.as_object().unwrap().keys().next().unwrap().clone(),
                            arg_json
                                .as_object()
                                .unwrap()
                                .values()
                                .next()
                                .unwrap()
                                .clone(),
                        );
                    }

                    let function_json = serde_json::json!({
                        "name": stringify!(#struct_name),
                        "description": #variant_desc,
                        "parameters": {
                            "type": "object",
                            "properties": parameters,
                            "required": parameters.keys().collect::<Vec<_>>()
                        }
                    });

                    total_tokens += 43;
                    total_tokens += #struct_name_tokens;
                    total_tokens += #variant_desc_tokens;

                    (function_json, total_tokens)
                }
            }
        });

        generated_structs.push(quote! {
            #[derive(Clone, serde::Deserialize, Debug)]
            pub struct #struct_name {
                #(#fields)*
            }
        });
    }

    if !has_gpt_variant {
        panic!("Enums that derive ToolSet must define a variant called 'GPT'.")
    }

    let all_function_calls = quote! {
        pub fn all_function_jsons() -> (serde_json::Value, usize) {
            let results = vec![#(#generated_struct_names::get_function_json(),)*];
            let combined_json = serde_json::Value::Array(results.iter().map(|(json, _)| json.clone()).collect());
            let total_tokens = results.iter().map(|(_, tokens)| tokens).sum();
            (combined_json, total_tokens)
        }

        pub fn function_jsons_under_limit(ranked_func_names: Vec<String>) -> (serde_json::Value, usize) {
            let results = vec![#(#generated_struct_names::get_function_json(),)*];

            let limit = #max_func_tokens as usize;
            let (functions_to_present, total_tokens) = results.into_iter().fold(
                (vec![], 0_usize),
                |(mut acc, token_count), (json, tokens)| {
                    if token_count + tokens <= limit {
                        acc.push((json.clone(), tokens));
                        (acc, token_count + tokens)
                    } else {
                        (acc, token_count)
                    }
                },
            );

            let combined_json = serde_json::Value::Array(functions_to_present.iter().map(|(json, _)| json.clone()).collect());
            (combined_json, total_tokens)
        }

        pub fn function_jsons_allowed_with_required(
            allowed_func_names: Vec<String>,
            required_func_names: Option<Vec<String>>
        ) -> (serde_json::Value, usize) {
            let results = vec![#(#generated_struct_names::get_function_json(),)*];
            let required_func_names = required_func_names.unwrap_or_default();

            // Take the vector of what has to be there just for it to function and add the ranked
            // functions to it, skipping ranked ones if it is already in the required list.
            let updated_func_names = required_func_names.iter()
                .chain(allowed_func_names.iter().filter(|name| !required_func_names.contains(name)))
                .cloned()
                .collect::<Vec<String>>();

            let (functions_to_present, total_tokens) = updated_func_names.iter()
                .filter_map(|name| results.iter().find(|(json, _)| json["name"] == *name))
                .fold((vec![], 0_usize), |(mut acc, token_count), (json, tokens)| {
                    acc.push((json.clone(), tokens));
                    (acc, token_count + tokens)
                });

            let combined_json = serde_json::Value::Array(functions_to_present.iter().map(|(json, _)| json.clone()).collect());
            (combined_json, total_tokens)
        }

        pub fn function_jsons_with_required_under_limit(
            ranked_func_names: Vec<String>,
            required_func_names: Option<Vec<String>>
        ) -> (serde_json::Value, usize) {
            let results = vec![#(#generated_struct_names::get_function_json(),)*];
            let required_func_names = required_func_names.unwrap_or_default();

            // Take the vector of what has to be there just for it to function and add the ranked
            // functions to it, skipping ranked ones if it is already in the required list.
            let updated_func_names = required_func_names.iter()
                .chain(ranked_func_names.iter().filter(|name| !required_func_names.contains(name)))
                .cloned()
                .collect::<Vec<String>>();

            let limit = #max_func_tokens as usize;

            let (functions_to_present, total_tokens) = updated_func_names.iter()
                .filter_map(|name| results.iter().find(|(json, _)| json["name"] == *name))
                .fold((vec![], 0_usize), |(mut acc, token_count), (json, tokens)| {
                    if token_count + tokens <= limit {
                        acc.push((json.clone(), tokens));
                        (acc, token_count + tokens)
                    } else {
                        (acc, token_count)
                    }
                });

            let combined_json = serde_json::Value::Array(functions_to_present.iter().map(|(json, _)| json.clone()).collect());
            (combined_json, total_tokens)
        }
    };

    {
        generated_clap_gpt_enum.push(quote! {
            pub enum CommandsGPT {
                GPT { a: String },
            }
        });
    }

    let struct_names: Vec<String> = generated_struct_names
        .iter()
        .map(|name| format!("{}", name))
        .collect();

    let match_arms: Vec<_> = generated_struct_names
        .iter()
        .map(|struct_name| {
            let response_name = format_ident!("{}", struct_name);

            quote! {
                Ok(FunctionResponse::#response_name(response)) => {
                    let result = response.execute_command();
                    let command_clone = command.clone();
                    let custom_system_message_clone = custom_system_message.clone();
                    let logger_clone = logger.clone();
                    let command_lock = command_clone.lock().await;
                    let command_inner_value = command_lock.as_ref().cloned();
                    drop(command_lock);

                    let run_result = result.run(execution_strategy_clone, command_inner_value, logger_clone, custom_system_message_clone).await;
                    match run_result {
                        Ok(run_result) => {
                            {
                                let prior_result_clone = prior_result.clone();
                                let mut prior_result_lock = prior_result_clone.lock().await;
                                *prior_result_lock = run_result.0;

                                let command_clone = command.clone();
                                let mut command_lock = command_clone.lock().await;
                                *command_lock = run_result.1;

                                let custom_system_message_clone = custom_system_message.clone();
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            println!("{:#?}", e);
                        }
                    }
                }
            }
        })
        .collect();

    // TODO: reload this shit into your head.
    let match_arms_no_return: Vec<_> = generated_struct_names
        .iter()
        .map(|struct_name| {
            let response_name = format_ident!("{}", struct_name);

            quote! {
                Ok(FunctionResponse::#response_name(response)) => {
                    let result = response.execute_command();
                    let run_result = result.run(execution_strategy_clone, None, logger_clone, custom_system_message_clone).await;
                    match run_result {
                        Ok(run_result) => {
                            {
                                // Feels like this is a dead lock.
                                // Update: isn't.
                                let mut prior_result_lock = prior_result_clone.lock().await;
                                *prior_result_lock = run_result.0;

                                let mut command_lock = command_clone.lock().await;
                                *command_lock = run_result.1;
                            }
                        }
                        Err(e) => {
                            println!("{:#?}", e);
                        }
                    }
                }
            }
        })
        .collect();

    #[cfg(feature = "function_filtering")]
    let filtering_delegate = quote! {
        openai_func_enums::get_tools_limited(CommandsGPT::function_jsons_with_required_under_limit, allowed_functions, required_functions)?
    };

    #[cfg(not(feature = "function_filtering"))]
    let filtering_delegate = quote! {
        openai_func_enums::get_tools_limited(CommandsGPT::function_jsons_allowed_with_required, allowed_functions, required_functions)?
    };

    let commands_gpt_impl = quote! {
        #[derive(Clone, Debug, serde::Deserialize)]
        pub enum FunctionResponse {
            #(
                #generated_struct_names(#generated_struct_names),
            )*
        }

        impl CommandsGPT {
            #all_function_calls

            fn to_snake_case(camel_case: &str) -> String {
                let mut snake_case = String::new();
                for (i, ch) in camel_case.char_indices() {
                    if i > 0 && ch.is_uppercase() {
                        snake_case.push('_');
                    }
                    snake_case.extend(ch.to_lowercase());
                }
                snake_case
            }

            pub fn parse_gpt_function_call(function_call: &FunctionCall) -> Result<FunctionResponse, Box<dyn std::error::Error + Send + Sync + 'static>> {
                match function_call.name.as_str() {
                    #(
                    #struct_names => {
                        match serde_json::from_str::<#generated_struct_names>(&function_call.arguments) {
                            Ok(arguments) => Ok(FunctionResponse::#generated_struct_names(arguments)),
                            Err(_) => {
                                let snake_case_args = function_call.arguments
                                    .as_str()
                                    .split(',')
                                    .map(|s| {
                                        let mut parts = s.split(':');
                                        match (parts.next(), parts.next()) {
                                            (Some(key), Some(value)) => {
                                                let key_trimmed = key.trim_matches(|c: char| !c.is_alphanumeric()).trim();
                                                let key_snake_case = Self::to_snake_case(key_trimmed);
                                                format!("\"{}\":{}", key_snake_case, value)
                                            },
                                            _ => s.to_string()
                                        }
                                    })
                                    .collect::<Vec<String>>()
                                    .join(",");

                                let snake_case_args = format!("{{{}", snake_case_args);

                                match serde_json::from_str::<#generated_struct_names>(&snake_case_args) {
                                    Ok(arguments) => {
                                        Ok(FunctionResponse::#generated_struct_names(arguments))
                                    }
                                    Err(e) => {
                                        Err(Box::new(openai_func_enums::CommandError::new("There was an issue deserializing function arguments.")))
                                    }
                                }
                            }
                        }
                    },
                    )*
                    _ => {
                        println!("{:#?}", function_call);
                        Err(Box::new(openai_func_enums::CommandError::new("Unknown function name")))
                    }
                }
            }

            fn calculate_token_count(text: &str) -> usize {
                let bpe = tiktoken_rs::cl100k_base().unwrap();
                bpe.encode_ordinary(&text).len()
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn run(
                prompt: &String,
                model_name: &str,
                request_token_limit: Option<usize>,
                max_response_tokens: Option<u16>,
                custom_system_message: Option<(String, usize)>,
                prior_result: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
                execution_strategy: ToolCallExecutionStrategy,
                command: std::sync::Arc<tokio::sync::Mutex<Option<Vec<String>>>>,
                allowed_functions: Option<Vec<String>>,
                required_functions: Option<Vec<String>>,
                logger: std::sync::Arc<openai_func_enums::Logger>,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {

                let tool_args: (Vec<async_openai::types::ChatCompletionTool>, usize) = if let Some(allowed_functions) = allowed_functions {
                    if !allowed_functions.is_empty() {
                        #filtering_delegate
                    } else {
                        get_tool_chat_completion_args(CommandsGPT::all_function_jsons)?
                    }

                } else {
                    get_tool_chat_completion_args(CommandsGPT::all_function_jsons)?
                };

                let custom_system_message_clone = custom_system_message.clone();
                let (this_system_message, system_message_tokens) = match custom_system_message_clone {
                    Some((message, tokens)) => {
                        (message.clone(), tokens)
                    }
                    None => (String::from("You are a helpful function calling bot."), 7)
                };

                let word_count = prompt.split_whitespace().count();

                let request_token_total = tool_args.1 + system_message_tokens + if word_count < 200 {
                    ((word_count as f64 / 0.75).round() as usize)
                } else {
                    Self::calculate_token_count(prompt.as_str())
                };

                if request_token_total > request_token_limit.unwrap_or(FUNC_ENUMS_MAX_REQUEST_TOKENS)  {
                    return Err(Box::new(openai_func_enums::CommandError::new("Request token count is too high")));
                }

                let this_system_message_clone = this_system_message.clone();

                let request = CreateChatCompletionRequestArgs::default()
                    .max_tokens(max_response_tokens.unwrap_or(FUNC_ENUMS_MAX_RESPONSE_TOKENS))
                    .model(model_name)
                    .temperature(0.0)
                    .messages([ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessageArgs::default()
                        .content(this_system_message_clone)
                        .build()?),
                    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessageArgs::default()
                        .content(prompt.to_string())
                        .build()?)])
                    .tools(tool_args.0)
                    .tool_choice("auto")
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

                if let Some(tool_calls) = response_message.tool_calls {
                    if tool_calls.len() == 1 {
                        let execution_strategy_clone = execution_strategy.clone();
                        let custom_system_message_clone = custom_system_message.clone();

                        match Self::parse_gpt_function_call(&tool_calls.first().unwrap().function) {
                            #(#match_arms,)*
                            Err(e) => {
                                println!("{:#?}", e);
                                return Err(Box::new(openai_func_enums::CommandError::new("Error running GPT command")));
                            }
                        };
                    } else {
                        match execution_strategy {
                            ToolCallExecutionStrategy::Async => {
                                let mut tasks = Vec::new();

                                let custom_system_message_clone = custom_system_message.clone();
                                for tool_call in tool_calls.iter() {
                                    match tool_call.r#type {
                                        ChatCompletionToolType::Function => {
                                            let function = tool_call.function.clone();
                                            let prior_result_clone = prior_result.clone();
                                            let command_clone = command.clone();
                                            let execution_strategy_clone = execution_strategy.clone();
                                            let logger_clone = logger.clone();
                                            let custom_system_message_clone = custom_system_message.clone();

                                            let task = tokio::spawn( async move {
                                                match Self::parse_gpt_function_call(&function) {
                                                    #(#match_arms_no_return,)*
                                                    Err(e) => {
                                                        println!("{:#?}", e);
                                                    }
                                                }
                                            });
                                            tasks.push(task);
                                        },
                                    }
                                }

                                for task in tasks {
                                    let _ = task.await;
                                }
                            },
                            ToolCallExecutionStrategy::Synchronous => {
                                for tool_call in tool_calls.iter() {
                                    match tool_call.r#type {
                                        ChatCompletionToolType::Function => {
                                            let prior_result_clone = prior_result.clone();
                                            let command_clone = command.clone();
                                            let execution_strategy_clone = execution_strategy.clone();
                                            let logger_clone = logger.clone();
                                            let custom_system_message_clone = custom_system_message.clone();

                                            match Self::parse_gpt_function_call(&tool_call.function) {
                                                #(#match_arms_no_return,)*
                                                Err(e) => {
                                                    println!("{:#?}", e);
                                                }
                                            }
                                        },
                                    }
                                }
                            },
                            ToolCallExecutionStrategy::Parallel => {
                                let mut handles = Vec::new();

                                for tool_call in tool_calls.iter() {
                                    match tool_call.r#type {
                                        ChatCompletionToolType::Function => {
                                            let function = tool_call.function.clone();
                                            let prior_result_clone = prior_result.clone();
                                            let command_clone = command.clone();

                                            // TODO: Think through. There's a lot of overhead to
                                            // make os threads this way. For now assume that if
                                            // strategy is set to "Parallel" that we only want to
                                            // put the intially returned tool calls on threads, and
                                            // if they themselves contain something multi-step we
                                            // will run those as if they are io-bound. Potentially
                                            // makes sense to support letting variants get
                                            // decorated with a execution strategy preference like
                                            // "this is io bound" or "this is cpu bound".
                                            // This will rarely matter.
                                            let execution_strategy_clone = ToolCallExecutionStrategy::Async;
                                            let logger_clone = logger.clone();
                                            let custom_system_message_clone = custom_system_message.clone();

                                            let handle = std::thread::spawn(move || {
                                                let rt = tokio::runtime::Runtime::new().unwrap();
                                                rt.block_on(async {
                                                    match Self::parse_gpt_function_call(&function) {
                                                        #(#match_arms_no_return,)*
                                                        Err(e) => {
                                                            println!("{:#?}", e);
                                                        }
                                                    }

                                                })
                                            });
                                            handles.push(handle);
                                        },
                                    }
                                }

                                for handle in handles {
                                    let _ = handle.join();
                                }
                            },
                        }
                    }
                    Ok(())
                } else {
                    return Ok(());
                }
            }
        }
    };

    let embedding_imports = quote! {

        #[cfg(any(
            feature = "compile_embeddings_all",
            feature = "compile_embeddings_update",
            feature = "function_filtering"
        ))]
        use openai_func_enums::FuncEnumsError;

        pub const FUNC_ENUMS_EMBED_PATH: &str = #embed_path;

        pub const FUNC_ENUMS_EMBED_MODEL: &str = #embed_model;
    };

    let gen = quote! {
        pub const FUNC_ENUMS_MAX_RESPONSE_TOKENS: u16 = #max_response_tokens;
        pub const FUNC_ENUMS_MAX_REQUEST_TOKENS: usize = #max_request_tokens;
        pub const FUNC_ENUMS_MAX_FUNC_TOKENS: u16 = #max_func_tokens;
        pub const FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS: u16 = #max_single_arg_tokens;

        use serde::Deserialize;
        use serde_json::{json, Value};

        use openai_func_enums::{
            generate_value_arg_info, get_tool_chat_completion_args,
            ArchivedFuncEmbedding,
        };

        use rkyv::{archived_root, Archived};
        use rkyv::vec::ArchivedVec;

        use async_trait::async_trait;
        use async_openai::{
            types::{
                ChatCompletionFunctionCall, ChatCompletionNamedToolChoice, ChatCompletionRequestMessage,
                ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
                ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequestArgs,
                CreateEmbeddingRequestArgs, FunctionCall, FunctionName,
            },
            Client,
        };
        use tokio::sync::{mpsc};

        #embedding_imports

        #(#generated_structs)*

        #(#json_generator_functions)*

        #(#generated_clap_gpt_enum)*

        #commands_gpt_impl
    };

    gen.into()
}

fn get_comment_from_attr(attr: &Attribute) -> Option<String> {
    if attr.path().is_ident("doc") {
        if let Meta::NameValue(meta) = &attr.meta {
            if meta.path.is_ident("doc") {
                let value = meta.value.clone();
                match value {
                    Expr::Lit(value) => match value.lit {
                        Lit::Str(value) => {
                            return Some(value.value());
                        }
                        _ => {
                            return None;
                        }
                    },
                    _ => {
                        return None;
                    }
                }
            }
        }
    }
    None
}

/// Calculate the token count of a given text string using the Byte Pair Encoding (BPE) tokenizer.
///
/// This function utilizes the BPE tokenizer from the `cl100k_base` library. It tokenizes the given text and
/// returns the count of the tokens. This can be used to measure how many tokens a particular text string
/// consumes, which is often relevant in the context of natural language processing tasks.
///
/// # Arguments
///
/// * `text` - A string slice that holds the text to tokenize.
///
/// # Returns
///
/// * `usize` - The count of tokens in the text.
///
/// # Example
///
/// ```
/// let text = "Hello, world!";
/// let token_count = calculate_token_count(text);
/// println!("Token count: {}", token_count);
/// ```
///
/// Note: This function can fail if the `cl100k_base` tokenizer is not properly initialized or the text cannot be tokenized.
fn calculate_token_count(text: &str) -> usize {
    let bpe = tiktoken_rs::cl100k_base().unwrap();
    bpe.encode_ordinary(text).len()
}

/// Convert a camelCase or PascalCase string into a snake_case string.
///
/// This function iterates over each character in the input string. If the character is an uppercase letter, it adds an
/// underscore before it (except if it's the first character) and then appends the lowercase version of the character
/// to the output string.
///
/// # Arguments
///
/// * `camel_case` - A string slice that holds the camelCase or PascalCase string to convert.
///
/// # Returns
///
/// * `String` - The converted snake_case string.
///
/// # Example
///
/// ```
/// let camel_case = "HelloWorld";
/// let snake_case = to_snake_case(camel_case);
/// assert_eq!(snake_case, "hello_world");
/// ```
fn to_snake_case(camel_case: &str) -> String {
    let mut snake_case = String::new();
    for (i, ch) in camel_case.char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake_case.push('_');
        }
        snake_case.extend(ch.to_lowercase());
    }
    snake_case
}

#[cfg(any(
    feature = "compile_embeddings_all",
    feature = "compile_embeddings_update"
))]
async fn get_single_embedding(
    text: &String,
    model: &String,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let request = CreateEmbeddingRequestArgs::default()
        .model(model)
        .input([text])
        .build()?;

    let response = client.embeddings().create(request).await?;

    match response.data.first() {
        Some(data) => {
            return Ok(data.embedding.to_owned());
        }
        None => {
            let embedding_error = openai_func_embeddings::FuncEnumsError::OpenAIError(
                String::from("Didn't get embedding vector back."),
            );
            let boxed_error: Box<dyn std::error::Error + Send + Sync> = Box::new(embedding_error);
            return Err(boxed_error);
        }
    }
}
