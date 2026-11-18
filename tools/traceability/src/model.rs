use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone)]
pub struct Package {
    pub name: String,
    pub root: PathBuf,
    pub targets: Vec<Target>,
}
#[derive(Clone)]
pub struct Target {
    pub kind: String,
    pub name: String,
    pub source: PathBuf,
}
pub struct Workspace {
    pub root: PathBuf,
    pub packages: Vec<Package>,
}
impl Workspace {
    pub fn load(root: PathBuf) -> Result<Self, String> {
        let metadata = if let Some(path) = env::var_os("TRACEABILITY_METADATA") {
            fs::read(path).map_err(|error| error.to_string())?
        } else {
            let result = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args(["metadata", "--no-deps", "--format-version=1", "--offline"])
                .current_dir(&root)
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .output()
                .map_err(|error| error.to_string())?;
            if !result.status.success() {
                return Err(String::from_utf8_lossy(&result.stderr).into_owned());
            }
            result.stdout
        };
        let metadata: Value =
            serde_json::from_slice(&metadata).map_err(|error| error.to_string())?;
        let members = metadata["workspace_members"]
            .as_array()
            .ok_or("metadata has no workspace members")?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or("metadata has no packages")?
            .iter()
            .filter(|package| members.contains(&package["id"]))
            .map(|package| {
                Ok(Package {
                    name: package["name"]
                        .as_str()
                        .ok_or("package has no name")?
                        .into(),
                    root: Path::new(
                        package["manifest_path"]
                            .as_str()
                            .ok_or("package has no manifest")?,
                    )
                    .parent()
                    .ok_or("manifest has no parent")?
                    .to_owned(),
                    targets: package["targets"]
                        .as_array()
                        .ok_or("package has no targets")?
                        .iter()
                        .map(|target| {
                            Ok(Target {
                                name: target["name"].as_str().ok_or("target has no name")?.into(),
                                kind: target["kind"][0]
                                    .as_str()
                                    .ok_or("target has no kind")?
                                    .into(),
                                source: target["src_path"]
                                    .as_str()
                                    .ok_or("target has no source")?
                                    .into(),
                            })
                        })
                        .collect::<Result<_, String>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { root, packages })
    }
    pub fn package_at(&self, path: &Path) -> Option<&Package> {
        self.packages
            .iter()
            .filter(|package| path.starts_with(&package.root))
            .max_by_key(|package| package.root.components().count())
    }
    pub fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

#[derive(Serialize)]
pub struct Unit {
    pub package: String,
    pub crate_name: String,
    pub target_kind: String,
    pub target_name: String,
    pub is_test: bool,
    pub source: String,
}
#[derive(Clone, Serialize)]
pub struct Api {
    pub id: String,
    pub package: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub line: usize,
}
#[derive(Serialize)]
pub struct Call {
    pub caller: String,
    pub caller_package: String,
    pub callee: String,
    pub path: String,
    pub line: usize,
    pub dispatch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expansion: Option<Location>,
}
#[derive(Clone, Serialize)]
pub struct Location {
    pub path: String,
    pub line: usize,
}
#[derive(Serialize)]
pub struct Test {
    pub package: String,
    pub target_kind: String,
    pub target_name: String,
    pub name: String,
    pub path: String,
    pub line: usize,
    pub targets: Vec<String>,
}
#[derive(Serialize)]
pub struct Unresolved {
    pub caller: String,
    pub path: String,
    pub line: usize,
    pub reason: String,
}
#[derive(Default, Serialize)]
pub struct Coverage {
    pub bodies: usize,
    pub calls: usize,
    pub workspace_calls: usize,
    pub external_calls: usize,
    pub unresolved_calls: usize,
    pub function_references: usize,
    pub implicit_drops: usize,
    pub assertions: usize,
    pub inline_assembly: usize,
}
#[derive(Serialize)]
pub struct Shard {
    pub generated_sources: BTreeMap<String, String>,
    pub version: u32,
    pub unit: Unit,
    pub apis: Vec<Api>,
    pub calls: Vec<Call>,
    pub references: Vec<Call>,
    pub tests: Vec<Test>,
    pub unresolved: Vec<Unresolved>,
    pub coverage: Coverage,
}
