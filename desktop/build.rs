// The Windows build carries rippl's icon in rippl.exe itself.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("../packaging/windows/rippl.rc", embed_resource::NONE).manifest_optional().ok();
    }
}
