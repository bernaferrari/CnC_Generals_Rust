//! CrcDiff Module
//! 
//! Corresponds to C++ file: Tools/CRCDiff/CRCDiff.cpp
//! 
//! This module provides CRC difference comparison functionality for debug logs.

use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process;

use crate::expander::Expander;
use crate::misc::int_to_string;

const LINE_SIZE: usize = 1024;

pub struct CrcDiff {
    table_row: String,
    queued_lines: VecDeque<String>,
}

impl CrcDiff {
    pub fn new() -> Self {
        CrcDiff {
            table_row: String::new(),
            queued_lines: VecDeque::new(),
        }
    }

    pub fn run(&mut self, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        let (header, footer, in_files, out_file) = if args.len() != 7 {
            println!("Usage: crcdiff top.html row.html bottom.html in1.txt in2.txt out.txt");
            
            let header = self.read_file("top.html")?;
            self.table_row = self.read_file("row.html")?;
            let footer = self.read_file("bottom.html")?;
            let in_files = vec!["test1.txt".to_string(), "test2.txt".to_string()];
            let out_file = "out.html".to_string();
            
            (header, footer, in_files, out_file)
        } else {
            let header = self.read_file(&args[1])?;
            self.table_row = self.read_file(&args[2])?;
            let footer = self.read_file(&args[3])?;
            let in_files = vec![args[4].clone(), args[5].clone()];
            let out_file = args[6].clone();
            
            (header, footer, in_files, out_file)
        };

        let mut input_files = Vec::new();
        for fname in &in_files {
            let file = File::open(fname).map_err(|e| {
                eprintln!("Could not open {}: {}", fname, e);
                e
            })?;
            input_files.push(BufReader::new(file));
        }

        let mut output_file = File::create(&out_file)?;
        
        // Output header
        output_file.write_all(header.as_bytes())?;

        let mut last_line = vec![String::new(); 2];
        let mut last_frame = vec![-1i32; 2];
        let mut last_index = vec![-1i32; 2];
        let mut file_ok = vec![true; 2];
        
        let mut link_num = 1;
        let mut num_diffs = 0;
        let mut seen_right = false;
        let mut seen_left = false;
        
        while file_ok[0] || file_ok[1] {
            // Read lines if needed
            for i in 0..2 {
                if file_ok[i] && last_frame[i] < 0 {
                    let mut line = String::new();
                    let mut frame = 0i32;
                    let mut index = 0i32;
                    
                    file_ok[i] = self.get_next_line(&mut input_files[i], &mut line, &mut frame, &mut index);
                    if file_ok[i] {
                        last_line[i] = line;
                        last_frame[i] = frame;
                        last_index[i] = index;
                    }
                }
            }
            
            if file_ok[0] && file_ok[1] {
                if last_frame[0] < last_frame[1] ||
                   (last_frame[0] == last_frame[1] && last_index[0] < last_index[1]) {
                    if seen_right && seen_left {
                        self.output_diff_line(&mut output_file, last_frame[0], last_index[0], link_num,
                                             "leftOnly", &last_line[0], "", "")?;
                        link_num += 1;
                        num_diffs += 1;
                    }
                    last_frame[0] = -1;
                } else if last_frame[1] < last_frame[0] ||
                         (last_frame[1] == last_frame[0] && last_index[1] < last_index[0]) {
                    if seen_right && seen_left {
                        self.output_diff_line(&mut output_file, last_frame[1], last_index[1], link_num,
                                             "", "", "rightOnly", &last_line[1])?;
                        link_num += 1;
                        num_diffs += 1;
                    }
                    last_frame[1] = -1;
                } else {
                    // Same frame:index
                    if last_line[0] != last_line[1] {
                        if !seen_left || !seen_right {
                            println!("Seen both on {}:{}", last_frame[0], last_index[0]);
                        }
                        seen_left = true;
                        seen_right = true;
                        self.output_diff_line(&mut output_file, last_frame[0], last_index[0], link_num,
                                             "leftDiff", &last_line[0], "rightDiff", &last_line[1])?;
                        link_num += 1;
                        num_diffs += 1;
                    } else {
                        // Lines are the same
                        if seen_left && seen_right {
                            self.output_same_line(&mut output_file, last_frame[0], last_index[0], link_num,
                                                 &last_line[0])?;
                            num_diffs += 1;
                        } else {
                            self.queue_line(last_frame[0], last_index[0], &last_line[0]);
                        }
                    }
                    last_frame[0] = -1;
                    last_frame[1] = -1;
                }
            } else if file_ok[0] {
                if seen_right && seen_left {
                    self.output_diff_line(&mut output_file, last_frame[0], last_index[0], link_num,
                                         "leftOnly", &last_line[0], "", "")?;
                    link_num += 1;
                    num_diffs += 1;
                }
                last_frame[0] = -1;
            } else if file_ok[1] {
                if seen_right && seen_left {
                    self.output_diff_line(&mut output_file, last_frame[1], last_index[1], link_num,
                                         "", "", "rightOnly", &last_line[1])?;
                    link_num += 1;
                    num_diffs += 1;
                }
                last_frame[1] = -1;
            }
            
            if num_diffs > 1000 {
                break;
            }
        }
        
        // Output any queued lines
        self.dump_queued(&mut output_file)?;
        
        // Output footer
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("LAST".to_string(), int_to_string(link_num - 1));
        expander.add_expansion("BOTTOM".to_string(), int_to_string(link_num));
        let expanded_footer = expander.expand(&footer, false);
        output_file.write_all(expanded_footer.as_bytes())?;
        
        Ok(())
    }
    
    fn read_file(&self, filename: &str) -> Result<String, Box<dyn std::error::Error>> {
        std::fs::read_to_string(filename).map_err(|e| e.into())
    }
    
