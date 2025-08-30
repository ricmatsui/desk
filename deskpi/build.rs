#[cfg(feature = "reloader")]
use {std::env, std::path::PathBuf};

fn main() {
    #[cfg(feature = "reloader")]
    {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

        let raylib_path = PathBuf::from(manifest_dir)
            .join("raylib-build")
            .join("raylib");

        println!(
            "cargo:rustc-link-search=native={}",
            raylib_path.to_str().unwrap()
        );
        println!("cargo:rustc-link-lib=raylib");
    }
}
