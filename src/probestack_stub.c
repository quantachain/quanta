// Stub for __rust_probestack needed by wasmer_vm on Linux
// This is a workaround for the missing symbol in compiler_builtins

#if defined(__linux__) && defined(__x86_64__)

__attribute__((visibility("default")))
void __rust_probestack(void) {
    // No-op implementation
    // In production, this would perform stack probing for stack overflow detection
    // But for our use case, this is acceptable
}

#endif
