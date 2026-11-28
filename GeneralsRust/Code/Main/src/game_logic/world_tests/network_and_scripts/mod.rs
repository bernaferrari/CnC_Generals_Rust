//! Host GameLogic tests — `network_and_scripts`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod host_queries_and_visibility;
mod script_repair_and_locomotion;
mod scripts_and_capture;

mod line_build_tests;
