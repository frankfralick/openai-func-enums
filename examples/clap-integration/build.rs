use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // This is the path to where embeddings will be stored.
    let relative_path = PathBuf::from("../embedding/function_embeddings.bin");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let absolute_path = PathBuf::from(manifest_dir).join(relative_path);

    if let Some(parent) = absolute_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).expect("Failed to create directories");
        }
    }

    // The two variables FUNC_ENUMS_EMBED_PATH and FUNC_ENUMS_EMBED_MODLE
    // are required for using the embedding-related functionality. You also
    // need to enable the "function_filtering" feature.
    println!(
        "cargo:warning=FUNC_ENUMS_EMBED_PATH set to: {}",
        absolute_path.display()
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_EMBED_PATH={}",
        absolute_path.display()
    );

    let embedding_model = "text-embedding-3-small";
    println!(
        "cargo:warning=FUNC_ENUMS_EMBED_MODEL set to: {}",
        embedding_model
    );
    println!("cargo:rustc-env=FUNC_ENUMS_EMBED_MODEL={}", embedding_model);

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
    let max_single_arg_tokens = 20_u16;
    println!(
        "cargo:warning=FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS set to: {}",
        max_single_arg_tokens
    );
    println!(
        "cargo:rustc-env=FUNC_ENUMS_MAX_SINGLE_ARG_TOKENS={}",
        max_single_arg_tokens
    );
}
