//! Result Interface Module
//!
//! Equivalent to the C++ ProfileResultInterface, provides interfaces for
//! writing out profiling results in various formats.

#[cfg(feature = "function-level")]
use crate::func_level::ProfileFuncId;
use crate::Profile;
use std::fs::File;
use std::io::{self, Write};

/// Result function interface - equivalent to ProfileResultInterface
pub trait ProfileResultInterface: Send + Sync {
    /// Write out results (called on program exit)
    fn write_results(&self);

    /// Get a name for this result function
    fn get_name(&self) -> &str;
}

/// C++-style CSV file result writer - equivalent to ProfileResultFileCSV
pub struct FileCsvResultWriter;

impl FileCsvResultWriter {
    pub fn create(_args: &[&str]) -> Box<dyn ProfileResultInterface> {
        Box::new(Self)
    }

    #[cfg(feature = "function-level")]
    fn write_thread_csv(&self, writer: &mut dyn Write, thread_id: usize) -> io::Result<()> {
        writeln!(
            writer,
            "Function\tFile\tCall count\tPTT (all)\tGTT (all)\tPT/C (all)\tGT/C (all)\tCaller (all)"
        )?;

        let frame_count = Profile::get_frame_count();
        for frame in 0..frame_count {
            let frame_name =
                Profile::get_frame_name(frame).unwrap_or_else(|| format!("frame:{}", frame));
            write!(
                writer,
                "\tCall ({0})\tPTT ({0})\tGTT ({0})\tPT/C ({0})\tGT/C ({0})\tCaller ({0})",
                frame_name
            )?;
        }
        writeln!(writer)?;

        let func_level = Profile::func_level();
        let mut thread_index = 0;
        while let Some(thread) = func_level.enum_threads(thread_index) {
            if thread.get_id() == thread_id {
                let mut func_index = 0;
                while let Some(func_id) = thread.enum_profile(func_index) {
                    let function_name = func_id.get_function().unwrap_or("unknown");
                    let source = func_id.get_source().unwrap_or("unknown");
                    let address = func_id.get_address();
                    let line = func_id.get_line();

                    write!(
                        writer,
                        "{}[{:08x}]\t{}, {}",
                        function_name, address, source, line
                    )?;

                    let frames = std::iter::once(ProfileFuncId::TOTAL)
                        .chain((0..frame_count).map(|v| v as u32));

                    for frame in frames {
                        let calls = func_id.get_calls(frame);
                        if calls == 0 {
                            write!(writer, "\t\t\t\t\t\t")?;
                            continue;
                        }

                        let function_time = func_id.get_function_time(frame);
                        let total_time = func_id.get_time(frame);

                        let pt_per_call = function_time / calls;
                        let gt_per_call = total_time / calls;

                        write!(
                            writer,
                            "\t{}\t{}\t{}\t{}\t{}\t",
                            calls, function_time, total_time, pt_per_call, gt_per_call
                        )?;

                        let callers = func_id.get_caller(frame);
                        for idx in 0..callers.len() {
                            if let Some((caller, count)) = callers.enumerate(idx) {
                                let caller_fn = caller.get_function().unwrap_or("unknown");
                                let caller_addr = caller.get_address();
                                write!(
                                    writer,
                                    " {}[{:08x}]({})",
                                    caller_fn,
                                    caller_addr,
                                    count.unwrap_or(0)
                                )?;
                            }
                        }
                    }

                    writeln!(writer)?;
                    func_index += 1;
                }
                break;
            }
            thread_index += 1;
        }

        Ok(())
    }

    #[cfg(not(feature = "function-level"))]
    fn write_thread_csv(&self, _writer: &mut dyn Write, _thread_id: usize) -> io::Result<()> {
        Ok(())
    }

    fn write_high_level_csv(&self) -> io::Result<()> {
        let mut file = File::create("profile-high.csv")?;

        write!(file, "Profile\tUnit\ttotal")?;
        let frame_count = Profile::get_frame_count();
        for frame in 0..frame_count {
            let frame_name =
                Profile::get_frame_name(frame).unwrap_or_else(|| format!("frame:{}", frame));
            write!(file, "\t{}", frame_name)?;
        }
        writeln!(file)?;

        let high_level = Profile::high_level();
        let mut index = 0;
        while let Some(id) = high_level.enum_profile(index) {
            write!(
                file,
                "{}\t{}\t{}",
                id.get_name(),
                id.get_unit(),
                id.get_total_value()
            )?;
            for frame in 0..frame_count {
                let value = id.get_value(frame).unwrap_or_else(|| "".to_string());
                write!(file, "\t{}", value)?;
            }
            writeln!(file)?;
            index += 1;
        }

        Ok(())
    }
}