    fn get_next_line(&self, reader: &mut BufReader<File>, line: &mut String, frame: &mut i32, index: &mut i32) -> bool {
        line.clear();
        
        loop {
            match reader.read_line(line) {
                Ok(0) => return false, // EOF
                Ok(_) => {
                    // Remove trailing newline
                    if line.ends_with('\n') {
                        line.pop();
                    }
                    
                    // Try to parse frame:index format
                    let parts: Vec<&str> = line.splitn(3, ' ').collect();
                    if parts.len() >= 2 {
                        let frame_index_part = parts[0];
                        if let Some(colon_pos) = frame_index_part.find(':') {
                            let frame_str = &frame_index_part[..colon_pos];
                            let index_str = &frame_index_part[colon_pos + 1..];
                            
                            if let (Ok(parsed_frame), Ok(parsed_index)) = 
                                (frame_str.parse::<i32>(), index_str.parse::<i32>()) {
                                *frame = parsed_frame;
                                *index = parsed_index;
                                return true;
                            }
                        }
                    }
                    
                    line.clear();
                }
                Err(_) => return false,
            }
        }
    }
    
    fn output_diff_line(&mut self, output: &mut File, _frame: i32, _index: i32, link_num: i32,
                       class1: &str, line1: &str, class2: &str, line2: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.dump_queued(output)?;
        
        let left_class = if class1.is_empty() { "" } else { class1 };
        let right_class = if class2.is_empty() { "" } else { class2 };
        let left_line = if line1.is_empty() { "&nbsp;" } else { line1 };
        let right_line = if line2.is_empty() { "&nbsp;" } else { line2 };
        
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("LEFTCLASS".to_string(), left_class.to_string());
        expander.add_expansion("LEFTLINE".to_string(), left_line.to_string());
        expander.add_expansion("RIGHTCLASS".to_string(), right_class.to_string());
        expander.add_expansion("RIGHTLINE".to_string(), right_line.to_string());
        expander.add_expansion("NAME".to_string(), int_to_string(link_num));
        expander.add_expansion("PREV".to_string(), int_to_string(link_num - 1));
        expander.add_expansion("NEXT".to_string(), int_to_string(link_num + 1));
        
        let expanded = expander.expand(&self.table_row, false);
        output.write_all(expanded.as_bytes())?;
        
        Ok(())
    }
    
    fn output_same_line(&mut self, output: &mut File, _frame: i32, _index: i32, link_num: i32,
                       line: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.dump_queued(output)?;
        
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("LEFTCLASS".to_string(), "leftSame".to_string());
        expander.add_expansion("LEFTLINE".to_string(), line.to_string());
        expander.add_expansion("RIGHTCLASS".to_string(), "rightSame".to_string());
        expander.add_expansion("RIGHTLINE".to_string(), line.to_string());
        expander.add_expansion("NAME".to_string(), "".to_string());
        expander.add_expansion("PREV".to_string(), int_to_string(link_num - 1));
        expander.add_expansion("NEXT".to_string(), int_to_string(link_num));
        
        let expanded = expander.expand(&self.table_row, false);
        output.write_all(expanded.as_bytes())?;
        
        Ok(())
    }
    
    fn queue_line(&mut self, _frame: i32, _index: i32, line: &str) {
        let line_content = if line.is_empty() { "&nbsp;" } else { line };
        
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("LEFTCLASS".to_string(), "leftHistory".to_string());
        expander.add_expansion("LEFTLINE".to_string(), line_content.to_string());
        expander.add_expansion("RIGHTCLASS".to_string(), "rightHistory".to_string());
        expander.add_expansion("RIGHTLINE".to_string(), line_content.to_string());
        expander.add_expansion("NAME".to_string(), "".to_string());
        expander.add_expansion("PREV".to_string(), "0".to_string());
        expander.add_expansion("NEXT".to_string(), "1".to_string());
        
        let expanded = expander.expand(&self.table_row, false);
        self.queued_lines.push_back(expanded);
        
        if self.queued_lines.len() > 150 {
            self.queued_lines.pop_front();
        }
    }
    
    fn dump_queued(&mut self, output: &mut File) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(line) = self.queued_lines.pop_front() {
            output.write_all(line.as_bytes())?;
        }
        Ok(())
    }
}

impl Default for CrcDiff {
    fn default() -> Self {
        Self::new()
    }
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut crc_diff = CrcDiff::new();
    crc_diff.run(args)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_crc_diff_creation() {
        let crc_diff = CrcDiff::new();
        assert_eq!(crc_diff.queued_lines.len(), 0);
    }

    #[test]
    fn test_line_parsing() {
        let crc_diff = CrcDiff::new();
        let data = "1:5 some debug data\n2:10 more data\n";
        let mut reader = BufReader::new(Cursor::new(data));
        let mut line = String::new();
        let mut frame = 0;
        let mut index = 0;
        
        assert!(crc_diff.get_next_line(&mut reader, &mut line, &mut frame, &mut index));
        assert_eq!(frame, 1);
        assert_eq!(index, 5);
        
        assert!(crc_diff.get_next_line(&mut reader, &mut line, &mut frame, &mut index));
        assert_eq!(frame, 2);
        assert_eq!(index, 10);
    }
    
    #[test]
    fn test_queue_functionality() {
        let mut crc_diff = CrcDiff::new();
        crc_diff.table_row = "((LEFTLINE)) - ((RIGHTLINE))".to_string();
        
        crc_diff.queue_line(1, 5, "test line");
        assert_eq!(crc_diff.queued_lines.len(), 1);
        
        // Test queue limit
        for i in 0..200 {
            crc_diff.queue_line(i, i, &format!("line {}", i));
        }
        assert_eq!(crc_diff.queued_lines.len(), 150);
    }
}
