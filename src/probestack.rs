// Stub implementation of __rust_probestack for wasmer_vm on Linux
// This is needed because wasmer_vm references this symbol but it's not provided by compiler_builtins

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[no_mangle]
pub extern "C" fn __rust_probestack() {
    // No-op implementation
    // In production builds with proper compiler settings, this would perform
    // stack probing for stack overflow detection. For our use case with wasmer,
    // this stub is sufficient.
}
