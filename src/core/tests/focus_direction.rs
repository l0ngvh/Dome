use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::{setup_logger, snapshot};
use insta::assert_snapshot;

#[test]
fn focus_left_right_in_horizontal_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.insert_window();

    hub.focus_left();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
        )
      )
    )
    ");

    hub.focus_right();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=3.33, y=0.00, w=3.33, h=10.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=6.67, y=0.00, w=3.33, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_up_down_in_vertical_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.insert_window();

    hub.focus_up();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");

    hub.focus_down();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=3.33)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=3.33, w=10.00, h=3.33)
          Window(id=WindowId(2), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");
}

#[test]
fn focus_right_selects_first_child_of_next_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus w0
    hub.focus_left();

    // focus_right should select w2 (first child of first nested container)
    hub.focus_right();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(3),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.33, y=0.00, w=6.67, h=10.00, direction=Vertical,
            Container(id=ContainerId(2), parent=ContainerId(1), x=3.33, y=0.00, w=6.67, h=5.00, direction=Horizontal,
              Window(id=WindowId(1), parent=ContainerId(2), x=3.33, y=0.00, w=3.33, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=6.67, y=0.00, w=3.33, h=5.00)
            )
            Window(id=WindowId(2), parent=ContainerId(1), x=3.33, y=5.00, w=6.67, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn focus_left_selects_last_child_of_previous_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, w1] [w2]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // focus_left from w2 should select w1 (last child of previous container)
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_left_from_nested_container_goes_to_grandparent_previous() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, [w1, [w2, w3]]]
    hub.insert_window();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();

    hub.focus_left();
    hub.focus_left();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=3.33, h=10.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=3.33, y=0.00, w=6.67, h=10.00, direction=Vertical,
            Window(id=WindowId(1), parent=ContainerId(1), x=3.33, y=0.00, w=6.67, h=5.00)
            Container(id=ContainerId(2), parent=ContainerId(1), x=3.33, y=5.00, w=6.67, h=5.00, direction=Horizontal,
              Window(id=WindowId(2), parent=ContainerId(2), x=3.33, y=5.00, w=3.33, h=5.00)
              Window(id=WindowId(3), parent=ContainerId(2), x=6.67, y=5.00, w=3.33, h=5.00)
            )
          )
        )
      )
    )
    ");
}

#[test]
fn focus_down_from_nested_container_goes_to_grandparent_next() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [[[w0, w1], w2], w3]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_left();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_down();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=6.67, direction=Horizontal,
            Container(id=ContainerId(2), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=6.67, direction=Vertical,
              Window(id=WindowId(0), parent=ContainerId(2), x=0.00, y=0.00, w=5.00, h=3.33)
              Window(id=WindowId(3), parent=ContainerId(2), x=0.00, y=3.33, w=5.00, h=3.33)
            )
            Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=6.67)
          )
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=6.67, w=10.00, h=3.33)
        )
      )
    )
    ");
}

#[test]
fn focus_right_from_last_child_goes_to_next_sibling_in_parent() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    // Create: [w0, w1] [w2]
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_parent();
    hub.toggle_new_window_direction();
    hub.insert_window();

    // Focus w1 (last in nested container)
    hub.focus_left();

    // focus_right from w1 should go to w2 (next sibling in parent)
    hub.focus_right();

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(2),
        Container(id=ContainerId(1), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Container(id=ContainerId(0), parent=ContainerId(1), x=0.00, y=0.00, w=5.00, h=10.00, direction=Vertical,
            Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=5.00)
            Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=5.00, h=5.00)
          )
          Window(id=WindowId(2), parent=ContainerId(1), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_down_into_horizontal_nested_container() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.insert_window();

    // Focus window 0 (top)
    hub.focus_up();
    hub.focus_up();

    hub.focus_down();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00, direction=Horizontal,
            Window(id=WindowId(1), parent=ContainerId(1), x=0.00, y=5.00, w=3.33, h=5.00)
            Window(id=WindowId(2), parent=ContainerId(1), x=3.33, y=5.00, w=3.33, h=5.00)
            Window(id=WindowId(3), parent=ContainerId(1), x=6.67, y=5.00, w=3.33, h=5.00)
          )
        )
      )
    )
    ");
}

#[test]
fn focus_left_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.focus_left();
    hub.focus_left(); // Already at leftmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_right_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.insert_window();
    hub.focus_right(); // Already at rightmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=5.00, h=10.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=5.00, y=0.00, w=5.00, h=10.00)
        )
      )
    )
    ");
}

#[test]
fn focus_up_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_up();
    hub.focus_up(); // Already at topmost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}

#[test]
fn focus_down_at_boundary_does_nothing() {
    setup_logger();
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 0.0);

    hub.insert_window();
    hub.toggle_new_window_direction();
    hub.insert_window();
    hub.focus_down(); // Already at bottommost

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(1),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=10.00, h=10.00, direction=Vertical,
          Window(id=WindowId(0), parent=ContainerId(0), x=0.00, y=0.00, w=10.00, h=5.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=0.00, y=5.00, w=10.00, h=5.00)
        )
      )
    )
    ");
}
