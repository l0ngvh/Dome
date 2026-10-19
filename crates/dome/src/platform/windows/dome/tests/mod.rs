mod actions;
mod admission;
mod fixture;
mod focus;
mod fullscreen;
mod ipc;
mod placement_drift;
mod scrolling;
mod title;

const PARTITION_CONFIG: &str = r#"
return {
    layout = "partition_tree",
    border_size = 1,
    minimum_width = 1,
    minimum_height = 1,
    partition_tree = { automatic_tiling = false, tab_bar_height = 2 },
    reserved_area = function(monitor) return nil end,
}
"#;
