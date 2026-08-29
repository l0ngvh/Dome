use std::path::PathBuf;

use interprocess::local_socket::{GenericFilePath, ToFsName};

pub fn socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::temp_dir().join("dome.sock")
    }
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\dome")
    }
}

pub fn socket_name() -> interprocess::local_socket::Name<'static> {
    socket_path().to_fs_name::<GenericFilePath>().unwrap()
}
