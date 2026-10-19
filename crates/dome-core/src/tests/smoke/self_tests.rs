use super::*;

#[test]
fn capture_panic_concurrently_keeps_each_threads_signature_and_restores_capture() {
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let workers: Vec<_> = ["first capture", "second capture"]
            .into_iter()
            .map(|message| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let signature = capture_panic(|| panic!("{message}"))
                        .expect("the invoking thread panicked");
                    assert_eq!(signature.normalized_payload, message);
                    assert!(signature.file.ends_with("self_tests.rs"));
                    assert!(signature.line > 0);
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    });
    assert!(capture_panic(|| {}).is_none());
    assert_eq!(
        capture_panic(|| panic!("next capture"))
            .unwrap()
            .normalized_payload,
        "next capture"
    );
}

use crate::tests::fixtures::configuration::load_config;

#[test]
fn load_and_reload_preserve_recorded_tiling_values() {
    let mut dome = SmokeStrategy::PartitionTree.build_dome();
    for strategy in SmokeStrategy::all() {
        let mut tiling = initial_tiling(*strategy);
        let source = tiling_source(&tiling);
        let mut expected = tiling.clone();
        expected
            .ignore
            .extend(crate::config::Config::default_ignore());
        assert_eq!(load_config(&source).config.tiling, expected);
        assert_eq!(dome.reload(&source).unwrap().tiling, expected);

        tiling.border_size = Length::new(0);
        tiling.partition_tree.tab_bar_height = Length::new(49);
        tiling.partition_tree.automatic_tiling = true;
        tiling.master.master_ratio = 0.234_567_9;
        tiling.master.master_count = 4;
        tiling.scrolling.column_width = SizeConstraint::Percent(33.333332);
        tiling.size_constraints = SizeConstraints {
            minimum_width: SizeConstraint::Pixels(Length::new(199)),
            minimum_height: SizeConstraint::Percent(12.345678),
            maximum_width: SizeConstraint::Percent(99.99999),
            maximum_height: SizeConstraint::Pixels(Length::new(0)),
        };
        let text = "quote\" slash\\ newline\n\r\t\0\u{7f}7 日本語".to_owned();
        tiling.ignore = vec![WindowMatcher {
            title: Some(text.clone()),
            app: Some(text.clone()),
            process: Some(text.clone()),
            bundle_id: Some(text.clone()),
            class: Some(text.clone()),
            aumid: Some(text),
        }];
        let source = tiling_source(&tiling);
        let mut expected = tiling.clone();
        expected
            .ignore
            .extend(crate::config::Config::default_ignore());
        assert_eq!(load_config(&source).config.tiling, expected);
        assert_eq!(dome.reload(&source).unwrap().tiling, expected);
    }
}

