// FMI-1: FMI 2.0 Co-Simulation export. FMI-2: FMI 2.0 Model Exchange export.
// Generates modelDescription.xml and fmi2_cs.c (CS) or fmi2_me.c (ME) that wrap model.c residual(); user compiles and zips as FMU.

use std::env;
use std::io::Write;

include!("fmi_part1.rs");
include!("fmi_part2.rs");
