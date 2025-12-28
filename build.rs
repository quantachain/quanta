fn main() {
    // Workaround for wasmer __rust_probestack issue on Linux
    if cfg!(target_os = "linux") {
        // Compile the probestack stub
        cc::Build::new()
            .file("src/probestack_stub.c")
            .compile("probestack_stub");
        
        println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
    }
}