impl ProfileResultInterface for FileCsvResultWriter {
    fn write_results(&self) {
        #[cfg(feature = "function-level")]
        {
            let func_level = Profile::func_level();
            let mut thread_index = 0;
            while let Some(thread) = func_level.enum_threads(thread_index) {
                let filename = format!("prof{:08x}-all.csv", thread.get_id());
                if let Ok(mut file) = File::create(&filename) {
                    let _ = self.write_thread_csv(&mut file, thread.get_id());
                }
                thread_index += 1;
            }
        }

        let _ = self.write_high_level_csv();
    }

    fn get_name(&self) -> &str {
        "file_csv"
    }
}

/// DOT file result writer - equivalent to ProfileResultFileDOT
pub struct DotResultWriter {
    filename: String,
    frame_name: Option<String>,
    fold_threshold: usize,
}

impl DotResultWriter {
    pub fn create(args: &[&str]) -> Box<dyn ProfileResultInterface> {
        let filename = args.get(0).copied().unwrap_or("profile.dot");
        let frame_name = args.get(1).map(|s| s.to_string());
        let fold_threshold = args
            .get(2)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        Box::new(Self {
            filename: filename.to_string(),
            frame_name,
            fold_threshold,
        })
    }

    #[cfg(feature = "function-level")]
    fn resolve_frame(&self) -> u32 {
        if let Some(frame_name) = &self.frame_name {
            let frame_count = Profile::get_frame_count();
            for frame in 0..frame_count {
                if let Some(name) = Profile::get_frame_name(frame) {
                    if name == *frame_name {
                        return frame as u32;
                    }
                }
            }
        }
        ProfileFuncId::TOTAL
    }

    #[cfg(feature = "function-level")]
    fn pick_thread(&self) -> Option<crate::func_level::ProfileFuncThread> {
        let func_level = Profile::func_level();
        let mut thread_index = 0;
        let mut best_thread: Option<crate::func_level::ProfileFuncThread> = None;
        let mut best_count = 0usize;
        while let Some(thread) = func_level.enum_threads(thread_index) {
            let mut count = 0usize;
            let mut func_index = 0;
            while thread.enum_profile(func_index).is_some() {
                count += 1;
                func_index += 1;
            }
            if count > best_count {
                best_count = count;
                best_thread = Some(thread);
            }
            thread_index += 1;
        }
        best_thread
    }

