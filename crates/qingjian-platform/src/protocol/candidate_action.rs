//! 宿主接管候选列表后的显示与选词操作；页码、文本同时校验，避免迟到点击选错词。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateAction {
    Visibility {
        own_window: bool,
    },

    Select {
        typed_keys: String,

        page: usize,
        index: usize,
        text: String,
    },

    Finalize {
        typed_keys: String,

        page: usize,
        index: usize,
        text: String,
    },

    Abort {
        typed_keys: String,
    },

    Raw {
        typed_keys: String,
    },
}
