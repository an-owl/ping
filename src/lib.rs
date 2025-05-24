#![no_std]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::{Index, IndexMut};
use uefi::boot::{open_protocol_exclusive, ScopedProtocol};
use uefi::fs::PathBuf;
use uefi::proto::device_path::{DevicePath};
use uefi::proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode, RegularFile};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::cstr16;
use uefi::proto::device_path::build::DevicePathBuilder;
use uefi::proto::device_path::build::media::FilePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

pub struct FileFinder {
    found_files: Vec<FileRef>,
}

impl FileFinder {
    pub const fn new() -> Self {
        FileFinder { found_files: Vec::new() }
    }

    pub fn locate_normal_boot_files_in_fs(&mut self, handle: &uefi::Handle) -> uefi::Result<()> {
        let mut fs: ScopedProtocol<SimpleFileSystem> = open_protocol_exclusive(*handle)?;
        let dev_path: ScopedProtocol<DevicePath> = open_protocol_exclusive(*handle)?;
        let mut root = fs.open_volume()?;

        let mut path = PathBuf::new();
        path.push(cstr16!("efi"));
        let efi_dir = root.open(&path.to_cstr16(), FileMode::Read, FileAttribute::empty())?;
        let mut efi_dir = efi_dir.into_directory().unwrap(); // Spec says this must be a dir


        // Iterate over all directories in /efi
        // I can't get an iterator for dir entries so this is not a nice way of doing this.
        loop {
            match efi_dir.read_entry_boxed() {
                Ok(Some(entry)) => {
                    let Ok(file) = efi_dir.open(entry.file_name(), FileMode::Read, FileAttribute::empty()) else {
                        continue
                    };


                    // We only care about directories
                    let Some(mut dir) = file.into_directory() else {
                        continue
                    };
                    {
                        let mut cpath = path.clone();
                        cpath.push(entry.file_name());
                        self.scan_efi_dir(&mut dir, &dev_path, cpath)?
                    }

                }
                Ok(None) => return Ok(()),
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    /// Assuming `dir` is `/efi/*/` this will locate all files with a ".efi" extension.
    fn scan_efi_dir(&mut self, dir: &mut Directory, fs_path: &DevicePath, file_path: PathBuf) -> uefi::Result<()> {
        loop {
            match find_in_dir(dir, |info| {
                info.is_regular_file() && info.file_name().to_string().ends_with(".efi") // We could skip the to_string() but that will take me longer
            }) {
                Ok(None) => return Ok(()),
                Ok(Some(entry)) => {

                    let file = dir.open(entry.file_name(), FileMode::Read, FileAttribute::empty())?;
                    let file = file.into_regular_file().unwrap(); // We should only get regular files

                    let mut cpath = file_path.clone();
                    cpath.push(entry.file_name());

                    let mut buff = Vec::new(); 
                    let mut builder = DevicePathBuilder::with_vec(&mut buff);
                    
                    for i in fs_path.node_iter() {
                        builder = builder.push(&i).unwrap();
                    }
                    builder = builder.push(&FilePath { path_name: cpath.to_cstr16() }).unwrap();
                    
                    self.found_files.push(FileRef::new(builder.finalize().unwrap().to_boxed() ,file));
                }
                Err(e) => {return Err(e)}
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&FileRef> {
        self.found_files.iter()
    }
    pub fn len(&self) -> usize {
        self.found_files.len()
    }
}

impl Index<usize> for FileFinder {
    type Output = FileRef;
    fn index(&self, index: usize) -> &Self::Output {
        &self.found_files[index]
    }
}

impl IndexMut<usize> for FileFinder {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.found_files[index]
    }
}

pub struct FileRef {
    full_path: Box<DevicePath>,
    file: RegularFile
}

impl FileRef {

    /// `partial_path` should be the absolute path form the fs root to the prent directory i.e. `/efi/boot/` for `fs1:/efi/boot/bootx64.efi`
    fn new(device_path:  Box<DevicePath>, file: RegularFile) -> Self {
        Self {
            full_path: device_path,
            file
        }
    }

    pub fn load_file(&mut self) -> uefi::Result<Vec<u8>> {
        let info: Box<FileInfo> = self.file.get_boxed_info()?;
        let mut buffer = Vec::new();
        buffer.resize(info.file_size() as usize, 0);
        
        self.file.read(&mut buffer)?;
        Ok(buffer)
    }
    
    pub fn path(&self) -> &DevicePath {
        &self.full_path
    }
}

impl core::fmt::Display for FileRef {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f,"{}",self.full_path.to_string(DisplayOnly(true),AllowShortcuts(true)).unwrap())
    }
}

/// Attempts to locate a file entry by calling `f` on it. When `f` returns `true` the [FileInfo]
/// passed to it will be returned.
/// If `f` matches nothing then `Ok(None)` is returned.
fn find_in_dir<F>(dir: &mut Directory, f: F) -> uefi::Result<Option<Box<FileInfo>>>
where
    F: Fn(&FileInfo) -> bool
{
    loop {
        match dir.read_entry_boxed() {
            Ok(Some(entry)) => {
                if f(&entry) {
                    return Ok(Some(entry));
                }
            }
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        }
    }
}