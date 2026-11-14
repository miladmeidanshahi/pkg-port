fn main() {
    println!("cargo:rerun-if-changed=pkgport.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("pkgport.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("failed to embed the Windows application icon");
    }
}
