fn main() {
    // If you want to use the embedding-related functionality, look at the
    // clap integration example. You need to set FUNC_ENUMS_EMBED_PATH
    // and FUNC_ENUMS_EMBED_MODEL and enable feature "function_filtering"
    let max_response_tokens = 1000_u16;
    println!(
        "cargo:warning=FUNC_ENUMS_MAX_RESPONSE_TOKENS set to: {}",
        max_response_tokens
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_MAX_RESPONSE_TOKENS={}",
        max_response_tokens
    );

    let max_request_tokens = 4191_usize;
    println!(
        "cargo:warning=FUNC_ENUMS_MAX_REQUEST_TOKENS set to: {}",
        max_request_tokens
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_MAX_REQUEST_TOKENS={}",
        max_request_tokens
    );

    let max_func_tokens = 500_u16;
    println!(
        "cargo:warning=FUNC_ENUMS_MAX_FUNC_TOKENS set to: {}",
        max_func_tokens
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_MAX_FUNC_TOKENS={}",
        max_func_tokens
    );

    // This currently doesn't do anything but it will soon. If you don't 
    // ever want this to come into play just set it high.
    let max_single_arg_tokens = 20u16;
    println!(
        "cargo:warning=FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS set to: {}",
        max_single_arg_tokens
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS={}",
        max_single_arg_tokens
    );
}
