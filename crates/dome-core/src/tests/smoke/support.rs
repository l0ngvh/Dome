use crate::core::{SizeConstraint, Strategy, TilingConfig, WindowMatcher};
use crate::tests::fixtures::TestDome;

pub(super) fn export_then_apply(dome: &mut TestDome) {
    let layouts = dome.capture_live_layouts();
    dome.apply_preferred_layouts(layouts);
}

pub(super) fn lua_string(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            character if character.is_ascii_control() => {
                result.push_str(&format!("\\{:03}", character as u32));
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

pub(super) fn strategy_name(strategy: Strategy) -> &'static str {
    match strategy {
        Strategy::PartitionTree => "partition_tree",
        Strategy::Master => "master",
        Strategy::Scrolling => "scrolling",
    }
}

fn size_constraint(value: SizeConstraint) -> String {
    match value {
        SizeConstraint::Pixels(value) => value.value().to_string(),
        SizeConstraint::Percent(value) => lua_string(&format!("{value}%")),
    }
}

pub(super) fn matcher_source(matcher: &WindowMatcher) -> String {
    let fields = [
        ("app", &matcher.app),
        ("bundle_id", &matcher.bundle_id),
        ("title", &matcher.title),
        ("process", &matcher.process),
        ("class", &matcher.class),
        ("aumid", &matcher.aumid),
    ]
    .into_iter()
    .filter_map(|(key, value)| {
        value
            .as_ref()
            .map(|value| format!("{key} = {}", lua_string(value)))
    })
    .collect::<Vec<_>>();
    format!("{{ {} }}", fields.join(", "))
}

pub(super) fn tiling_source(tiling: &TilingConfig) -> String {
    let constraints = &tiling.size_constraints;
    let ignore = tiling
        .ignore
        .iter()
        .map(matcher_source)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "return {{
            layout = '{}', border_size = {},
            partition_tree = {{ tab_bar_height = {}, automatic_tiling = {} }},
            master = {{ master_ratio = {}, master_count = {} }},
            scrolling = {{ column_width = {} }},
            minimum_width = {}, minimum_height = {}, maximum_width = {}, maximum_height = {},
            ignore = {{ {ignore} }},
        }}",
        strategy_name(tiling.layout),
        tiling.border_size.value(),
        tiling.partition_tree.tab_bar_height.value(),
        tiling.partition_tree.automatic_tiling,
        tiling.master.master_ratio,
        tiling.master.master_count,
        size_constraint(tiling.scrolling.column_width),
        size_constraint(constraints.minimum_width),
        size_constraint(constraints.minimum_height),
        size_constraint(constraints.maximum_width),
        size_constraint(constraints.maximum_height),
    )
}

pub(super) fn title_matchers(titles: &[String]) -> Vec<WindowMatcher> {
    titles
        .iter()
        .map(|title| WindowMatcher {
            title: Some(title.clone()),
            ..WindowMatcher::default()
        })
        .collect()
}
