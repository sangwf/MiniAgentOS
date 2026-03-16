use std::path::Path;

fn main() {
    let mbedtls_dir = Path::new("third_party/mbedtls");
    let lib_dir = mbedtls_dir.join("library");

    let mut build = cc::Build::new();
    build
        .include(mbedtls_dir.join("include"))
        .include("c_headers")
        .include(".")
        .define("MBEDTLS_CONFIG_FILE", Some("\"mbedtls_config.h\""))
        .flag("-ffreestanding")
        .flag("-fno-builtin")
        .flag("-mstrict-align")
        .flag("-mgeneral-regs-only")
        .flag("-fno-vectorize")
        .flag("-fno-slp-vectorize");

    let entries = std::fs::read_dir(&lib_dir).expect("mbedtls library dir");
    for entry in entries {
        let entry = entry.expect("mbedtls entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("c") {
            build.file(path);
        }
    }
    build.file("mbedtls_shims.c");
    build.file("mbedtls_wrap.c");
    build.file("mbedtls_diag.c");

    build.compile("mbedtls");

    println!("cargo:rerun-if-changed=mbedtls_config.h");
    println!("cargo:rerun-if-changed=mbedtls_shims.c");
    println!("cargo:rerun-if-changed=mbedtls_wrap.c");
    println!("cargo:rerun-if-changed=mbedtls_diag.c");
    println!("cargo:rerun-if-changed=c_headers");
    println!("cargo:rerun-if-changed=third_party/mbedtls");
    println!("cargo:rerun-if-changed=xsecret.txt");
    println!("cargo:rerun-if-env-changed=OPENAI_API_KEY");
    println!("cargo:rerun-if-env-changed=X_BEARER_TOKEN");
    println!("cargo:rerun-if-env-changed=X_CONSUMER_KEY");
    println!("cargo:rerun-if-env-changed=X_CONSUMER_KEY_SECRET");
    println!("cargo:rerun-if-env-changed=X_ACCESS_TOKEN");
    println!("cargo:rerun-if-env-changed=X_ACCESS_TOEKN");
    println!("cargo:rerun-if-env-changed=X_ACCESS_TOKEN_SECRET");

    let secrets = std::fs::read_to_string("xsecret.txt").unwrap_or_default();
    let mut bearer_token = String::new();
    let mut api_key = String::new();
    let mut api_secret = String::new();
    let mut access_token = String::new();
    let mut access_secret = String::new();
    for line in secrets.lines() {
        if let Some(v) = line.strip_prefix("Bearer Token:") {
            bearer_token = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("API Key:") {
            api_key = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("API Secret:") {
            api_secret = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Access Token:") {
            access_token = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Access Token Secret:") {
            access_secret = v.trim().to_string();
        }
    }

    if api_key.is_empty() {
        api_key = env_trimmed("X_CONSUMER_KEY");
    }
    if bearer_token.is_empty() {
        bearer_token = env_trimmed("X_BEARER_TOKEN");
    }
    if api_secret.is_empty() {
        api_secret = env_trimmed("X_CONSUMER_KEY_SECRET");
    }
    if access_token.is_empty() {
        access_token = env_trimmed("X_ACCESS_TOKEN");
    }
    if access_token.is_empty() {
        access_token = env_trimmed("X_ACCESS_TOEKN");
    }
    if access_secret.is_empty() {
        access_secret = env_trimmed("X_ACCESS_TOKEN_SECRET");
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = Path::new(&out_dir).join("x_secrets.rs");
    let mut out = String::new();
    out.push_str("pub static X_BEARER_TOKEN: &[u8] = b\"");
    out.push_str(&escape_bytes(&bearer_token));
    out.push_str("\";\n");
    out.push_str("pub static X_API_KEY: &[u8] = b\"");
    out.push_str(&escape_bytes(&api_key));
    out.push_str("\";\n");
    out.push_str("pub static X_API_SECRET: &[u8] = b\"");
    out.push_str(&escape_bytes(&api_secret));
    out.push_str("\";\n");
    out.push_str("pub static X_ACCESS_TOKEN: &[u8] = b\"");
    out.push_str(&escape_bytes(&access_token));
    out.push_str("\";\n");
    out.push_str("pub static X_ACCESS_SECRET: &[u8] = b\"");
    out.push_str(&escape_bytes(&access_secret));
    out.push_str("\";\n");
    out.push_str("pub const X_BEARER_TOKEN_READY: bool = ");
    out.push_str(&(!bearer_token.is_empty()).to_string());
    out.push_str(";\n");
    out.push_str("pub const X_SECRETS_READY: bool = ");
    out.push_str(&( !api_key.is_empty()
        && !api_secret.is_empty()
        && !access_token.is_empty()
        && !access_secret.is_empty()).to_string());
    out.push_str(";\n");
    std::fs::write(out_path, out).expect("write x_secrets.rs");

    let openai_api_key = std::env::var("OPENAI_API_KEY")
        .unwrap_or_default()
        .trim()
        .to_string();
    let openai_out_path = Path::new(&out_dir).join("openai_secrets.rs");
    let mut openai_out = String::new();
    openai_out.push_str("pub static OPENAI_EMBEDDED_API_KEY: &[u8] = b\"");
    openai_out.push_str(&escape_bytes(&openai_api_key));
    openai_out.push_str("\";\n");
    openai_out.push_str("pub const OPENAI_EMBEDDED_API_KEY_READY: bool = ");
    openai_out.push_str(&(!openai_api_key.is_empty()).to_string());
    openai_out.push_str(";\n");
    std::fs::write(openai_out_path, openai_out).expect("write openai_secrets.rs");
}

fn escape_bytes(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            _ => out.push(b as char),
        }
    }
    out
}

fn env_trimmed(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_default()
        .trim()
        .to_string()
}