    #[cfg(feature = "function-level")]
    fn write_folded(
        &self,
        writer: &mut dyn Write,
        thread: &crate::func_level::ProfileFuncThread,
        frame: u32,
    ) -> io::Result<()> {
        const MAX_FUNCTIONS_PER_FILE: usize = 200;
        use std::collections::HashMap;

        #[derive(Default)]
        struct FoldHelper {
            ids: Vec<crate::func_level::ProfileFuncId>,
            mark: bool,
        }

        let mut fold: HashMap<String, FoldHelper> = HashMap::new();

        let mut func_index = 0;
        while let Some(id) = thread.enum_profile(func_index) {
            let source = id.get_source().unwrap_or("unknown").to_string();
            let entry = fold.entry(source).or_default();
            if entry.ids.len() < MAX_FUNCTIONS_PER_FILE {
                entry.ids.push(id);
            }
            func_index += 1;
        }

        let sources: Vec<String> = fold.keys().cloned().collect();

        for source in &sources {
            for helper in fold.values_mut() {
                helper.mark = false;
            }

            let ids = fold
                .get(source)
                .map(|entry| entry.ids.clone())
                .unwrap_or_default();

            for id in ids {
                let callers = id.get_caller(frame);
                for idx in 0..callers.len() {
                    if let Some((caller, _count)) = callers.enumerate(idx) {
                        let caller_source = caller.get_source().unwrap_or("unknown");
                        if let Some(entry) = fold.get_mut(caller_source) {
                            if entry.mark {
                                continue;
                            }
                            entry.mark = true;
                            writeln!(writer, "\"{}\" -> \"{}\"", caller_source, source)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "function-level")]
    fn write_unfolded(
        &self,
        writer: &mut dyn Write,
        thread: &crate::func_level::ProfileFuncThread,
        frame: u32,
    ) -> io::Result<()> {
        let mut func_index = 0;
        while let Some(id) = thread.enum_profile(func_index) {
            if id.get_calls(frame) != 0 {
                writeln!(
                    writer,
                    "f{:08x} [label=\"{}\"]",
                    id.get_address(),
                    id.get_function().unwrap_or("unknown")
                )?;
            }
            func_index += 1;
        }

        let mut func_index = 0;
        while let Some(id) = thread.enum_profile(func_index) {
            let callers = id.get_caller(frame);
            for idx in 0..callers.len() {
                if let Some((caller, count)) = callers.enumerate(idx) {
                    let count = count.unwrap_or(0);
                    writeln!(
                        writer,
                        "f{:08x} -> f{:08x} [headlabel=\"{}\"];\n",
                        caller.get_address(),
                        id.get_address(),
                        count
                    )?;
                }
            }
            func_index += 1;
        }

        Ok(())
    }
}

impl ProfileResultInterface for DotResultWriter {
    fn write_results(&self) {
        #[cfg(feature = "function-level")]
        {
            let thread = match self.pick_thread() {
                Some(thread) => thread,
                None => return,
            };

            let frame = self.resolve_frame();

            let mut active = 0usize;
            let mut func_index = 0;
            while let Some(id) = thread.enum_profile(func_index) {
                if id.get_calls(frame) != 0 {
                    active += 1;
                }
                func_index += 1;
            }

            let mut file = match File::create(&self.filename) {
                Ok(file) => file,
                Err(_) => return,
            };

            let arrowhead = if active > self.fold_threshold {
                "closed"
            } else {
                "none"
            };

            let _ = writeln!(file, "digraph G {{ rankdir=\"LR\";");
            let _ = writeln!(file, "node [shape=box, fontname=Arial]");
            let _ = writeln!(
                file,
                "edge [arrowhead={}, labelfontname=Arial, labelfontsize=10, labelangle=0, labelfontcolor=blue]",
                arrowhead
            );

            if active > self.fold_threshold {
                let _ = self.write_folded(&mut file, &thread, frame);
            } else {
                let _ = self.write_unfolded(&mut file, &thread, frame);
            }

            let _ = writeln!(file, "}}");
        }
    }

    fn get_name(&self) -> &str {
        "file_dot"
    }
}

/// CSV file result writer - equivalent to ProfileResultFileCSV
pub struct CsvResultWriter {
    filename: String,
    include_high_level: bool,
    include_func_level: bool,
}

impl CsvResultWriter {
    /// Create a new CSV result writer
    pub fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            include_high_level: true,
            include_func_level: true,
        }
    }

    /// Create a CSV writer with specific options
    pub fn with_options(
        filename: &str,
        include_high_level: bool,
        include_func_level: bool,
    ) -> Self {
        Self {
            filename: filename.to_string(),
            include_high_level,
            include_func_level,
        }
    }

    /// Factory function for creating CSV writers from command line arguments
    pub fn create(args: &[&str]) -> Box<dyn ProfileResultInterface> {
        let filename = args.first().unwrap_or(&"profile_results.csv");
        Box::new(Self::new(filename))
    }

    /// Write high-level profiling results to CSV
    fn write_high_level_csv(&self, writer: &mut dyn Write) -> io::Result<()> {
        if !self.include_high_level {
            return Ok(());
        }

        writeln!(writer, "# High Level Profiling Results")?;
        writeln!(writer, "Name,Description,Unit,Current Value,Total Value")?;

        let high_level = Profile::high_level();
        let mut index = 0;

        while let Some(id) = high_level.enum_profile(index) {
            let name = id.get_name().replace(',', "_"); // CSV-safe
            let desc = id.get_description().replace(',', "_");
            let unit = id.get_unit().replace(',', "_");
            let current = id.get_current_value().replace(',', "_");
            let total = id.get_total_value().replace(',', "_");

            writeln!(writer, "{},{},{},{},{}", name, desc, unit, current, total)?;
            index += 1;
        }

        writeln!(writer)?; // Empty line separator
        Ok(())
    }

    /// Write function-level profiling results to CSV
    #[cfg(feature = "function-level")]
    fn write_func_level_csv(&self, writer: &mut dyn Write) -> io::Result<()> {
        if !self.include_func_level {
            return Ok(());
        }

        writeln!(writer, "# Function Level Profiling Results")?;
        writeln!(
            writer,
            "Thread ID,Function,Source,Line,Address,Total Calls,Total Time,Function Time"
        )?;

        let func_level = Profile::func_level();
        let mut thread_index = 0;

        while let Some(thread) = func_level.enum_threads(thread_index) {
            let thread_id = thread.get_id();
            let mut func_index = 0;

            while let Some(func_id) = thread.enum_profile(func_index) {
                let function_name = func_id
                    .get_function()
                    .unwrap_or("unknown")
                    .replace(',', "_");
                let source = func_id.get_source().unwrap_or("unknown").replace(',', "_");
                let line = func_id.get_line();
                let address = func_id.get_address();
                let total_calls = func_id.get_calls(crate::func_level::ProfileFuncId::TOTAL);
                let total_time = func_id.get_time(crate::func_level::ProfileFuncId::TOTAL);
                let function_time =
                    func_id.get_function_time(crate::func_level::ProfileFuncId::TOTAL);

                writeln!(
                    writer,
                    "{},{},{},{},{:#x},{},{},{}",
                    thread_id,
                    function_name,
                    source,
                    line,
                    address,
                    total_calls,
                    total_time,
                    function_time
                )?;

                func_index += 1;
            }

            thread_index += 1;
        }

        writeln!(writer)?; // Empty line separator
        Ok(())
    }

    #[cfg(not(feature = "function-level"))]
    fn write_func_level_csv(&self, _writer: &mut dyn Write) -> io::Result<()> {
        Ok(()) // No-op when function-level profiling is disabled
    }

    /// Write frame information to CSV
    fn write_frame_info_csv(&self, writer: &mut dyn Write) -> io::Result<()> {
        writeln!(writer, "# Frame Information")?;
        writeln!(writer, "Frame Number,Frame Name")?;

        let frame_count = Profile::get_frame_count();
        for i in 0..frame_count {
            if let Some(frame_name) = Profile::get_frame_name(i) {
                let safe_name = frame_name.replace(',', "_");
                writeln!(writer, "{},{}", i, safe_name)?;
            }
        }

        Ok(())
    }
}

impl ProfileResultInterface for CsvResultWriter {
    fn write_results(&self) {
        match self.write_csv_file() {
            Ok(()) => log::info!("Successfully wrote CSV results to {}", self.filename),
            Err(e) => log::error!("Failed to write CSV results to {}: {}", self.filename, e),
        }
    }

    fn get_name(&self) -> &str {
        "csv_file_writer"
    }
}

impl CsvResultWriter {
    fn write_csv_file(&self) -> io::Result<()> {
        let mut file = File::create(&self.filename)?;

        // Write header
        writeln!(file, "# Profile Results CSV Export")?;
        writeln!(file, "# Generated by Profile-Rust Library")?;
        writeln!(
            file,
            "# CPU Frequency: {} Hz",
            Profile::get_clock_cycles_per_second().unwrap_or(0)
        )?;
        writeln!(
            file,
            "# Total Frames Recorded: {}",
            Profile::get_frame_count()
        )?;
        writeln!(file)?;

        // Write high-level results
        self.write_high_level_csv(&mut file)?;

        // Write function-level results
        self.write_func_level_csv(&mut file)?;

        // Write frame information
        self.write_frame_info_csv(&mut file)?;

        file.flush()?;
        Ok(())
    }
}

/// HTML result writer for generating web-viewable reports
pub struct HtmlResultWriter {
    filename: String,
}

impl HtmlResultWriter {
    /// Create a new HTML result writer
    pub fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
        }
    }

