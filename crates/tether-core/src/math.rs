pub use glam::{Mat4, Vec2, Vec3};

use std::f32::consts::{PI, TAU};

pub fn wrap_angle(a: f32) -> f32 {
    let r = a.rem_euclid(TAU);
    if r > PI { r - TAU } else { r }
}

pub fn signed_angle(from: Vec2, to: Vec2) -> f32 {
    from.perp_dot(to).atan2(from.dot(to))
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn wrap_angle_keeps_small_angles() {
        assert!(near(wrap_angle(0.5), 0.5));
        assert!(near(wrap_angle(-0.5), -0.5));
    }

    #[test]
    fn wrap_angle_maps_into_half_open_range() {
        assert!(near(wrap_angle(PI + 0.25), -PI + 0.25));
        assert!(near(wrap_angle(-PI - 0.25), PI - 0.25));
        assert!(near(wrap_angle(4.0 * PI + 1.0), 1.0));
        assert!(near(wrap_angle(PI), PI));
        assert!(near(wrap_angle(-PI), PI));
    }

    #[test]
    fn signed_angle_is_positive_for_left_turns() {
        assert!(near(signed_angle(Vec2::X, Vec2::Y), FRAC_PI_2));
        assert!(near(signed_angle(Vec2::X, -Vec2::Y), -FRAC_PI_2));
        assert!(near(signed_angle(Vec2::X, Vec2::new(3.0, 0.0)), 0.0));
    }

    #[test]
    fn smoothstep_clamps_and_eases() {
        assert_eq!(smoothstep(1.0, 3.0, 0.0), 0.0);
        assert_eq!(smoothstep(1.0, 3.0, 4.0), 1.0);
        assert!(near(smoothstep(1.0, 3.0, 2.0), 0.5));
        assert!(near(smoothstep(0.0, 1.0, 0.25), 0.15625));
    }
}
