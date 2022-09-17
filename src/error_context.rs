//! 错误模板的上下文。

use serde::Serialize;

/// 错误模板的上下文。
#[derive(Serialize)]
pub struct ErrorContext {
    /// 页面的标题。
    pub title: String,
    /// 错误消息。
    pub message: String,
}
