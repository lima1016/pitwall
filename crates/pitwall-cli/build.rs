//! rust-embed는 컴파일 시점에 `web/dist` 폴더가 있어야 한다.
//! 프론트를 아직 빌드하지 않은 상태에서도 `cargo build`가 되도록 빈 폴더를 만들어 둔다.
//! 폴더가 비어 있으면 서버는 안내 페이지를 보여 준다 (embed.rs).

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(&dist).expect("failed to create web/dist placeholder");
    }
    println!("cargo:rerun-if-changed=../../web/dist");
}
