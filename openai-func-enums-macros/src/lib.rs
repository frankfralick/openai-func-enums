use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Ident, Lit};
use tiktoken_rs::cl100k_base;

/// The `arg_description` attribute is a procedural macro used to provide additional description for an enum.
///
/// This attribute does not modify the code it annotates but instead attaches metadata in the form of a description.
/// This can be helpful for better code readability and understanding the purpose of different enums.
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

    let name_str = format!("{}", ident);
    let name_token_count = calculate_token_count(&name_str);

    let mut description = String::new();
    let mut desc_tokens = 0_usize;

    for attr in &attrs {
        if attr.path().is_ident("arg_description") {
            let _result = attr.parse_nested_meta(|meta| {
                let content = meta.input;

                while !content.is_empty() {
                    if meta.path.is_ident("description") {
                        let value = meta.value()?;
                        if let Ok(Lit::Str(value)) = value.parse() {
                            description = value.value();
                        }
                    } else if meta.path.is_ident("tokens") {
                        let value = meta.value()?;
                        if let Ok(Lit::Int(value)) = value.parse() {
                            desc_tokens = value.base10_parse::<usize>()?;
                            return Ok(());
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
        impl EnumDescriptor for #ident {
            fn name_with_token_count() -> (String, usize) {
                (String::from(#name_str), #name_token_count)
            }
            fn arg_description_with_token_count() -> (String, usize) {
                (String::from(#description), #desc_tokens)
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

    let variant_names_with_token_counts: Vec<_> = variants
        .iter()
        .map(|(variant_name, token_count)| {
            quote! { (stringify!(#variant_name).to_string(), #token_count) }
        })
        .collect();

    let variant_name_with_token_count: Vec<_> = variants
        .iter()
        .map(|(variant_name, token_count)| {
            quote! { Self::#variant_name => (stringify!(#variant_name).to_string(), #token_count) }
        })
        .collect();

    let expanded = quote! {
        impl VariantDescriptors for #enum_name {
            fn variant_names_with_token_counts() -> Vec<(String, usize)> {
                vec![
                    #(#variant_names_with_token_counts),*
                ]
            }

            fn variant_name_with_token_count(&self) -> (String, usize) {
                match self {
                    #(#variant_name_with_token_count,)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// A procedural macro to generate information about an enum.
///
/// This macro generates code that uses the `EnumDescriptor` and `VariantDescriptors`
/// traits to extract information about an enum, including its name, variant names,
/// and their corresponding token counts. Additionally, it uses the `FunctionArgument` trait
/// to fetch the argument description. All this information is serialized into JSON.
///
/// The macro returns a tuple containing the JSON and the total token count.
///
/// # Usage
///
/// ```rust
/// #[generate_enum_info]
/// enum MyEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// The generated code will look like this:
///
/// ```rust
/// {
///     use serde_json::Value;
///     let mut total_tokens = 0;
///
///     let (arg_desc, arg_count) = <MyEnum as ::openai_func_enums::FunctionArgument>::argument_description_with_token_count();
///     total_tokens += arg_count;
///
///     let enum_name = <MyEnum as EnumDescriptor>::name_with_token_count();
///     total_tokens += enum_name.1;
///     total_tokens += enum_name.1;
///
///     let enum_variants = <MyEnum as VariantDescriptors>::variant_names_with_token_counts();
///     total_tokens += enum_variants.iter().map(|(_, token_count)| *token_count).sum::<usize>();
///
///     let json_enum = serde_json::json!({
///         enum_name.0: {
///             "type": "string",
///             "enum": enum_variants.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(),
///             "description": arg_desc,
///         }
///     });
///
///     total_tokens += 11;
///
///     (json_enum, total_tokens)
/// }
/// ```
///
/// Note: It is assumed that the enum implements the `EnumDescriptor`, `VariantDescriptors`, and `FunctionArgument` traits.
/// The actual token count is computed during compile time using these traits' methods.
#[proc_macro]
pub fn generate_enum_info(input: TokenStream) -> TokenStream {
    let enum_ident = parse_macro_input!(input as Ident);

    // When this is consumed by the function that creates the overall function,
    // we are going to be requiring all the arguments, which means we will repeat
    // their names in the "required" part of openai's function schema. So we will
    // count the tokens associated with this enum name twice here.
    let output = quote! {
        {
            use serde_json::Value;
            let mut total_tokens = 0;

            // let (arg_desc, arg_count) = <#enum_ident as ::openai_func_enums::FunctionArgument>::argument_description_with_token_count();
            let (arg_desc, arg_count) = <#enum_ident as EnumDescriptor>::arg_description_with_token_count();
            total_tokens += arg_count;

            let enum_name = <#enum_ident as EnumDescriptor>::name_with_token_count();
            total_tokens += enum_name.1;
            total_tokens += enum_name.1;

            let enum_variants = <#enum_ident as VariantDescriptors>::variant_names_with_token_counts();
            total_tokens += enum_variants.iter().map(|(_, token_count)| *token_count).sum::<usize>();

            let json_enum = serde_json::json!({
                enum_name.0: {
                    "type": "string",
                    "enum": enum_variants.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>(),
                    "description": arg_desc,
                }
            });

            total_tokens += 11;

            (json_enum, total_tokens)
        }
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
/// #[func_description]
/// enum MyEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// Note: The actual usage of the description provided through this attribute happens
/// in the `FunctionCallResponse` derive macro and is retrieved in the `impl_function_call_response` function.
#[proc_macro_attribute]
pub fn func_description(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

/// This procedural macro derives the `FunctionCallResponse` trait for an enum.
///
/// The derive macro expects an enum and it generates a new struct for each variant of the enum.
/// The generated struct is named by appending "Response" to the variant's name. Each struct has the same fields as the variant.
/// Also, a `name`, `to_function_call` and `get_function_json` method is implemented for each struct.
///
/// In the `get_function_json` method, any description provided through the `func_description` attribute is used.
///
/// # Usage
///
/// ```rust
/// #[derive(FunctionCallResponse)]
/// #[func_description]
/// enum MyEnum {
///     Variant1,
///     Variant2,
/// }
/// ```
///
/// Note: This macro can only be applied to enums and it requires the `func_description` attribute to be applied to the enum.
#[proc_macro_derive(FunctionCallResponse, attributes(func_description))]
pub fn derive_function_call_response(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();

    let gen = impl_function_call_response(&ast);

    gen.into()
}

/// This function generates a `FunctionCallResponse` implementation for each variant of an enum.
///
/// For each enum variant, it creates a new struct with the same fields as the variant and also
/// generates `name`, `to_function_call`, and `get_function_json` methods for the struct.
///
/// In the `get_function_json` method, it utilizes the description provided through the `func_description` attribute.
///
/// This function is used by the `FunctionCallResponse` derive macro.
fn impl_function_call_response(ast: &DeriveInput) -> proc_macro2::TokenStream {
    match &ast.data {
        Data::Enum(enum_data) => {
            let mut generated_structs = Vec::new();
            let mut json_generator_functions = Vec::new();

            for variant in &enum_data.variants {
                let variant_name = &variant.ident;
                let struct_name = format_ident!("{}Response", variant_name);

                let mut description = String::new();
                let mut desc_tokens = 0_usize;

                for attr in &variant.attrs {
                    if attr.path().is_ident("func_description") {
                        let attribute_parsed = attr.parse_nested_meta(|meta| {
                            let content = meta.input;

                            while !content.is_empty() {
                                if meta.path.is_ident("description") {
                                    let value = meta.value()?;
                                    if let Ok(Lit::Str(value)) = value.parse() {
                                        description = value.value();
                                    }
                                } else if meta.path.is_ident("tokens") {
                                    let value = meta.value()?;
                                    if let Ok(Lit::Int(value)) = value.parse() {
                                        desc_tokens = value.base10_parse::<usize>()?;
                                        return Ok(());
                                    }
                                }

                                return Ok(());
                            }
                            Err(meta.error("unrecognized my_attribute"))
                        });
                        match attribute_parsed {
                            Ok(_attribute_parsed) => {}
                            Err(e) => {
                                println!("Error parsing attribute:   {:#?}", e);
                            }
                        }
                    }
                }

                let fields: Vec<_> = variant
                    .fields
                    .iter()
                    .map(|f| {
                        let field_name =
                            format_ident!("{}", to_snake_case(&f.ty.to_token_stream().to_string()));
                        let field_type = &f.ty;
                        quote! {
                            pub #field_name: #field_type,
                        }
                    })
                    .collect();

                let field_info: Vec<_> = variant
                    .fields
                    .iter()
                    .map(|f| {
                        let field_type = &f.ty;
                        quote! {
                            generate_enum_info!(#field_type)
                        }
                    })
                    .collect();

                json_generator_functions.push(quote! {
                    impl #struct_name {
                        pub fn name() -> String {
                            stringify!(#struct_name).to_string()
                        }

                        pub fn to_function_call() -> ChatCompletionFunctionCall {
                            let function_call_json = json!({
                                "name": stringify!(#struct_name)
                            });

                            ChatCompletionFunctionCall::Object(function_call_json)
                        }

                        pub fn get_function_json() -> (Value, usize) {
                            let mut parameters = serde_json::Map::new();
                            let mut total_tokens = 0;
                            for (arg_json, arg_tokens) in vec![#(#field_info),*] {
                                total_tokens += arg_tokens;
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

                            let function_json = json!({
                                "name": stringify!(#struct_name),
                                "description": #description,
                                "parameters": {
                                    "type": "object",
                                    "properties": parameters,
                                    "required": parameters.keys().collect::<Vec<_>>()
                                }
                            });

                            total_tokens += 12;
                            total_tokens += #desc_tokens;

                            (function_json, total_tokens)
                        }
                    }
                });

                generated_structs.push(quote! {
                    #[derive(serde::Deserialize, Debug)]
                    #[serde(rename_all = "PascalCase")]
                    pub struct #struct_name {
                        #(#fields)*
                    }
                });
            }

            let gen = quote! {
                #(#generated_structs)*

                #(#json_generator_functions)*

            };

            return gen.into();
        }
        _ => panic!("FunctionCallResponse can only be derived for enums"),
    }
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
    let bpe = cl100k_base().unwrap();
    bpe.encode_ordinary(&text).len()
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
/// # Examplejj
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
