use std::time::Duration;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_render_coordinator::config::RenderConfig;
use liquide_render_coordinator::coordinator::RenderCoordinator;
use liquide_render_coordinator::render_task::{
    RenderDataFormat, RenderScene, RenderTask, RenderTaskKind,
};

fn solid_scene(width: u32, height: u32, color: Color) -> RenderScene {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, width as f32, height as f32)),
    );
    root.add_child(SceneNode::new(
        1,
        SceneNodeKind::Background { color },
        NodeProperties::new(Rect::new(0.0, 0.0, width as f32, height as f32)),
    ));

    RenderScene::new(width, height, root.flatten())
}

fn contains_nonblank_pixel(bytes: &[u8]) -> bool {
    bytes
        .chunks_exact(4)
        .any(|pixel| pixel[3] != 0 && (pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0))
}

#[tokio::test]
async fn submitted_task_polls_real_nonblank_rendered_pixels() {
    let config = RenderConfig::builder()
        .window_threads(1)
        .enable_background(true)
        .timeout(Duration::from_millis(250))
        .build();
    let coordinator = RenderCoordinator::new(config).await.unwrap();

    let scene = solid_scene(32, 24, Color::new(40, 100, 180, 255));
    let task = RenderTask::new(0, RenderTaskKind::Background).with_scene(scene);
    let task_id = coordinator.submit_task(task).await.unwrap();

    let mut output = None;
    for _ in 0..40 {
        if let Some(found) = coordinator
            .poll_outputs()
            .await
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.task_id == task_id)
        {
            output = Some(found);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let output = output.expect("render output should be produced");
    assert!(output.success, "render failed: {:?}", output.error);

    let metadata = output.metadata.as_ref().expect("render metadata");
    assert_eq!(metadata.width, 32);
    assert_eq!(metadata.height, 24);
    assert_eq!(metadata.stride, 32 * 4);
    assert_eq!(metadata.format, RenderDataFormat::Bgra8);
    assert!(!metadata.damage_tiles.is_empty());

    let data = output.data.as_ref().expect("rendered pixel data");
    assert_eq!(data.format(), RenderDataFormat::Bgra8);
    assert_eq!(data.data().len(), (32 * 24 * 4) as usize);
    assert!(contains_nonblank_pixel(data.data()));
}
