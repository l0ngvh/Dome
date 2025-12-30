use crate::core::hub::Hub;
use crate::core::node::Dimension;
use crate::core::tests::snapshot;
use insta::assert_snapshot;

#[test]
fn window_with_border() {
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut hub = Hub::new(screen, 2.0, 3.0);
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=10.00 h=10.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(0),
        Window(id=WindowId(0), parent=WorkspaceId(0), x=2.00, y=2.00, w=6.00, h=6.00)
      )
    )

                                                                                                                                                          
     ********                                                                                                                                             
     *      *                                                                                                                                             
     *      *                                                                                                                                             
     *      *                                                                                                                                             
     *  W0  *                                                                                                                                             
     *      *                                                                                                                                             
     *      *                                                                                                                                             
     ********
    ");
}

#[test]
fn border_with_nested_containers() {
    let screen = Dimension {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 20.0,
    };
    let mut hub = Hub::new(screen, 1.0, 3.0);
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.insert_tiling();
    hub.toggle_spawn_direction();
    hub.insert_tiling();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WorkspaceId(0), screen=(x=0.00 y=0.00 w=50.00 h=20.00),
      Workspace(id=WorkspaceId(0), name=0, focused=WindowId(4),
        Container(id=ContainerId(0), parent=WorkspaceId(0), x=0.00, y=0.00, w=50.00, h=20.00, direction=Horizontal,
          Window(id=WindowId(0), parent=ContainerId(0), x=1.00, y=1.00, w=10.50, h=18.00)
          Window(id=WindowId(1), parent=ContainerId(0), x=13.50, y=1.00, w=10.50, h=18.00)
          Window(id=WindowId(2), parent=ContainerId(0), x=26.00, y=1.00, w=10.50, h=18.00)
          Container(id=ContainerId(1), parent=ContainerId(0), x=37.50, y=0.00, w=12.50, h=20.00, direction=Vertical,
            Window(id=WindowId(3), parent=ContainerId(1), x=38.50, y=1.00, w=10.50, h=8.00)
            Window(id=WindowId(4), parent=ContainerId(1), x=38.50, y=11.00, w=10.50, h=8.00)
          )
        )
      )
    )

    +-----------++----------++-----------++----------+                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||    W3    |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           ||          |                                                                                                    
    |           ||          ||           |+----------+                                                                                                    
    |    W0     ||    W1    ||    W2     |************                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*    W4    *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    |           ||          ||           |*          *                                                                                                    
    +-----------++----------++-----------+************
    ");
}
