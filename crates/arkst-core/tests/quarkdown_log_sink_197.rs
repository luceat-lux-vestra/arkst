use std::cell::RefCell;

use arkst_core::{
    compile, compile_with_log_sink, CompileOptions, LogEvent, LogLevel, LogSink,
    VirtualProjectBuilder,
};

#[derive(Default)]
struct CollectingSink {
    events: RefCell<Vec<LogEvent>>,
}

impl LogSink for CollectingSink {
    fn emit(&self, event: &LogEvent) {
        self.events.borrow_mut().push(event.clone());
    }
}

fn project(source: &str) -> arkst_core::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project")
}

#[test]
fn explicit_core_log_sink_is_wired_through_full_project_compile() {
    let project = project(".log {first}\n.log {42}\n.debug {hidden}\nvisible\n");
    let sink = CollectingSink::default();

    let result = compile_with_log_sink(&project, &CompileOptions::default(), &sink);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let events = sink.events.borrow();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].level, LogLevel::Log);
    assert_eq!(events[0].message, "first");
    assert_eq!(events[1].level, LogLevel::Log);
    assert_eq!(events[1].message, "42");
    assert_eq!(events[2].level, LogLevel::Debug);
    assert_eq!(events[2].message, "hidden");
}

#[test]
fn ordinary_core_compile_remains_fail_closed_without_log_sink() {
    let project = project(".log {requires-sink}\n");

    let result = compile(&project, &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, "E3010");
}
