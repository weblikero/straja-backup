fn main() {
    println!("cargo:rerun-if-changed=native/vss.cpp");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            // POD + Windows heap only: no exceptions, RTTI or additional C++ runtime DLL.
            .cpp_link_stdlib(None)
            .flag_if_supported("/EHs-c-")
            .flag_if_supported("/GR-")
            .flag_if_supported("/Zl")
            .file("native/vss.cpp")
            .flag_if_supported("/std:c++17")
            .compile("straja_vss");
        for library in ["vssapi", "ole32", "oleaut32", "advapi32"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}
