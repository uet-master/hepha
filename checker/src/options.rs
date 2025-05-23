// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{Arg, Command};
use itertools::Itertools;

use hepha_annotations::*;
use rustc_session::EarlyDiagCtxt;

/// Creates the clap::Command metadata for argument parsing.
fn make_options_parser(running_test_harness: bool) -> Command {
    // We could put this into lazy_static! with a Mutex around, but we really do not expect
    // to construct this more than once per regular program run.
    let mut parser = Command::new("HEPHA")
        .no_binary_name(true)
        .version("v1.1.7")
        .arg(Arg::new("single_func")
            .long("single_func")
            .num_args(1)
            .help("Focus analysis on the named function.")
            .long_help("Name is the simple name of a top-level crate function or a HEPHA summary key."))
        .arg(Arg::new("diag")
            .long("diag")
            .num_args(1)
            .value_parser(["default", "verify", "library", "paranoid"])
            .default_value("default")
            .help("Level of diagnostics.\n")
            .long_help("With `default`, false positives will be avoided where possible.\nWith 'verify' errors are reported for incompletely analyzed functions.\nWith `paranoid`, all possible errors will be reported.\n"))
        .arg(Arg::new("constant_time")
            .long("constant_time")
            .num_args(1)
            .help("Enable verification of constant-time security.")
            .long_help("Name is a top-level crate type"))
        .arg(Arg::new("body_analysis_timeout")
            .long("body_analysis_timeout")
            .num_args(1)
            .default_value("30")
            .help("The maximum number of seconds that HEPHA will spend analyzing a function body.")
            .long_help("The default is 30 seconds."))
        .arg(Arg::new("crate_analysis_timeout")
            .long("crate_analysis_timeout")
            .num_args(1)
            .default_value("240")
            .help("The maximum number of seconds that HEPHA will spend analyzing a function body.")
            .long_help("The default is 240 seconds."))
        .arg(Arg::new("statistics")
            .long("statistics")
            .num_args(0)
            .help("Just print out whether crates were analyzed, etc.")
            .long_help("Just print out whether crates were analyzed and how many diagnostics were produced for each crate."))
        .arg(Arg::new("call_graph_config")
            .long("call_graph_config")
            .num_args(1)
            .help("Path call graph config.")
            .long_help(r#"Path to a JSON file that configures call graph output. Please see the documentation for details (https://github.com/endorlabs/HEPHA/blob/main/documentation/CallGraph.md)."#))
        .arg(Arg::new("print_function_names")
            .long("print_function_names")
            .num_args(0)
            .help("Just print out the signatures of functions in the crate"))
        .arg(Arg::new("print_summaries")
            .long("print_summaries")
            .num_args(0)
            .help("Print out function summaries (work in progress)"));
    if running_test_harness {
        parser = parser.arg(Arg::new("test_only")
            .long("test_only")
            .num_args(0)
            .help("Focus analysis on #[test] methods.")
            .long_help("Only #[test] methods and their usage are analyzed. This must be used together with the rustc --test option."));
    }
    parser
}

/// Represents options passed to HEPHA.
#[derive(Debug, Default)]
pub struct Options {
    pub single_func: Option<String>,
    pub test_only: bool,
    pub diag_level: DiagLevel,
    pub constant_time_tag_name: Option<String>,
    pub max_analysis_time_for_body: u64,
    pub max_analysis_time_for_crate: u64,
    pub statistics: bool,
    pub call_graph_config: Option<String>,
    pub print_function_names: bool,
    pub print_summaries: bool,
}

/// Represents diag level.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd)]
pub enum DiagLevel {
    /// When a function calls a function without a body and with no foreign function summary, the call assumed to be
    /// correct and any diagnostics that depend on the result of the call in some way are suppressed.
    #[default]
    Default,
    /// Like Default, but emit a diagnostic if there is a call to a function without a body and with no foreign function summary.
    Verify,
    /// Like Verify, but issues diagnostics if non analyzed code can provide arguments that will cause
    /// the analyzed code to go wrong. I.e. it requires all preconditions to be explicit.
    /// This mode should be used for any library whose callers are not known and therefore not analyzed.
    Library,
    // Like Library, but also carries on analysis of functions after a call to an incompletely
    // analyzed function has been encountered.
    Paranoid,
}

impl Options {
    /// Parse options from an argument string. The argument string will be split using unix
    /// shell escaping rules. Any content beyond the leftmost `--` token will be returned
    /// (excluding this token).
    pub fn parse_from_str(
        &mut self,
        s: &str,
        handler: &EarlyDiagCtxt,
        running_test_harness: bool,
    ) -> Vec<String> {
        self.parse(
            &shellwords::split(s).unwrap_or_else(|e| {
                handler.early_fatal(format!("Cannot parse argument string: {e:?}"))
            }),
            handler,
            running_test_harness,
        )
    }

    /// Parses options from a list of strings. Any content beyond the leftmost `--` token
    /// will be returned (excluding this token).
    pub fn parse(
        &mut self,
        args: &[String],
        handler: &EarlyDiagCtxt,
        running_test_harness: bool,
    ) -> Vec<String> {
        let mut hepha_args_end = args.len();
        let mut rustc_args_start = 0;
        if let Some((p, _)) = args.iter().find_position(|s| s.as_str() == "--") {
            hepha_args_end = p;
            rustc_args_start = p + 1;
        }
        let hepha_args = &args[0..hepha_args_end];
        let matches = if rustc_args_start == 0 {
            // The arguments may not be intended for HEPHA and may get here
            // via some tool, so do not report errors here, but just assume
            // that the arguments were not meant for HEPHA.
            match make_options_parser(running_test_harness).try_get_matches_from(hepha_args.iter())
            {
                Ok(matches) => {
                    // Looks like these are HEPHA options after all and there are no rustc options.
                    rustc_args_start = args.len();
                    matches
                }
                Err(e) => match e.kind() {
                    ErrorKind::DisplayHelp => {
                        // help is ambiguous, so display both HEPHA and rustc help.
                        eprintln!("{e}");
                        return args.to_vec();
                    }
                    ErrorKind::UnknownArgument => {
                        // Just send all the arguments to rustc.
                        // Note that this means that HEPHA options and rustc options must always
                        // be separated by --. I.e. any  HEPHA options present in arguments list
                        // will stay unknown to HEPHA and will make rustc unhappy.
                        return args.to_vec();
                    }
                    _ => {
                        eprintln!("{e}");
                        e.exit();
                    }
                },
            }
        } else {
            // This will display error diagnostics for arguments that are not valid for HEPHA.
            make_options_parser(running_test_harness).get_matches_from(hepha_args.iter())
        };

        if matches.contains_id("single_func") {
            self.single_func = matches.get_one::<String>("single_func").cloned();
        }
        if matches.contains_id("diag") {
            self.diag_level = match matches.get_one::<String>("diag").unwrap().as_str() {
                "default" => DiagLevel::Default,
                "verify" => DiagLevel::Verify,
                "library" => DiagLevel::Library,
                "paranoid" => DiagLevel::Paranoid,
                _ => assume_unreachable!(),
            };
        }
        if running_test_harness
            && !matches!(
                matches.value_source("test_only"),
                Some(ValueSource::DefaultValue)
            )
        {
            self.test_only = true;
            if self.diag_level != DiagLevel::Paranoid {
                self.diag_level = DiagLevel::Library;
            }
        }
        if matches.contains_id("constant_time") {
            self.constant_time_tag_name = matches.get_one::<String>("constant_time").cloned();
        }
        if matches.contains_id("body_analysis_timeout") {
            self.max_analysis_time_for_body =
                match matches.get_one::<String>("body_analysis_timeout") {
                    Some(s) => match s.parse::<u64>() {
                        Ok(v) => v,
                        Err(_) => handler.early_fatal("--body_analysis_timeout expects an integer"),
                    },
                    None => assume_unreachable!(),
                }
        }
        if matches.contains_id("crate_analysis_timeout") {
            self.max_analysis_time_for_crate = match matches
                .get_one::<String>("crate_analysis_timeout")
            {
                Some(s) => match s.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => handler.early_fatal("--crate_analysis_timeout expects an integer"),
                },
                None => assume_unreachable!(),
            }
        }
        if !matches!(
            matches.value_source("statistics"),
            Some(ValueSource::DefaultValue)
        ) {
            self.statistics = true;
        }
        if matches.contains_id("call_graph_config") {
            self.call_graph_config = matches.get_one::<String>("call_graph_config").cloned();
        }
        if !matches!(
            matches.value_source("print_function_names"),
            Some(ValueSource::DefaultValue)
        ) {
            self.print_function_names = true;
        }
        if !matches!(
            matches.value_source("print_summaries"),
            Some(ValueSource::DefaultValue)
        ) {
            self.print_summaries = true;
        }
        args[rustc_args_start..].to_vec()
    }
}
