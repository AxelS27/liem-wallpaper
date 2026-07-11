use lw_core::{EasingStyle, EasingDirection};
use lw_transition::interpolate;

#[test]
fn test_interpolate_boundaries() {
    let styles = [
        EasingStyle::Linear,
        EasingStyle::Sine,
        EasingStyle::Quad,
        EasingStyle::Cubic,
        EasingStyle::Quart,
        EasingStyle::Quint,
        EasingStyle::Exponential,
        EasingStyle::Circular,
        EasingStyle::Back,
        EasingStyle::Bounce,
        EasingStyle::Elastic,
    ];

    let directions = [
        EasingDirection::In,
        EasingDirection::Out,
        EasingDirection::InOut,
    ];

    for &style in &styles {
        for &direction in &directions {
            // Test exact boundaries
            assert!((interpolate(0.0, style, direction) - 0.0).abs() < f32::EPSILON);
            assert!((interpolate(1.0, style, direction) - 1.0).abs() < f32::EPSILON);

            // Test clamping behavior
            assert!((interpolate(-0.5, style, direction) - 0.0).abs() < f32::EPSILON);
            assert!((interpolate(1.5, style, direction) - 1.0).abs() < f32::EPSILON);
        }
    }
}

#[test]
fn test_linear_interpolation() {
    assert!((interpolate(0.25, EasingStyle::Linear, EasingDirection::In) - 0.25).abs() < f32::EPSILON);
    assert!((interpolate(0.5, EasingStyle::Linear, EasingDirection::Out) - 0.5).abs() < f32::EPSILON);
    assert!((interpolate(0.75, EasingStyle::Linear, EasingDirection::InOut) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn test_quad_in_interpolation() {
    assert!((interpolate(0.5, EasingStyle::Quad, EasingDirection::In) - 0.25).abs() < f32::EPSILON);
    assert!(interpolate(0.25, EasingStyle::Quad, EasingDirection::In) < 0.25);
    assert!(interpolate(0.75, EasingStyle::Quad, EasingDirection::In) < 0.75);
}

#[test]
fn test_quad_out_interpolation() {
    assert!((interpolate(0.5, EasingStyle::Quad, EasingDirection::Out) - 0.75).abs() < f32::EPSILON);
    assert!(interpolate(0.25, EasingStyle::Quad, EasingDirection::Out) > 0.25);
    assert!(interpolate(0.75, EasingStyle::Quad, EasingDirection::Out) > 0.75);
}
