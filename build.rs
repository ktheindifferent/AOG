use std::{
    env, fs,
    io::{Read, Write},
    path::Path,
};
fn add_tree(zip: &mut zip::ZipWriter<fs::File>, root: &Path, prefix: &str) {
    let mut entries: Vec<_> = fs::read_dir(root)
        .expect("asset directory")
        .map(Result::unwrap)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            add_tree(zip, &path, &name);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            zip.start_file(
                name,
                zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            let mut bytes = Vec::new();
            fs::File::open(path)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            zip.write_all(&bytes).unwrap();
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=www");
    println!("cargo:rerun-if-changed=firmware/package");
    let file =
        fs::File::create(Path::new(&env::var_os("OUT_DIR").unwrap()).join("assets.zip")).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    add_tree(&mut zip, Path::new("www"), "www");
    add_tree(&mut zip, Path::new("firmware/package"), "firmware");
    zip.finish().unwrap();
}