    /// Factory function for creating HTML writers
    pub fn create(args: &[&str]) -> Box<dyn ProfileResultInterface> {
        let filename = args.first().unwrap_or(&"profile_results.html");
        Box::new(Self::new(filename))
    }

    fn write_html_file(&self) -> io::Result<()> {
        let mut file = File::create(&self.filename)?;

        // Write HTML header
        writeln!(file, "<!DOCTYPE html>")?;
        writeln!(file, "<html>")?;
        writeln!(file, "<head>")?;
        writeln!(file, "    <title>Profile Results</title>")?;
        writeln!(file, "    <style>")?;
        writeln!(file, "        body {{ font-family: Arial, sans-serif; }}")?;
        writeln!(
            file,
            "        table {{ border-collapse: collapse; width: 100%; }}"
        )?;
        writeln!(
            file,
            "        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}"
        )?;
        writeln!(file, "        th {{ background-color: #f2f2f2; }}")?;
        writeln!(file, "        .section {{ margin: 20px 0; }}")?;
        writeln!(file, "    </style>")?;
        writeln!(file, "</head>")?;
        writeln!(file, "<body>")?;
        writeln!(file, "    <h1>Profile Results</h1>")?;

        // Write system information
        writeln!(file, "    <div class=\"section\">")?;
        writeln!(file, "        <h2>System Information</h2>")?;
        writeln!(
            file,
            "        <p>CPU Frequency: {} Hz</p>",
            Profile::get_clock_cycles_per_second().unwrap_or(0)
        )?;
        writeln!(
            file,
            "        <p>Total Frames Recorded: {}</p>",
            Profile::get_frame_count()
        )?;
        writeln!(file, "    </div>")?;

        // Write high-level profiling results
        self.write_high_level_html(&mut file)?;

        // Write function-level profiling results
        self.write_func_level_html(&mut file)?;

        // Write frame information
        self.write_frame_info_html(&mut file)?;

        // Close HTML
        writeln!(file, "</body>")?;
        writeln!(file, "</html>")?;

        file.flush()?;
        Ok(())
    }

