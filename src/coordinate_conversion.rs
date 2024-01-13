use bevy::{prelude::*, render::camera::CameraProjection};

pub fn ndc_to_world(
    ndc: Vec3,
    camera_transform: &Transform,
    camera_projection: &impl CameraProjection,
) -> Vec3 {
    // Unproject from NDC to camera space
    let camera_space = camera_projection
        .get_projection_matrix()
        .inverse()
        .project_point3(ndc);

    // Convert from camera space to world space
    let world_space = camera_transform.compute_matrix() * camera_space.extend(1.0);

    world_space.truncate()
}

pub fn screen_to_ndc(screen_pos: Vec2, window: &Window, depth: f32) -> Vec3 {
    // Convert screen coordinates to NDC
    // Screen coordinates are typically (0,0) in the top-left and (window_width, window_height) in the bottom-right
    let ndc_x = (screen_pos.x / window.width()) * 2.0 - 1.0;
    let ndc_y = (screen_pos.y / window.height()) * 2.0 - 1.0;

    // Invert y-axis since screen coordinates usually have y going down but NDC has y going up
    Vec3::new(ndc_x, -ndc_y, depth)
}
