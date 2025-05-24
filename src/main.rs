#![no_main]
#![no_std]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec::Vec;
use blip::FileFinder;
use uefi::boot::{
    ScopedProtocol, SearchType, get_handle_for_protocol, image_handle, load_image,
    locate_handle_buffer, open_protocol_exclusive, stall, start_image, unload_image,
};
use uefi::helpers::init;
use uefi::prelude::*;
use uefi::proto::BootPolicy;
use uefi::proto::console::text::Key;
use uefi::{CString16, Identify, print, println};

#[entry]
fn efi_main() -> Status {
    init().unwrap();
    let handles = match locate_handle_buffer(SearchType::ByProtocol(
        &uefi::proto::media::fs::SimpleFileSystem::GUID,
    )) {
        Ok(h) => h,
        Err(error) => {
            log::error!("Failed to locate filesystem handles: {error}");
            return error.status();
        }
    };

    let mut locator = FileFinder::new();

    for handle in handles.iter() {
        match locator.locate_normal_boot_files_in_fs(handle) {
            Ok(_) => {}
            Err(error) => {
                log::error!("Failed to locate files: {error}");
            }
        }
    }

    println!("found {} possible files", locator.len());
    for (i, file) in locator.iter().enumerate() {
        println!("{: >4}: {}", i, file)
    }
    if locator.len() == 0 {
        println!("No valid files found exiting");
        return Status::SUCCESS;
    }

    let input_option = loop {
        println!("Select a file to boot (empty to exit)");
        let Ok(input) = get_input_buffer() else {
            log::error!("Failed to get input buffer: Input device returned error");
            return Status::DEVICE_ERROR;
        };
        // user requested exit
        if input.len() == 0 {
            log::debug!("Input buffer empty, exiting");
            return Status::SUCCESS;
        }
        let Ok(num): Result<usize, _> = input.parse() else {
            println!("Failed to parse input buffer: {} unrecognised", input);
            continue;
        };
        if num >= locator.len() {
            println!("Invalid selection");
            continue;
        }
        break num;
    };

    let file = &mut locator[input_option];

    // We use BootSelection here because we are acting as a boot menu
    let child = match load_image(
        image_handle(),
        boot::LoadImageSource::FromDevicePath {
            device_path: file.path(),
            boot_policy: BootPolicy::BootSelection,
        },
    ) {
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
    stall(3_000_000);

    Status::SUCCESS
}

fn get_input_buffer() -> uefi::Result<alloc::string::String> {
    let mut input: ScopedProtocol<uefi::proto::console::text::Input> =
        open_protocol_exclusive(get_handle_for_protocol::<uefi::proto::console::text::Input>()?)?;
    let mut buffer = Vec::new();

    while let Some(_) = input.read_key()? {} // Ignore all buffered input
    loop {
        log::trace!("waiting for key event");
        let event = input
            .wait_for_key_event()
            .expect("The docs dont mention why this can fail");
        let _ = boot::wait_for_event(&mut [event]); // I actually dont care about the return code here
        let in_key = input.read_key();
        log::trace!("key event {:?}", in_key);
        match in_key {
            // input.read_key() {
            Ok(Some(Key::Printable(c))) => {
                print!("{}", c);
                if c == '\r' {
                    // I hate this
                    let mut output = CString16::new();
                    for i in buffer {
                        output.push(i);
                    }
                    return Ok(output.to_string());
                } else if c == '\u{8}' {
                    // backspace
                    let _ = buffer.pop();
                } else {
                    buffer.push(c);
                }
            }

            Err(e) => {
                return Err(e); // lol your keyboard broke
            }
            _ => {} // Ignore unknown input
        }
    }
}