    fn write_high_level_html(&self, writer: &mut dyn Write) -> io::Result<()> {
        writeln!(writer, "    <div class=\"section\">")?;
        writeln!(writer, "        <h2>High Level Profiling</h2>")?;
        writeln!(writer, "        <table>")?;
        writeln!(writer, "            <tr><th>Name</th><th>Description</th><th>Unit</th><th>Current</th><th>Total</th></tr>")?;

        let high_level = Profile::high_level();
        let mut index = 0;

        while let Some(id) = high_level.enum_profile(index) {
            let name = html_escape(id.get_name());
            let desc = html_escape(id.get_description());
            let unit = html_escape(id.get_unit());
            let current = html_escape(&id.get_current_value());
            let total = html_escape(&id.get_total_value());

            writeln!(
                writer,
                "            <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                name, desc, unit, current, total
            )?;
            index += 1;
        }

        writeln!(writer, "        </table>")?;
        writeln!(writer, "    </div>")?;
        Ok(())
    }

    #[cfg(feature = "function-level")]
    fn write_func_level_html(&self, writer: &mut dyn Write) -> io::Result<()> {
        writeln!(writer, "    <div class=\"section\">")?;
        writeln!(writer, "        <h2>Function Level Profiling</h2>")?;
        writeln!(writer, "        <table>")?;
        writeln!(writer, "            <tr><th>Thread</th><th>Function</th><th>Source</th><th>Line</th><th>Address</th><th>Calls</th><th>Total Time</th><th>Function Time</th></tr>")?;

        let func_level = Profile::func_level();
        let mut thread_index = 0;

        while let Some(thread) = func_level.enum_threads(thread_index) {
            let thread_id = thread.get_id();
            let mut func_index = 0;

            while let Some(func_id) = thread.enum_profile(func_index) {
                let function_name = html_escape(func_id.get_function().unwrap_or("unknown"));
                let source = html_escape(func_id.get_source().unwrap_or("unknown"));
                let line = func_id.get_line();
                let address = func_id.get_address();
                let total_calls = func_id.get_calls(crate::func_level::ProfileFuncId::TOTAL);
                let total_time = func_id.get_time(crate::func_level::ProfileFuncId::TOTAL);
                let function_time =
                    func_id.get_function_time(crate::func_level::ProfileFuncId::TOTAL);

                writeln!(
                    writer,
                    "            <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:#x}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    thread_id, function_name, source, line, address,
                    total_calls, total_time, function_time
                )?;

                func_index += 1;
            }

            thread_index += 1;
        }

        writeln!(writer, "        </table>")?;
        writeln!(writer, "    </div>")?;
        Ok(())
    }

