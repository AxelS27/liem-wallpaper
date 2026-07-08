use lw_core::EasingType;
use lw_transition::interpolate;

#[test]
fn test_interpolate_boundaries() {
    for &easing in
        &[EasingType::Linear, EasingType::EaseIn, EasingType::EaseOut, EasingType::EaseInOut]
    {
        // Test exact boundaries
        assert!((interpolate(0.0, easing) - 0.0).abs() < f32::EPSILON);
        assert!((interpolate(1.0, easing) - 1.0).abs() < f32::EPSILON);

        // Test clamping behavior
        assert!((interpolate(-0.5, easing) - 0.0).abs() < f32::EPSILON);
        assert!((interpolate(1.5, easing) - 1.0).abs() < f32::EPSILON);
    }
}

#[test]
fn test_linear_interpolation() {
    assert!((interpolate(0.25, EasingType::Linear) - 0.25).abs() < f32::EPSILON);
    assert!((interpolate(0.5, EasingType::Linear) - 0.5).abs() < f32::EPSILON);
    assert!((interpolate(0.75, EasingType::Linear) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn test_ease_in_interpolation() {
    // EaseIn is t^2, so values should be strictly less than linear, except at boundaries
    assert!((interpolate(0.5, EasingType::EaseIn) - 0.25).abs() < f32::EPSILON);
    assert!(interpolate(0.25, EasingType::EaseIn) < 0.25);
    assert!(interpolate(0.75, EasingType::EaseIn) < 0.75);
}

#[test]
fn test_ease_out_interpolation() {
    // EaseOut starts fast, so values should be strictly greater than linear, except at boundaries
    assert!((interpolate(0.5, EasingType::EaseOut) - 0.75).abs() < f32::EPSILON);
    assert!(interpolate(0.25, EasingType::EaseOut) > 0.25);
    assert!(interpolate(0.75, EasingType::EaseOut) > 0.75);
}

#[test]
fn test_ease_in_out_interpolation() {
    // EaseInOut is symmetric around (0.5, 0.5)
    assert!((interpolate(0.5, EasingType::EaseInOut) - 0.5).abs() < f32::EPSILON);

    // First half decelerates/accelerates slower than linear
    assert!((interpolate(0.25, EasingType::EaseInOut) - 0.125).abs() < f32::EPSILON);
    assert!(interpolate(0.25, EasingType::EaseInOut) < 0.25);

    // Second half starts faster
    assert!((interpolate(0.75, EasingType::EaseInOut) - 0.875).abs() < f32::EPSILON);
    assert!(interpolate(0.75, EasingType::EaseInOut) > 0.75);
}
