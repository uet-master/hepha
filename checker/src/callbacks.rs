// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
#![allow(clippy::borrowed_box)]

use crate::call_graph::CallGraph;
use crate::constant_domain::ConstantValueCache;
use crate::crate_visitor::CrateVisitor;
use crate::known_names::KnownNamesCache;
use crate::options::Options;
use crate::summaries::SummaryCache;

use crate::type_visitor::TypeCache;
use crate::utils;
use log::info;
use log_derive::*;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::path::PathBuf;
use std::rc::Rc;
use tempfile::TempDir;

/// Private state used to implement the callbacks.
pub struct MiraiCallbacks {
    /// Options provided to the analysis.
    options: Options,
    /// The relative path of the file being compiled.
    file_name: String,
    /// A path to the directory where analysis output, such as the summary cache, should be stored.
    output_directory: PathBuf,
    /// True if this run is done via cargo test
    test_run: bool,
}

/// Constructors
impl MiraiCallbacks {
    pub fn new(options: Options) -> MiraiCallbacks {
        MiraiCallbacks {
            options,
            file_name: String::new(),
            output_directory: PathBuf::default(),
            test_run: false,
        }
    }

    pub fn test_runner(options: Options) -> MiraiCallbacks {
        MiraiCallbacks {
            options,
            file_name: String::new(),
            output_directory: PathBuf::default(),
            test_run: true,
        }
    }
}

impl Debug for MiraiCallbacks {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        "MiraiCallbacks".fmt(f)
    }
}

impl Default for MiraiCallbacks {
    fn default() -> Self {
        Self::new(Options::default())
    }
}

impl rustc_driver::Callbacks for MiraiCallbacks {
    /// Called before creating the compiler instance
    #[logfn(TRACE)]
    fn config(&mut self, config: &mut interface::Config) {
        self.file_name = config
            .input
            .source_name()
            .prefer_remapped_unconditionaly()
            .to_string();
        info!("Processing input file: {}", self.file_name);
        if config.opts.test {
            info!("in test only mode");
            self.options.test_only = true;
        }
        config.crate_cfg.push("hepha".to_string());
        match &config.output_dir {
            None => {
                self.output_directory = std::env::temp_dir();
                self.output_directory.pop();
            }
            Some(path_buf) => self.output_directory.push(path_buf.as_path()),
        };
    }

    /// Called after the compiler has completed all analysis passes and before it lowers MIR to LLVM IR.
    /// At this point the compiler is ready to tell us all it knows and we can proceed to do abstract
    /// interpretation of all functions that will end up in the compiler output.
    /// If this method returns false, the compilation will stop.
    #[logfn(TRACE)]
    fn after_analysis<'tcx>(
        &mut self,
        compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        compiler.sess.dcx().abort_if_errors();
        if self
            .output_directory
            .to_str()
            .expect("valid string")
            .contains("/build/")
        {
            // No need to analyze a build script, but do generate code.
            return Compilation::Continue;
        }
        self.analyze_with_hepha(compiler, tcx);
        if self.test_run {
            // We avoid code gen for test cases because LLVM is not used in a thread safe manner.
            Compilation::Stop
        } else {
            // Although HEPHA is only a checker, cargo still needs code generation to work.
            Compilation::Continue
        }
    }
}

impl MiraiCallbacks {
    /// Analyze the crate currently being compiled, using the information given in compiler and tcx.
    #[logfn(TRACE)]
    fn analyze_with_hepha<'tcx>(&mut self, compiler: &interface::Compiler, tcx: TyCtxt<'tcx>) {
        if self.options.print_function_names {
            for local_def_id in tcx.hir().body_owners() {
                let def_id = local_def_id.to_def_id();
                let span = tcx.def_span(def_id);
                eprint!("{span:?}: ");
                let name = utils::def_id_as_qualified_name_str(tcx, def_id);
                eprintln!("{name}");
            }
            return;
        }
        let output_dir = String::from(self.output_directory.to_str().expect("valid string"));
        let summary_store_path = if std::env::var("HEPHA_SHARE_PERSISTENT_STORE").is_ok() {
            output_dir
        } else {
            let temp_dir = TempDir::new().expect("failed to create a temp dir");
            String::from(temp_dir.into_path().to_str().expect("valid string"))
        };
        info!(
            "storing summaries for {} at {}/.summary_store.sled",
            self.file_name, summary_store_path
        );
        let call_graph_config = self.options.call_graph_config.to_owned();
        let mut crate_visitor = CrateVisitor {
            buffered_diagnostics: Vec::new(),
            constant_time_tag_cache: None,
            constant_time_tag_not_found: false,
            constant_value_cache: ConstantValueCache::default(),
            diagnostics_for: HashMap::new(),
            file_name: self.file_name.as_str(),
            known_names_cache: KnownNamesCache::create_cache_from_language_items(),
            options: &std::mem::take(&mut self.options),
            session: &compiler.sess,
            generic_args_cache: HashMap::new(),
            summary_cache: SummaryCache::new(summary_store_path),
            tcx,
            test_run: self.test_run,
            type_cache: Rc::new(RefCell::new(TypeCache::new())),
            call_graph: CallGraph::new(call_graph_config, tcx),
        };
        if crate_visitor.options.print_summaries {
            crate_visitor.call_graph.config.include_calls_in_summaries = true;
        }
        crate_visitor.analyze_some_bodies();
        crate_visitor.call_graph.output();
        crate_visitor.print_summaries();
    }
}