    #[cfg(not(feature = "function-level"))]
    fn write_func_level_html(&self, _writer: &mut dyn Write) -> io::Result<()> {
        Ok(()) // No-op when function-level profiling is disabled
    }

    fn write_frame_info_html(&self, writer: &mut dyn Write) -> io::Result<()> {
        writeln!(writer, "    <div class=\"section\">")?;
        writeln!(writer, "        <h2>Frame Information</h2>")?;
        writeln!(writer, "        <table>")?;
        writeln!(
            writer,
            "            <tr><th>Frame Number</th><th>Frame Name</th></tr>"
        )?;

        let frame_count = Profile::get_frame_count();
        for i in 0..frame_count {
            if let Some(frame_name) = Profile::get_frame_name(i) {
                let safe_name = html_escape(&frame_name);
                writeln!(
                    writer,
                    "            <tr><td>{}</td><td>{}</td></tr>",
                    i, safe_name
                )?;
            }
        }

        writeln!(writer, "        </table>")?;
        writeln!(writer, "    </div>")?;
        Ok(())
    }
}

impl ProfileResultInterface for HtmlResultWriter {
    fn write_results(&self) {
        match self.write_html_file() {
            Ok(()) => log::info!("Successfully wrote HTML results to {}", self.filename),
            Err(e) => log::error!("Failed to write HTML results to {}: {}", self.filename, e),
        }
    }

    fn get_name(&self) -> &str {
        "html_file_writer"
    }
}

/// Console result writer for outputting results to stdout
pub struct ConsoleResultWriter {
    verbose: bool,
}

impl ConsoleResultWriter {
    /// Create a new console result writer
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Factory function for creating console writers
    pub fn create(args: &[&str]) -> Box<dyn ProfileResultInterface> {
        let verbose = args.first().map(|&s| s == "verbose").unwrap_or(false);
        Box::new(Self::new(verbose))
    }
}

impl ProfileResultInterface for ConsoleResultWriter {
    fn write_results(&self) {
        println!("\n=== PROFILE RESULTS ===");
        println!(
            "CPU Frequency: {} Hz",
            Profile::get_clock_cycles_per_second().unwrap_or(0)
        );
        println!("Total Frames: {}\n", Profile::get_frame_count());

        // High-level results
        println!("HIGH LEVEL PROFILING:");
        println!(
            "{:<40} {:<15} {:<10} {:<15}",
            "Name", "Current", "Total", "Unit"
        );
        println!("{}", "-".repeat(80));

        let high_level = Profile::high_level();
        let mut index = 0;

        while let Some(id) = high_level.enum_profile(index) {
            println!(
                "{:<40} {:<15} {:<15} {:<10}",
                id.get_name(),
                id.get_current_value(),
                id.get_total_value(),
                id.get_unit()
            );

            if self.verbose {
                println!("  Description: {}", id.get_description());
            }

            index += 1;
        }

        // Function-level results (if enabled)
        #[cfg(feature = "function-level")]
        {
            let func_level = Profile::func_level();
            if func_level.get_thread_count() > 0 {
                println!("\nFUNCTION LEVEL PROFILING:");
                println!(
                    "{:<10} {:<30} {:<20} {:<10} {:<10}",
                    "Thread", "Function", "Source", "Calls", "Time"
                );
                println!("{}", "-".repeat(80));

                let mut thread_index = 0;
                while let Some(thread) = func_level.enum_threads(thread_index) {
                    let mut func_index = 0;
                    while let Some(func_id) = thread.enum_profile(func_index) {
                        println!(
                            "{:<10} {:<30} {:<20} {:<10} {:<10}",
                            thread.get_id(),
                            func_id.get_function().unwrap_or("unknown"),
                            func_id.get_source().unwrap_or("unknown"),
                            func_id.get_calls(crate::func_level::ProfileFuncId::TOTAL),
                            func_id.get_time(crate::func_level::ProfileFuncId::TOTAL)
                        );
                        func_index += 1;
                    }
                    thread_index += 1;
                }
            }
        }

        println!("\n=== END PROFILE RESULTS ===\n");
    }

    fn get_name(&self) -> &str {
        "console_writer"
    }
}

