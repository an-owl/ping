#![no_main]
#![no_std]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec::Vec;
use uefi::boot::{get_handle_for_protocol, image_handle, load_image, locate_handle_buffer, open_protocol_exclusive, start_image, unload_image, ScopedProtocol, SearchType};
use uefi::prelude::*;
use uefi::{println, CString16, Identify};
use uefi::proto::console::text::Key;
use blip::FileFinder;

#[entry]
fn efi_main() -> Status {
    let handles = match locate_handle_buffer(SearchType::ByProtocol(&uefi::proto::media::fs::SimpleFileSystem::GUID)) {
        Ok(h) => h,
        Err(error) => {
            log::error!("Failed to locate filesystem handles: {error}");
            return error.status();
        }
    };
    
    let mut locator = FileFinder::new();
    
    for handle in handles.iter() {
        match open_protocol_exclusive(*handle) { // I'd use exclusive but other handles may be using these.
            Ok(mut proto) => {
                let _ = locator.locate_normal_boot_files_in_fs(&mut *proto).map_err(|error| {
                    // Log error and ignore
                    log::error!("Failed to locate filesystem handles: {error}");
                });
            }
            Err(error) => {
                log::debug!("Failed to open protocol: {error}"); // may occur do to other handles doing things
            }
        }
    }
    
    println!("found {} possible files", locator.len());
    for (i, file) in locator.iter().enumerate() {
        println!("{: >4}: {}",i,file)
    }
    
    let input_option = loop {
        println!("Select a file to boot (empty to exit)");
        let Ok(input) = get_input_buffer() else {
            log::error!("Failed to get input buffer: Input device returned error");
            return Status::DEVICE_ERROR;
        };
        // user requested exit
        if input.len() == 0 {
            return Status::SUCCESS;
        }
        let Ok(num): Result<usize,_> = input.parse() else {
            println!("Failed to parse input buffer: {} unrecognised", input);
            continue
        };
        if num >= locator.len() {
            println!("Invalid selection");
            continue
        }
        break num
    };
    
    let file = &mut locator[input_option];
    let file_data = match file.load_file() {
        Ok(data) => data,
        Err(error) => {
            log::error!("Failed to load file: {error}");
            return error.status();
        }
    };
    
    let child = match load_image(image_handle(),  uefi::boot::LoadImageSource::FromBuffer { buffer: &*file_data, file_path: None }) { // im not sure how to construct the file path
        Ok(data) => data,
        Err(error) => {
            log::error!("Failed to load image: {error}");
            return error.status();
        }
    };
    
    match start_image(child) {
        Ok(_) => {}
        Err(error) => {
            log::error!("Failed to start image: {error}");
        }
    };
    let _ = unload_image(child); // we dont care if this fails

    Status::SUCCESS
}

fn get_input_buffer() -> uefi::Result<alloc::string::String> {
    let mut input: ScopedProtocol<uefi::proto::console::text::Input> = open_protocol_exclusive(get_handle_for_protocol::<uefi::proto::console::text::Input>()?)?;
    let mut buffer = Vec::new();
    

    while let Some(_) = input.read_key()? {} // Ignore all buffered input
    loop {
        let event = input.wait_for_key_event().expect("The docs dont mention why this can fail");
        let _ = boot::wait_for_event(&mut [event]); // I actually dont care about the return code here
        match input.read_key() {
            
            Ok(Some(Key::Printable(c))) => {
                if c == '\n' {
                    // I hate this
                    let mut output = CString16::new();
                    for i in buffer {
                        output.push(i);
                    }
                    return Ok(output.to_string())
                    
                } else if c == '\u{8}' { // backspace
                    let _ = buffer.pop();
                } else {
                    buffer.push(c);
                }
            }

            Err(e) => {
                return Err(e) // lol your keyboard broke
            }
            _ => {} // Ignore unknown input
        }
    }
}
