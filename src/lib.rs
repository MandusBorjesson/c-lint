pub mod config;
pub mod diag;
pub mod extract;
pub mod ignore;
pub mod lexer;
pub mod lint;
pub mod style;

pub use config::{find_config_path, Config, Target};
pub use diag::{Diagnostic, Severity};
pub use lint::{collect_c_files, lint_file, lint_source};
