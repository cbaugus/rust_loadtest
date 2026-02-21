fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/loadtest.proto");

    // Use protox (pure-Rust protobuf compiler) so no system `protoc` is needed.
    // Install via: cargo add --build protox  (already in [build-dependencies])
    let fds = protox::compile(["proto/loadtest.proto"], ["proto/"])?;

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_fds(fds)?;

    Ok(())
}