/// Simple HTML escaping function
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Result function registry for managing different output formats
pub struct ResultFunctionRegistry;

impl ResultFunctionRegistry {
    /// Get a list of available result functions
    pub fn get_available_functions() -> Vec<(&'static str, &'static str)> {
        vec![
            ("file_csv", "Write results to C++-style CSV files"),
            ("file_dot", "Write results to DOT call graph"),
            ("csv_file", "[ filename ] - Write results to CSV file"),
            ("html_file", "[ filename ] - Write results to HTML file"),
            ("console", "[ verbose ] - Write results to console"),
        ]
    }

    /// Create a result function by name
    pub fn create_function(name: &str, args: &[&str]) -> Option<Box<dyn ProfileResultInterface>> {
        match name {
            "file_csv" => Some(FileCsvResultWriter::create(args)),
            "file_dot" => Some(DotResultWriter::create(args)),
            "csv_file" => Some(CsvResultWriter::create(args)),
            "html_file" => Some(HtmlResultWriter::create(args)),
            "console" => Some(ConsoleResultWriter::create(args)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_csv_writer() {
        let temp_file = "test_results.csv";

        // Create some profile data first
        Profile::clear_patterns();
        Profile::add_pattern("test*", true).unwrap();
        Profile::start_range(Some("test_csv")).unwrap();

        let high_level = Profile::high_level();
        let id = high_level
            .add_profile("test.csv", "CSV Test", "count", 0, 0)
            .unwrap();
        id.increment(42.0);

        Profile::stop_range(Some("test_csv")).unwrap();

        // Write CSV results
        let writer = CsvResultWriter::new(temp_file);
        writer.write_results();

        // Check that file was created
        assert!(PathBuf::from(temp_file).exists());

        // Read and verify content
        let content = fs::read_to_string(temp_file).unwrap();
        assert!(content.contains("Profile Results CSV Export"));
        assert!(content.contains("High Level Profiling Results"));
        assert!(content.contains("test.csv"));

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_html_writer() {
        let temp_file = "test_results.html";

        // Create some profile data
        let high_level = Profile::high_level();
        let id = high_level
            .add_profile("test.html", "HTML Test", "bytes", 2, 0)
            .unwrap();
        id.increment(1024.0);

        // Write HTML results
        let writer = HtmlResultWriter::new(temp_file);
        writer.write_results();

        // Check that file was created
        assert!(PathBuf::from(temp_file).exists());

        // Read and verify content
        let content = fs::read_to_string(temp_file).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Profile Results"));
        assert!(content.contains("test.html"));
        assert!(content.contains("HTML Test"));

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_console_writer() {
        // Just test that it doesn't panic
        let writer = ConsoleResultWriter::new(false);
        writer.write_results();

        let writer = ConsoleResultWriter::new(true);
        writer.write_results();
    }

    #[test]
    fn test_result_function_registry() {
        let functions = ResultFunctionRegistry::get_available_functions();
        assert_eq!(functions.len(), 5);

        let file_csv_writer = ResultFunctionRegistry::create_function("file_csv", &[]);
        assert!(file_csv_writer.is_some());
        assert_eq!(file_csv_writer.unwrap().get_name(), "file_csv");

        let file_dot_writer = ResultFunctionRegistry::create_function("file_dot", &[]);
        assert!(file_dot_writer.is_some());
        assert_eq!(file_dot_writer.unwrap().get_name(), "file_dot");

        let csv_writer = ResultFunctionRegistry::create_function("csv_file", &["test.csv"]);
        assert!(csv_writer.is_some());
        assert_eq!(csv_writer.unwrap().get_name(), "csv_file_writer");

        let html_writer = ResultFunctionRegistry::create_function("html_file", &["test.html"]);
        assert!(html_writer.is_some());
        assert_eq!(html_writer.unwrap().get_name(), "html_file_writer");

        let console_writer = ResultFunctionRegistry::create_function("console", &["verbose"]);
        assert!(console_writer.is_some());
        assert_eq!(console_writer.unwrap().get_name(), "console_writer");

        let unknown = ResultFunctionRegistry::create_function("unknown", &[]);
        assert!(unknown.is_none());
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("normal text"), "normal text");
        assert_eq!(
            html_escape("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
        assert_eq!(html_escape("A & B"), "A &amp; B");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }
}
