use crate::config::bootstrap::{
    LUARC_JSON, META_LUA, STARTER_LUA, bootstrap_config, write_editor_support,
};

struct CleanupDir(std::path::PathBuf);
impl Drop for CleanupDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn temp_config_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("dome_{tag}_{nanos}"))
}

#[test]
fn bootstrap_writes_starter_config_when_missing() {
    let dir = temp_config_dir("bootstrap_missing");
    let _cleanup = CleanupDir(dir.clone());
    let path = dir.join("config.lua");

    bootstrap_config(path.to_str().unwrap());

    assert_eq!(std::fs::read_to_string(&path).unwrap(), STARTER_LUA);
}

#[test]
fn bootstrap_leaves_an_existing_config_untouched() {
    let dir = temp_config_dir("bootstrap_existing");
    let _cleanup = CleanupDir(dir.clone());
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.lua");
    std::fs::write(&path, "return {}\n").unwrap();

    bootstrap_config(path.to_str().unwrap());

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "return {}\n");
}

#[test]
fn editor_support_writes_the_meta_and_luarc() {
    let dir = temp_config_dir("editor_support");
    let _cleanup = CleanupDir(dir.clone());
    let config = dir.join("config.lua");

    write_editor_support(config.to_str().unwrap());

    assert_eq!(
        std::fs::read_to_string(dir.join("dome.meta.lua")).unwrap(),
        META_LUA
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(".luarc.json")).unwrap(),
        LUARC_JSON
    );
}

#[test]
fn editor_support_refreshes_the_meta_but_keeps_the_luarc() {
    let dir = temp_config_dir("editor_support_refresh");
    let _cleanup = CleanupDir(dir.clone());
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.lua");
    std::fs::write(dir.join("dome.meta.lua"), "-- stale\n").unwrap();
    let customized = "{ \"custom\": true }\n";
    std::fs::write(dir.join(".luarc.json"), customized).unwrap();

    write_editor_support(config.to_str().unwrap());

    assert_eq!(
        std::fs::read_to_string(dir.join("dome.meta.lua")).unwrap(),
        META_LUA
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(".luarc.json")).unwrap(),
        customized
    );
}
