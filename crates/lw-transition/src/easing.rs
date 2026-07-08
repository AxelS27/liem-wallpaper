use lw_core::EasingType;

/// Interpolates time `t` (from 0.0 to 1.0) using the specified `EasingType`.
#[must_use]
pub fn interpolate(t: f32, easing: EasingType) -> f32 {
    let t = t.clamp(0.0, 1.0);

    match easing {
        EasingType::Linear => t,
        EasingType::EaseIn => t * t,
        EasingType::EaseOut => t * (2.0 - t),
        EasingType::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
    }
}
