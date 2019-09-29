use std::io;
use std::path::Path;
use std::path::PathBuf;

use failure::err_msg;
use failure::ResultExt;

pub fn dir_of<F>(path: &Path, current_dir: F) -> Result<PathBuf, failure::Error>
where
    F: FnOnce() -> Result<PathBuf, io::Error>,
{
    Ok(match path.parent() {
        None => PathBuf::from("/"),
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => {
            let mut cwd =
                current_dir().with_context(|_| err_msg("determining current working directory"))?;
            cwd.push(path);
            cwd
        }
    })
}

#[test]
fn test_dir_of() {
    assert_eq!(
        PathBuf::from("/home/quux"),
        dir_of(Path::new("bar"), || Ok(PathBuf::from("/home/quux"))).unwrap()
    );

    assert_eq!(
        PathBuf::from("/foo"),
        dir_of(Path::new("/foo/bar"), || panic!()).unwrap()
    );

    assert_eq!(
        PathBuf::from("/home/quux/../foo"),
        dir_of(Path::new("../foo/bar"), || Ok(PathBuf::from("/home/quux"))).unwrap()
    );

    assert_eq!(
        PathBuf::from("/"),
        dir_of(Path::new("/bar"), || panic!()).unwrap()
    );
}
