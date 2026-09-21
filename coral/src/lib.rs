use std::path::{Component, Path};

pub fn world_path(name: &str) -> std::io::Result<&Path> {
    let world_path = Path::new(name);

    if name.ends_with('/')
        || name.ends_with('\\')
        || world_path.components().count() != 1
        || !matches!(world_path.components().next(), Some(Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid world name {:?}: world_name must be a single directory name",
                name
            ),
        ));
    }
    Ok(world_path)
}

#[cfg(test)]
mod tests {
    use super::world_path;
    use std::path::Path;

    #[test]
    fn valid_world_name() {
        assert_eq!(world_path("world").unwrap(), Path::new("world"));
    }

    #[test]
    fn invalid_world_name() {
        assert!(world_path("world/").is_err());
        assert!(world_path("world\\").is_err());
        assert!(world_path("/world").is_err());
        assert!(world_path("../world").is_err());
        assert!(world_path("foo/bar").is_err());
        assert!(world_path("./world").is_err());
        assert!(world_path("..").is_err());
        assert!(world_path(".").is_err());
        assert!(world_path("").is_err());
    }
}
