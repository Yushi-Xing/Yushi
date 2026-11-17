//! 冻结的打字任务；目标只用于模拟选词，不参与词库或模型装配。

use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(super) struct Scenario {
    pub id: String,

    pub scene: String,

    pub text: String,

    pub keys: String,

    /// 实际键流；退格用 \u{0008}，为空时从 keys / erase 生成。
    #[serde(default)]
    pub keystrokes: Option<String>,

    /// 简拼找不到目标时，取消未上屏组句再用全拼输入；这次补救计入操作数。
    #[serde(default)]
    pub retry_keys: Option<String>,

    /// 相邻同名任务共用引擎，模拟一段聊天里的连续上屏与模式切换。
    #[serde(default)]
    pub session: Option<String>,

    /// 虚拟按键到达间隔，用于从实测处理耗时估算排队，不进行人为 sleep。
    #[serde(default = "default_interval")]
    pub interval_ms: u32,

    #[serde(default)]
    pub erase: usize,

    #[serde(default)]
    pub english: bool,

    #[serde(default)]
    pub punctuation: Option<char>,
}

fn default_interval() -> u32 {
    80
}
