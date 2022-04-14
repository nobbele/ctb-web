/// Returns a point that is the percent of `progress` inbetween `min` and `max`
pub fn lerp(min: f32, max: f32, progress: f32) -> f32 {
    min + (max - min) * progress
}

pub fn clamped_lerp(min: f32, max: f32, progress: f32) -> f32 {
    lerp(min, max, progress).clamp(min.min(max), max.max(min))
}

#[test]
fn test_lerp() {
    assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
    assert_eq!(lerp(0.0, 10.0, 1.5), 15.0);
    assert_eq!(lerp(5.0, 10.0, -1.0), 0.0);

    assert_eq!(clamped_lerp(0.0, 10.0, 0.5), 5.0);
    assert_eq!(clamped_lerp(0.0, 10.0, 1.5), 10.0);
    assert_eq!(clamped_lerp(5.0, 10.0, -1.0), 5.0);
}

/// Returns the percent of which `value` is inbetween `min` and `max`
pub fn inv_lerp(min: f32, max: f32, value: f32) -> f32 {
    (value - min) / (max - min)
}

#[test]
fn test_inv_lerp() {
    assert_eq!(inv_lerp(0.0, 10.0, 5.0), 0.5);
    assert_eq!(inv_lerp(0.0, 10.0, 15.0), 1.5);
    assert_eq!(inv_lerp(5.0, 10.0, 0.0), -1.0);
}

/// Maps `value` in the coordinate system of `in_min` to `in_max` to the coordinate system of `out_min` to `out_max`
pub fn remap(in_min: f32, in_max: f32, out_min: f32, out_max: f32, value: f32) -> f32 {
    lerp(out_min, out_max, inv_lerp(in_min, in_max, value))
}

pub fn clamped_remap(in_min: f32, in_max: f32, out_min: f32, out_max: f32, value: f32) -> f32 {
    lerp(out_min, out_max, inv_lerp(in_min, in_max, value))
        .clamp(out_min.min(out_max), out_max.max(out_min))
}

#[test]
fn test_remap() {
    assert_eq!(remap(0.0, 1.0, -1.0, 1.0, 0.5), 0.0);
    assert_eq!(remap(0.0, 1000.0, 0.0, 1.0, 250.0), 0.25);
    assert_eq!(remap(0.0, 1000.0, 1.0, 0.0, 250.0), 0.75);
    assert_eq!(remap(0.0, 1000.0, 0.0, 1.0, -250.0), -0.25);

    assert_eq!(clamped_remap(0.0, 1.0, -1.0, 1.0, 0.5), 0.0);
    assert_eq!(clamped_remap(0.0, 1000.0, 0.0, 1.0, 250.0), 0.25);
    assert_eq!(clamped_remap(0.0, 1000.0, 1.0, 0.0, 250.0), 0.75);
    assert_eq!(clamped_remap(0.0, 1000.0, 0.0, 1.0, -250.0), 0.0);
}