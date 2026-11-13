use glam::camera::rh::proj::directx::perspective;
use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec2, Vec3};

pub const DEFAULT_DISTANCE: f32 = 11.0;
pub const MIN_DISTANCE: f32 = 4.5;
pub const MAX_DISTANCE: f32 = 26.0;
pub const PITCH: f32 = 0.92;
pub const YAW: f32 = 0.6;
pub const FOV: f32 = 0.62;
pub const NEAR: f32 = 0.1;
pub const FAR: f32 = 400.0;
pub const FOLLOW: f32 = 0.6;
pub const ZOOM_STEP: f32 = 0.88;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub target: Vec2,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub viewport: Vec2,
}

impl View {
    pub fn new(viewport: Vec2) -> Self {
        Self {
            target: Vec2::ZERO,
            distance: DEFAULT_DISTANCE,
            yaw: YAW,
            pitch: PITCH,
            fov: FOV,
            viewport: viewport.max(Vec2::ONE),
        }
    }

    pub fn forward(&self) -> Vec2 {
        Vec2::new(self.yaw.sin(), self.yaw.cos())
    }

    pub fn eye(&self) -> Vec3 {
        let back = (-self.forward()).extend(0.0) * self.pitch.cos();
        self.target.extend(0.0) + (back + Vec3::Z * self.pitch.sin()) * self.distance
    }

    pub fn view_proj(&self) -> Mat4 {
        let aspect = self.viewport.x / self.viewport.y;
        let proj = perspective(self.fov, aspect, NEAR, FAR);
        proj * look_at_mat4(self.eye(), self.target.extend(0.0), Vec3::Z)
    }

