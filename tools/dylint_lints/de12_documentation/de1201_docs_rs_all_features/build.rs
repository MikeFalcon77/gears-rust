const ENV_EXCLUDED_CRATES: &str = "DE1201_DOCS_RS_ALL_FEATURES_EXCLUDED_CRATES";

fn main() {
    println!("cargo:rerun-if-env-changed={ENV_EXCLUDED_CRATES}");

    if let Ok(value) = std::env::var(ENV_EXCLUDED_CRATES) {
        println!("cargo:rustc-env={ENV_EXCLUDED_CRATES}={value}");
    }
}
