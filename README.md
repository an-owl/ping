# Blip

Blip is a UEFI application to locate and start bootloaders in the event the boot-entries have been lost.

## Operation

Blip iterates over the all the handles with the EFI_SIMPLE_FILE_SYSTEM_PROTOCOL and searches for any file matching the 
regex pattern `/efi/(.*?)/(.*?).efi`. When this is completed it will allow the user to select which file to run which 
will (hopefully) boot the operating system.

Files are listed with their full device-path which can be used to identify different drives.

Because Blip is not signed 

## Installation

Blip should be installed onto an EFI system partition formatted as FAT32 in the path `/EFI/BOOT/BOOT$arch`
for x86_64 which this is likely to be run on the full path should be `/EFI/BOOT/BOOTx64`.
This is default search path that the UEFI specification defines for bootloaders as defined in the UEFI Specification 2.9 section 3.5.1.1.

## Building

To build this project from source requires a rust compiler version 1.81. Installation instructions can be found (here)[https://doc.rust-lang.org/stable/book/ch01-01-installation.html].
This requires the appropriate target to be installed, for x86_64 targets this can be done using `cargo target add x86_64-unknown-uefi`.
With these installed the project can be built by running `cargo build --release --target=x86_64-unknown-uefi` in the project root. 
This will build a COFF file to `.target/x86_64-unknown-none/release/blip.efi` which can then be installed by following 
the (installation)[#Installation] instructions.