    pub fn project(&self, p: Vec3) -> Option<Vec2> {
        let clip = self.view_proj() * p.extend(1.0);
        if clip.w <= 1e-6 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec2::new(
            (ndc.x + 1.0) * 0.5 * self.viewport.x,
            (1.0 - ndc.y) * 0.5 * self.viewport.y,
        ))
    }

    pub fn ray(&self, screen: Vec2) -> (Vec3, Vec3) {
        let ndc = Vec2::new(
            screen.x / self.viewport.x * 2.0 - 1.0,
            1.0 - screen.y / self.viewport.y * 2.0,
        );
        let inverse = self.view_proj().inverse();
        let near = inverse.project_point3(ndc.extend(0.0));
        let far = inverse.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize())
    }

    pub fn pick(&self, screen: Vec2, height: f32) -> Option<Vec2> {
        let (origin, dir) = self.ray(screen);
        if dir.z.abs() < 1e-6 {
            return None;
        }
        let t = (height - origin.z) / dir.z;
        (t >= 0.0).then(|| (origin + dir * t).truncate())
    }

    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * ZOOM_STEP.powf(steps)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn follow(&mut self, goal: Vec2, dt: f32) {
        let k = 1.0 - (-dt / FOLLOW).exp();
        self.target += (goal - self.target) * k;
    }

    pub fn basis(&self) -> (Vec3, Vec3) {
        let dir = (self.target.extend(0.0) - self.eye()).normalize();
        let right = dir.cross(Vec3::Z).normalize();
        (right, right.cross(dir))
    }

    pub fn pixel_size(&self) -> f32 {
        2.0 * self.distance * (0.5 * self.fov).tan() / self.viewport.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

    fn view() -> View {
        let mut v = View::new(SIZE);
        v.target = Vec2::new(1.0, 2.0);
        v
    }

    fn near(a: Vec2, b: Vec2, tolerance: f32) -> bool {
        a.distance(b) < tolerance
    }

    #[test]
    fn new_view_uses_the_defaults() {
        let v = View::new(SIZE);
        assert_eq!(v.target, Vec2::ZERO);
        assert_eq!(
            (v.distance, v.yaw, v.pitch, v.fov),
            (DEFAULT_DISTANCE, YAW, PITCH, FOV)
        );
        assert_eq!(v.viewport, SIZE);
    }

    #[test]
    fn eye_sits_behind_and_above_the_target() {
        let v = view();
        let eye = v.eye();
        assert!((eye.distance(v.target.extend(0.0)) - v.distance).abs() < 1e-3);
        assert!((eye.z - v.distance * PITCH.sin()).abs() < 1e-3);
        let back = (eye.truncate() - v.target).normalize();
        assert!(near(back, -v.forward(), 1e-4));
    }

    #[test]
    fn target_projects_to_the_center() {
        let v = view();
        let s = v.project(v.target.extend(0.0)).expect("visible");
        assert!(near(s, SIZE * 0.5, 0.01));
    }

    #[test]
    fn pick_inverts_project_on_the_plane() {
        let v = view();
        for p in [Vec2::ZERO, Vec2::new(3.0, -2.0), Vec2::new(-4.0, 1.5)] {
            let s = v.project(p.extend(0.0)).expect("visible");
            assert!(near(v.pick(s, 0.0).expect("hit"), p, 1e-3), "{p}");
        }
        assert!(near(v.pick(SIZE * 0.5, 0.0).unwrap(), v.target, 1e-3));
    }

    #[test]
    fn pick_finds_points_on_a_raised_plane() {
        let v = view();
        let p = Vec3::new(1.5, 1.0, 0.5);
        let s = v.project(p).expect("visible");
        assert!(near(v.pick(s, 0.5).unwrap(), p.truncate(), 1e-3));
        assert!(!near(v.pick(s, 0.0).unwrap(), p.truncate(), 0.1));
    }

    #[test]
    fn ray_points_down_from_the_eye() {
        let v = view();
        let (origin, dir) = v.ray(SIZE * 0.5);
        assert!((dir.length() - 1.0).abs() < 1e-4);
        assert!(dir.z < 0.0);
        assert!(origin.distance(v.eye()) < 0.2);
    }

    #[test]
    fn far_points_appear_higher_and_world_x_runs_up_right() {
        let v = view();
        let center = v.project(v.target.extend(0.0)).unwrap();
        let ahead = v
            .project((v.target + v.forward() * 2.0).extend(0.0))
            .unwrap();
        assert!(ahead.y < center.y);
        let east = v.project((v.target + Vec2::X).extend(0.0)).unwrap();
        assert!(east.x > center.x && east.y < center.y);
    }

    #[test]
    fn points_behind_the_eye_are_not_projected() {
        let v = view();
        let behind = v.eye() + (v.eye() - v.target.extend(0.0));
        assert_eq!(v.project(behind), None);
    }

    #[test]
    fn zoom_moves_within_limits() {
        let mut v = view();
        v.zoom(1.0);
        assert!((v.distance - DEFAULT_DISTANCE * ZOOM_STEP).abs() < 1e-4);
        v.zoom(100.0);
        assert_eq!(v.distance, MIN_DISTANCE);
        v.zoom(-100.0);
        assert_eq!(v.distance, MAX_DISTANCE);
    }

    #[test]
    fn follow_eases_toward_the_goal() {
        let mut v = view();
        let goal = Vec2::new(5.0, 2.0);
        v.follow(goal, FOLLOW);
        assert!((v.target.x - (1.0 + 4.0 * 0.632)).abs() < 0.01);
        for _ in 0..600 {
            v.follow(goal, 1.0 / 60.0);
        }
        assert!(near(v.target, goal, 1e-3));
    }

    #[test]
    fn basis_is_orthonormal_and_matches_the_screen() {
        let v = view();
        let (right, up) = v.basis();
        assert!((right.length() - 1.0).abs() < 1e-5 && (up.length() - 1.0).abs() < 1e-5);
        assert!(right.dot(up).abs() < 1e-5);
        let center = v.project(v.target.extend(0.0)).unwrap();
        let r = v.project(v.target.extend(0.0) + right).unwrap();
        assert!(r.x > center.x && (r.y - center.y).abs() < 0.5);
        let u = v.project(v.target.extend(0.0) + up).unwrap();
        assert!(u.y < center.y && (u.x - center.x).abs() < 0.5);
    }

    #[test]
    fn pixel_size_grows_with_distance() {
        let mut v = view();
        let close = v.pixel_size();
        v.zoom(-2.0);
        assert!(close > 0.0 && v.pixel_size() > close);
    }
}
