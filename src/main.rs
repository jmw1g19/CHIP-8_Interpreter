mod chip8;
mod graphics;

use std::{fs, env};
use rfd::FileDialog;
use crate::{chip8::CPU, graphics::WindowsSDL2};

fn main() {
    println!("      _     _       _____   _       _                           _            ");
    println!("     | |   (_)     |  _  | (_)     | |                         | |           ");
    println!("  ___| |__  _ _ __  \\ V /   _ _ __ | |_ ___ _ __ _ __  _ __ ___| |_ ___ _ __ ");
    println!(" / __| '_ \\| | '_ \\ / _ \\  | | '_ \\| __/ _ \\ '__| '_ \\| '__/ _ \\ __/ _ \\ '__|");
    println!("| (__| | | | | |_) | |_| | | | | | | ||  __/ |  | |_) | | |  __/ ||  __/ |  ");
    println!(" \\___|_| |_|_| .__/\\_____/ |_|_| |_|\\__\\___|_|  | .__/|_|  \\___|\\__\\___|_|   ");
    println!("             | |                                | |                          ");
    println!("             |_|                                |_|                          ");
    println!("--                Written by Joshua Wardle (buildz), 2023                  --");

    let args: Vec<String> = env::args().collect();
    
    let rom_path: String;

    if args.len() > 1 {
        rom_path = args[1].to_string();
    } else {
        let files = FileDialog::new()
        .add_filter("CHIP-8 ROM", &["ch8"])
        .set_directory("/")
        .pick_file();

        match files{
            Some(path) => { rom_path = path.into_os_string().into_string().unwrap(); },
            None => { println!("No game selected! Exiting..."); return; },
        }
    }
    
    let rom = fs::read(&rom_path).expect("ROM not readable! Exiting...");

    let mut emu = CPU::new();
    emu.load(&rom);

    let mut graphics_layer = WindowsSDL2::new();

    match graphics_layer.start_interpreter(&mut emu){
        Ok(()) => {  }
        Err(msg) => println!("An error occurred: {}", msg),
    }

}
