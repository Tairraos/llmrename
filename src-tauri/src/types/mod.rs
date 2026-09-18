//! 纯数据层：所有跨层传输的数据模型。
//! 只依赖 serde，不引用任何上层。

pub mod asset;
pub mod config;
pub mod error;
pub mod history;
pub mod log;
pub mod rename;

pub use asset::*;
pub use config::*;
pub use error::*;
pub use history::*;
pub use log::*;
pub use rename::*;
