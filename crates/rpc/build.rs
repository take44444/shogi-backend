//! Generates Rust protobuf messages and Tonic clients and servers with vendored protoc.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure().compile_with_config(
        config,
        &["../../proto/game.proto"],
        &["../../proto"],
    )?;
    println!("cargo:rerun-if-changed=../../proto/game.proto");
    Ok(())
}
