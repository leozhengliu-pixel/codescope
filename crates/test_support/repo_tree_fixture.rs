use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::symlink;

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

    #[allow(dead_code)]
    pub fn add_search_ignored_and_binary_variants(&self) {
        fs::create_dir_all(self.root.join(".git")).unwrap();
        fs::write(
            self.root.join(".git").join("HEAD"),
            "build_router should be ignored\n",
        )
        .unwrap();
        fs::write(
            self.root.join("target").join("generated.txt"),
            "build_router should also be ignored\n",
        )
        .unwrap();
        fs::write(self.root.join("image.png"), b"not really an image").unwrap();
        fs::write(self.root.join("binary.dat"), [0_u8, 159, 146, 150]).unwrap();
    }

    #[cfg(unix)]
    #[allow(dead_code)]
    pub fn add_browse_symlink_variants(&self) -> PathBuf {
        symlink(
            self.root.join("README.md"),
            self.root.join("src").join("readme-link.rs"),
        )
        .unwrap();

        let outside_path = self.root.parent().unwrap().join("outside-secret.txt");
        fs::write(&outside_path, "secret generated token\n").unwrap();
        symlink(&outside_path, self.root.join("src").join("outside-secret.rs")).unwrap();

        outside_path
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
