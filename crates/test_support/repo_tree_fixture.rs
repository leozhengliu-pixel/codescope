use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub struct CanonicalRepoTreeRoot {
    pub root: PathBuf,
}

impl CanonicalRepoTreeRoot {
    pub fn create(
        label: &str,
        readme_contents: &str,
        main_rs_contents: &str,
        generated_path: &str,
    ) -> Self {
        validate_generated_path(generated_path);

        let root = unique_temp_dir(label);
        fs::create_dir_all(root.join("src")).unwrap();
        let generated_full_path = root.join(generated_path);
        fs::create_dir_all(generated_full_path.parent().unwrap()).unwrap();
        fs::write(root.join("README.md"), readme_contents).unwrap();
        fs::write(root.join("src").join("main.rs"), main_rs_contents).unwrap();
        fs::write(&generated_full_path, generated_fixture_contents(generated_path)).unwrap();

        Self { root }
    }
}

pub fn unique_temp_dir(label: &str) -> PathBuf {
    let suffix = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sourcebot-{label}-test-{nanos}-{suffix}"))
}

pub fn assert_common_layout(root: &Path, generated_path: &str) {
    assert!(root.join("README.md").is_file(), "missing README.md");
    assert!(
        root.join("src").join("main.rs").is_file(),
        "missing src/main.rs"
    );
    assert!(
        root.join(generated_path).is_file(),
        "missing {generated_path}"
    );
}

fn validate_generated_path(generated_path: &str) {
    let path = Path::new(generated_path);
    assert!(
        !path.is_absolute(),
        "generated_path must be relative under target/: {generated_path}"
    );

    let mut components = path.components();
    assert!(
        matches!(components.next(), Some(Component::Normal(first)) if first == "target"),
        "generated_path must stay under target/: {generated_path}"
    );

    let mut saw_child = false;
    for component in components {
        match component {
            Component::Normal(_) => saw_child = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                panic!("generated_path must stay under target/: {generated_path}");
            }
        }
    }

    assert!(
        saw_child,
        "generated_path must include a file under target/: {generated_path}"
    );
}

fn generated_fixture_contents(generated_path: &str) -> &'static str {
    match generated_path {
        "target/generated.rs" => "pub fn generated() {}\n",
        "target/generated.txt" => "generated placeholder\n",
        _ => "generated fixture placeholder\n",
    }
}
