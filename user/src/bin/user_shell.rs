#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

extern crate alloc;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;

use alloc::string::String;
use alloc::vec::Vec;
use user_lib::console::getchar;
use user_lib::{close, dup, exec, fork, open, waitpid, OpenFlags};

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        let args: Vec<_> = line.as_str().split(' ').collect();
                        let mut args_copy: Vec<String> = args
                            .iter()
                            .map(|&arg| {
                                let mut string = String::new();
                                string.push_str(arg);
                                string.push('\0');
                                string
                            })
                            .collect();

                        // redirect input
                        let mut input = String::new();
                        if let Some((idx, _)) = args_copy
                            .iter()
                            .enumerate()
                            .find(|(_, arg)| arg.as_str() == "<\0")
                        {
                            input.clone_from(&args_copy[idx + 1]);
                            args_copy.drain(idx..=idx + 1);
                        }
                        if !input.is_empty() {
                            let input_fd = open(input.as_str(), OpenFlags::RDONLY);
                            if input_fd == -1 {
                                println!("Error when opening file {}", input);
                                return -4;
                            }
                            let input_fd = input_fd as usize;
                            close(0);
                            assert_eq!(dup(input_fd), 0);
                            close(input_fd);
                        }

                        // redirect output
                        let mut output = String::new();
                        if let Some((idx, _)) = args_copy
                            .iter()
                            .enumerate()
                            .find(|(_, arg)| arg.as_str() == ">\0")
                        {
                            output.clone_from(&args_copy[idx + 1]);
                            args_copy.drain(idx..=idx + 1);
                        }
                        if !output.is_empty() {
                            let output_fd =
                                open(output.as_str(), OpenFlags::CREATE | OpenFlags::WRONLY);
                            if output_fd == -1 {
                                println!("Error when opening file {}", output);
                                return -4;
                            }
                            let output_fd = output_fd as usize;
                            close(1);
                            assert_eq!(dup(output_fd), 1);
                            close(output_fd);
                        }

                        let mut args_addr: Vec<*const u8> =
                            args_copy.iter().map(|arg| arg.as_ptr()).collect();
                        args_addr.push(core::ptr::null::<u8>());
                        if exec(args_copy[0].as_str(), args_addr.as_slice()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{0} {0}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
