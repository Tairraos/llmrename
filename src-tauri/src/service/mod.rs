//! 业务编排层：模板渲染、视觉模型调用、防冲突重命名。
//! 只依赖 Types / Config / Repo，不引用 Runtime。

pub mod history;
pub mod models;
pub mod namer;
pub mod prompts;
pub mod renderer;
pub mod vision;
