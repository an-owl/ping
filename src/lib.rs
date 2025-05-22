#![no_std]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::{Index, IndexMut};
use uefi::{cstr16, CString16};
use uefi::proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode, FileSystemVolumeLabel, RegularFile};

pub struct FileFinder {
    found_files: Vec<FileRef>,
}

impl FileFinder {
    pub const fn new() -> Self {
        FileFinder { found_files: Vec::new() }
    }

    pub fn locate_normal_boot_files_in_fs(&mut self, fs: &mut uefi::proto::media::fs::SimpleFileSystem) -> uefi::Result<()> {
        let mut root = fs.open_volume()?;
        let efi_dir = root.open(cstr16!("efi"), FileMode::Read, FileAttribute::empty())?;
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
                    let mut leading_path = CString16::from(cstr16!("\\efi\\"));
                    leading_path.push_str(entry.file_name());

                    self.scan_efi_dir(&mut dir, leading_path)?
                }
                Ok(None) => return Ok(()),
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    /// Assuming `dir` is `/efi/*/` this will locate all files with a ".efi" extension.
    fn scan_efi_dir(&mut self, dir: &mut Directory, leading_path: CString16) -> uefi::Result<()> {
        loop {
            match find_in_dir(dir, |info| {
                info.is_regular_file() && info.file_name().to_string().ends_with(".efi") // We could skip the to_string() but that will take me longer
            }) {
                Ok(None) => return Ok(()),
                Ok(Some(entry)) => {
                    let file = dir.open(entry.file_name(), FileMode::Read, FileAttribute::empty())?;
                    let file = file.into_regular_file().unwrap(); // We should only get regular files
                    self.found_files.push(FileRef::new(&leading_path,file)?);
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
    full_path: CString16,
    file: RegularFile
}

impl FileRef {

    /// `partial_path` should be the absolute path form the fs root to the prent directory i.e. `/efi/boot/` for `fs1:/efi/boot/bootx64.efi`
    fn new(partial_path: &CString16, mut file: RegularFile) -> uefi::Result<Self> {
        let mut full_path = CString16::new();
        let index: Box<FileSystemVolumeLabel> = file.get_boxed_info()?;
        let info: Box<FileInfo> = file.get_boxed_info()?;
        full_path.push_str(index.volume_label());
        full_path.push_str(partial_path);
        full_path.push_str(info.file_name());
        
        Ok(FileRef {full_path, file})
    }

    pub fn load_file(&mut self) -> uefi::Result<Vec<u8>> {
        let info: Box<FileInfo> = self.file.get_boxed_info()?;
        let mut buffer = Vec::new();
        buffer.resize(info.file_size() as usize, 0);
        
        self.file.read(&mut buffer)?;
        Ok(buffer)
    }
}

impl core::fmt::Display for FileRef {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f,"{}",self.full_path)
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