use nix::sys::stat::{lstat, SFlag};

pub fn is_regular(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFREG.bits() == SFlag::S_IFREG.bits()
}
pub fn is_link(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFLNK.bits() == SFlag::S_IFLNK.bits()
}

pub fn is_dir(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFDIR.bits() == SFlag::S_IFDIR.bits()
}
