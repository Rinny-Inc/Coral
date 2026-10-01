use crate::playerdata::PlayerData;

pub type SpawnPos = (f64, f64, f64, f32, f32);

pub fn join_position(saved: Option<&PlayerData>, world_spawn: SpawnPos) -> SpawnPos {
    match saved {
        Some(d) => (d.x, d.y, d.z, d.yaw, d.pitch),
        None => world_spawn,
    }
}

pub fn respawn_position(bed_spawn: Option<(i32, i32, i32)>, world_spawn: SpawnPos) -> SpawnPos {
    match bed_spawn {
        Some((x, y, z)) => (x as f64 + 0.5, y as f64, z as f64 + 0.5, 0.0, 0.0),
        None => world_spawn,
    }
}

pub fn spawn_chunk(x: f64, z: f64) -> (i32, i32) {
    ((x.floor() as i32) >> 4, (z.floor() as i32) >> 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD_SPAWN: SpawnPos = (0.5, 5.0, 0.5, 0.0, 0.0);
    const MOVED_SPAWN: SpawnPos = (100.5, 64.0, -199.5, 90.0, -15.0);

    fn saved_at(x: f64, y: f64, z: f64) -> PlayerData {
        PlayerData {
            x,
            y,
            z,
            yaw: 42.0,
            pitch: -7.0,
            ..Default::default()
        }
    }

    #[test]
    fn first_join_uses_world_spawn() {
        assert_eq!(join_position(None, WORLD_SPAWN), WORLD_SPAWN);
        assert_eq!(join_position(None, MOVED_SPAWN), MOVED_SPAWN);
    }

    #[test]
    fn rejoin_restores_saved_position_and_facing() {
        let d = saved_at(12.5, 70.0, -3.5);
        assert_eq!(
            join_position(Some(&d), WORLD_SPAWN),
            (12.5, 70.0, -3.5, 42.0, -7.0)
        );
    }

    #[test]
    fn rejoin_ignores_the_bed_spawn() {
        let mut d = saved_at(12.5, 70.0, -3.5);
        d.bed_spawn = Some((500, 64, 500));
        assert_eq!(join_position(Some(&d), WORLD_SPAWN).0, 12.5);
    }

    #[test]
    fn respawn_without_a_bed_uses_world_spawn() {
        assert_eq!(respawn_position(None, WORLD_SPAWN), WORLD_SPAWN);
    }

    #[test]
    fn respawn_without_a_bed_keeps_the_world_spawn_facing() {
        let (_, _, _, yaw, pitch) = respawn_position(None, MOVED_SPAWN);
        assert_eq!((yaw, pitch), (90.0, -15.0));
    }

    #[test]
    fn respawn_with_a_bed_centres_on_it() {
        assert_eq!(
            respawn_position(Some((10, 64, -20)), WORLD_SPAWN),
            (10.5, 64.0, -19.5, 0.0, 0.0)
        );
    }

    #[test]
    fn bed_spawn_at_negative_coordinates_stays_inside_its_block() {
        let (x, _, z, _, _) = respawn_position(Some((-1, 4, -20)), WORLD_SPAWN);
        assert_eq!((x, z), (-0.5, -19.5));
        assert_eq!(x.floor() as i32, -1);
        assert_eq!(z.floor() as i32, -20);
    }

    #[test]
    fn spawn_chunk_floors_negative_coordinates() {
        assert_eq!(spawn_chunk(0.5, 0.5), (0, 0));
        assert_eq!(spawn_chunk(16.0, 31.9), (1, 1));
        assert_eq!(spawn_chunk(-0.5, -19.5), (-1, -2));
        assert_eq!(spawn_chunk(-16.0, -16.1), (-1, -2));
    }

    #[test]
    fn respawn_chunk_matches_the_respawn_block() {
        let (x, _, z, _, _) = respawn_position(Some((-1, 4, -20)), WORLD_SPAWN);
        assert_eq!(spawn_chunk(x, z), (-1, -2));
        assert_eq!(((x as i32) >> 4, (z as i32) >> 4), (0, -2));
    }
}
