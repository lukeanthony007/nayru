fn main() {
    tauri_build::build();

    // Copy onnxruntime.dll next to the output binary so koko can find it at runtime.
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // OUT_DIR is like target/debug/build/nayru-app-xxx/out — walk up to target/debug
    let mut target_dir = std::path::PathBuf::from(&out_dir);
    for _ in 0..3 {
        target_dir.pop();
    }

    let dll_src = std::path::Path::new("binaries/onnxruntime.dll");
    let dll_dst = target_dir.join("onnxruntime.dll");
    if dll_src.exists() && !dll_dst.exists() {
        let _ = std::fs::copy(dll_src, &dll_dst);
    }

    println!("cargo:rerun-if-changed=binaries/onnxruntime.dll");
}
