//! Stick deadzone + acceleration curves, used by mouse-mode layers to turn
//! a normalized stick deflection into a per-frame pixel delta.

use crate::config::AccelCurve;

/// Applies a radial deadzone: values with magnitude below `deadzone` are
/// clamped to zero, and the remaining range is rescaled to [0.0, 1.0] so
/// there's no dead gap right past the deadzone edge.
pub fn apply_deadzone(value: f32, deadzone: f32) -> f32 {
    let mag = value.abs();
    if mag < deadzone {
        return 0.0;
    }
    let sign = value.signum();
    let scaled = (mag - deadzone) / (1.0 - deadzone).max(f32::EPSILON);
    sign * scaled.clamp(0.0, 1.0)
}

/// Converts a deadzone-applied stick axis value in [-1.0, 1.0] into a
/// per-second pixel velocity given a sensitivity (px/sec at full
/// deflection) and acceleration curve.
pub fn axis_to_velocity(value: f32, sensitivity: f32, curve: &AccelCurve) -> f32 {
    let mag = value.abs();
    let shaped = match curve {
        AccelCurve::Linear => mag,
        AccelCurve::Quadratic => mag * mag,
    };
    value.signum() * shaped * sensitivity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_clamps_small_values_to_zero() {
        assert_eq!(apply_deadzone(0.05, 0.08), 0.0);
        assert_eq!(apply_deadzone(-0.05, 0.08), 0.0);
    }

    #[test]
    fn deadzone_rescales_remaining_range() {
        let v = apply_deadzone(1.0, 0.08);
        assert!((v - 1.0).abs() < 0.001);
    }

    #[test]
    fn quadratic_curve_is_gentler_near_center() {
        let linear = axis_to_velocity(0.5, 100.0, &AccelCurve::Linear);
        let quad = axis_to_velocity(0.5, 100.0, &AccelCurve::Quadratic);
        assert!(quad < linear);
    }
}
