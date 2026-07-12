use lw_core::{EasingDirection, EasingStyle};

/// Interpolates time `t` (from 0.0 to 1.0) using the specified `EasingStyle` and `EasingDirection`.
#[must_use]
pub fn interpolate(t: f32, style: EasingStyle, direction: EasingDirection) -> f32 {
    let t = t.clamp(0.0, 1.0);

    // EasingStyle functions implemented as EasingDirection::In:
    let f = |u: f32| -> f32 {
        match style {
            EasingStyle::Linear => u,
            EasingStyle::Sine => 1.0 - (u * std::f32::consts::PI / 2.0).cos(),
            EasingStyle::Quad => u * u,
            EasingStyle::Cubic => u * u * u,
            EasingStyle::Quart => u * u * u * u,
            EasingStyle::Quint => u * u * u * u * u,
            EasingStyle::Exponential => {
                if u <= 0.0 {
                    0.0
                } else {
                    (2.0f32).powf(10.0 * u - 10.0)
                }
            }
            EasingStyle::Circular => 1.0 - (1.0 - u * u).sqrt(),
            EasingStyle::Back => {
                let s = 1.70158;
                u * u * ((s + 1.0) * u - s)
            }
            EasingStyle::Bounce => {
                // In-Bounce is derived as 1 - OutBounce(1 - u)
                1.0 - out_bounce(1.0 - u)
            }
            EasingStyle::Elastic => {
                if u <= 0.0 {
                    0.0
                } else if u >= 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    -((2.0f32).powf(10.0 * u - 10.0)) * ((10.0 * u - 10.75) * c4).sin()
                }
            }
        }
    };

    match direction {
        EasingDirection::In => f(t),
        EasingDirection::Out => 1.0 - f(1.0 - t),
        EasingDirection::InOut => {
            if t < 0.5 {
                f(t * 2.0) * 0.5
            } else {
                1.0 - f((1.0 - t) * 2.0) * 0.5
            }
        }
    }
}

fn out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;

    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}
