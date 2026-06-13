use std::path::{Path, PathBuf};

pub(crate) const TARGET_DEPLOY_DIR: &str = "target/deploy";
pub(crate) const TARGET_IDL_DIR: &str = "target/idl";
pub(crate) const TARGET_TYPES_DIR: &str = "target/types";

pub(crate) fn deploy_dir(root: &Path) -> PathBuf {
    root.join(TARGET_DEPLOY_DIR)
}

pub(crate) fn idl_dir(root: &Path) -> PathBuf {
    root.join(TARGET_IDL_DIR)
}

pub(crate) fn types_dir(root: &Path) -> PathBuf {
    root.join(TARGET_TYPES_DIR)
}

pub(crate) fn deploy_file(root: &Path, program_name: &str) -> PathBuf {
    deploy_dir(root).join(format!("{program_name}.so"))
}

pub(crate) fn keypair_file(root: &Path, program_name: &str) -> PathBuf {
    deploy_dir(root).join(format!("{program_name}-keypair.json"))
}

pub(crate) fn idl_file(root: &Path, program_name: &str) -> PathBuf {
    idl_dir(root).join(format!("{program_name}.json"))
}

pub(crate) fn typescript_file(root: &Path, program_name: &str) -> PathBuf {
    types_dir(root).join(format!("{program_name}.ts"))
}