#[test]
fn apply_recorded_layouts_on_each_monitor_matches_file_layout_behavior() {
    let title = "title\"\\\n\r\t\0\u{7f}7 日本語".to_owned();
    let monitor_name = "display\"\\\n\x007";
    let tree_ops = vec![
        PrefTreeBuildOp::InsertLeaf {
            leaf_id: 0,
            title: title.clone(),
            anchor: None,
            split: SplitMode::Horizontal,
        },
        PrefTreeBuildOp::InsertLeaf {
            leaf_id: 1,
            title: "second".into(),
            anchor: Some(0),
            split: SplitMode::Vertical,
        },
        PrefTreeBuildOp::InsertLeaf {
            leaf_id: 2,
            title: "third".into(),
            anchor: Some(1),
            split: SplitMode::Tabbed,
        },
        PrefTreeBuildOp::InsertLeaf {
            leaf_id: 3,
            title: title.clone(),
            anchor: Some(CONTAINER_BASE),
            split: SplitMode::Horizontal,
        },
    ];
    for (strategy, tiling) in [
        (
            Strategy::PartitionTree,
            "layout = 'partition_tree', tree = { split = 'vertical', children = {
                { title = title },
                { split = 'tabbed', children = {{ title = 'second' }, { title = 'third' }} },
                { title = title },
            } }",
        ),
        (
            Strategy::Master,
            "layout = 'master', master = {{ title = title }, { title = title }},
             secondary = {{ title = 'second' }, { title = 'third' }}",
        ),
        (
            Strategy::Scrolling,
            "layout = 'scrolling', columns = {
                { children = {{ title = title }, { title = title }} },
                { children = {} },
                { children = {{ title = 'second' }, { title = 'third' }} },
            }",
        ),
    ] {
        let mut dome = SmokeStrategy::PartitionTree.build_dome();
        let mut reference = SmokeStrategy::PartitionTree.build_dome();
        for instance in [&mut dome, &mut reference] {
            instance.add_monitor(monitor(monitor_name, [150, 0, 150, 30]));
        }
        let op = RecordedOp::ApplyPreferredLayouts {
            workspace_name: title.clone(),
            strategy,
            tree_ops: tree_ops.clone(),
            master: vec![title.clone(), title.clone()],
            secondary: vec!["second".into(), "third".into()],
            columns: vec![
                vec![title.clone(), title.clone()],
                Vec::new(),
                vec!["second".into(), "third".into()],
            ],
            float: vec!["floating".into(), "shared".into()],
            fullscreen: vec!["fullscreen".into(), "shared".into()],
        };
        dome.apply_preferred_layouts(preferred_layout_everywhere(&dome, &op));
        reference.apply_layout(&format!(
            r#"
            local title = "title\"\\\n\r\t\000\1277 日本語"
            local workspace = {{ {tiling},
                float = {{{{ title = 'floating' }}, {{ title = 'shared' }}}},
                fullscreen = {{{{ title = 'fullscreen' }}, {{ title = 'shared' }}}},
            }}
            return {{
                primary = {{ [title] = workspace }},
                ["display\"\\\n\0007"] = {{ [title] = workspace }},
            }}
            "#
        ));
        for (monitor_index, monitor_name) in ["primary", monitor_name].into_iter().enumerate() {
            for instance in [&mut dome, &mut reference] {
                instance.run_action(Action::Focus {
                    target: FocusTarget::Workspace {
                        name: title.clone(),
                        monitor: Some(monitor_name.to_owned()),
                    },
                });
            }
            for (index, window_title) in [
                "third",
                "second",
                &title,
                &title,
                "unmatched",
                "floating",
                "floating",
                "fullscreen",
                "shared",
                "shared",
            ]
            .into_iter()
            .enumerate()
            {
                let label = format!("monitor-{monitor_index}-window-{index}");
                for instance in [&mut dome, &mut reference] {
                    instance.open(window(window_title).label(&label).frame([
                        150 * monitor_index as i32 + 10,
                        2,
                        30,
                        8,
                    ]));
                }
                assert_eq!(
                    dome.snapshot(),
                    reference.snapshot(),
                    "{strategy:?} on {monitor_name:?} after opening {label}"
                );
            }
            for command in ["focus left", "focus right", "focus up", "focus down"] {
                dome.run(command);
                reference.run(command);
                assert_eq!(
                    dome.snapshot(),
                    reference.snapshot(),
                    "{strategy:?}: {command}"
                );
            }
        }
    }
}

#[test]
fn replay_skips_missing_producers_and_allows_repeated_titles() {
    let operations = vec![
        RecordedOp::DeleteWindow {
            window: RecordedWindow(100),
        },
        RecordedOp::RemoveMonitor {
            monitor: RecordedMonitor(100),
        },
        RecordedOp::SetFocus {
            window: RecordedWindow(100),
        },
        RecordedOp::InsertTiling {
            producer_id: 1,
            title: None,
        },
        RecordedOp::InsertTiling {
            producer_id: 3,
            title: None,
        },
        RecordedOp::DeleteWindow {
            window: RecordedWindow(1),
        },
        RecordedOp::DeleteWindow {
            window: RecordedWindow(1),
        },
        RecordedOp::InsertTiling {
            producer_id: 5,
            title: None,
        },
        RecordedOp::ExportLayout,
    ];
    replay_without_capture(&operations, master_dome);
}

/// The reducer only works when a seed replays to the same op sequence every
/// time. A process-randomized order in generation, such as iterating a
/// `HashMap` or `HashSet`, breaks that silently.
#[test]
fn same_seed_records_the_same_ops() {
    const DETERMINISM_OPS: usize = 4000;
    for seed in [42u64, 149, 167, 777, 1234] {
        let strategy = strategy_for_seed(seed);
        let first = record_ops(seed, DETERMINISM_OPS, strategy);
        let second = record_ops(seed, DETERMINISM_OPS, strategy);
        assert_eq!(
            format!("{first:?}"),
            format!("{second:?}"),
            "seed {seed} ({strategy:?}) recorded a different op sequence on a second run"
        );
    }
}

fn master_dome() -> TestDome {
    SmokeStrategy::Master.build_dome()
}
