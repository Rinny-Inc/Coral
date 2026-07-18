use rand::RngExt;

/// Manual player drop
pub fn manual_drop_velocity(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw_rad = (yaw as f64).to_radians();
    let pitch_rad = (pitch as f64).to_radians();

    let base = 0.3;

    let mut vx = -yaw_rad.sin() * pitch_rad.cos() * base;
    let mut vz = yaw_rad.cos() * pitch_rad.cos() * base;
    let mut vy = -pitch_rad.sin() * base + 0.1;

    let mut rng = rand::rng();
    let jitter = |rng: &mut rand::rngs::ThreadRng| -> f64 {
        (rng.random::<f32>() - rng.random::<f32>()) as f64 * 0.02
    };
    vx += jitter(&mut rng);
    vy += jitter(&mut rng);
    vz += jitter(&mut rng);

    (vx, vy, vz)
}

/// Block break drop from mining
pub fn break_drop_velocity() -> (f64, f64, f64) {
    let mut rng = rand::rng();
    let vx = (rng.random::<f64>() - rng.random::<f64>()) * 0.1;
    let vy = rng.random::<f64>() * 0.1 + 0.2;
    let vz = (rng.random::<f64>() - rng.random::<f64>()) * 0.1;
    (vx, vy, vz)
}
