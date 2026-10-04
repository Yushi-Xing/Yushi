//! 同一输入下按读法保存模型分数；切换读法不互相作废，输入或前文变化才换代。

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

/// 神经分缓存：一组条件（前文 + 用户按键）下各整句文本的神经分。
///
/// 一次查询里整句转换会跑好几遍（纠错变体、中英混输比分……），同一前文、读法和文本只问模型一次；
/// 异步打分时查询先把「还没分的文本」攒在 `wanted` 里，壳在停顿后一次送去后台，结果回来按文本填进来，
/// 再查一次就都在缓存里了。前文一变整张表作废。
#[derive(Debug)]
pub(crate) struct NeuralCache {
    /// 这些分数对应的前文。
    context: String,

    /// 这些分数对应的用户按键（P2C 的条件；字级模型用不到，但两者一起构成缓存身份）。
    keys: String,

    /// 读法 → 文本 → 神经分；None 表示已预留、结果未到。
    scores: HashMap<String, HashMap<String, Option<f64>>>,

    /// 当前输入作用域。
    scope: String,

    /// 结果所属的查询代，跨会话重置也不能复用。
    epoch: u64,

    /// 已预留的文本分数槽位总数（包含后台进行中的任务）。
    entries: usize,

    /// 按读法分组的待打分文本（不重复）。
    wanted: BTreeMap<String, Vec<String>>,

    /// 模型不经词图、直接按按键生成的整句，连同生成时用的那段按键。
    ///
    /// 这段按键与打分用的 `keys` 不是一回事：打分的条件是某一批路径覆盖的那段（英文尾巴那条只覆盖头段），
    /// 生成的条件永远是整段作用域。所以自带一份键，不跟着 [`Self::ensure_condition`] 作废。
    /// 前文不参与：P2C 只认按键。
    generated: Option<(String, Vec<String>)>,

    /// 还没生成、等着送去后台的那段按键。
    generation_wanted: Option<String>,
}

/// 缓存最多留多少条文本：前文不变时一次组句里的候选也就几十条，超过说明有别的东西在刷。
const MAX_ENTRIES: usize = 1024;

impl NeuralCache {
    /// 同一输入的各读法共享一代缓存；前文或实际输入变化才清空分数。
    pub fn prepare(&mut self, context: &str, scope: &str) {
        if self.context != context || self.scope != scope {
            self.reset_scores(context);
            self.scope = scope.to_owned();
            self.generation_wanted = None;
        }
    }

    fn reset_scores(&mut self, context: &str) {
        self.context = context.to_owned();
        self.epoch = NEXT_EPOCH.fetch_add(1, Ordering::Relaxed);
        self.scores.clear();
        self.wanted.clear();
        self.entries = 0;
    }

    /// 切换当前读法，不能清掉同一查询里其他读法的缓存或待办。
    pub fn ensure_condition(&mut self, context: &str, keys: &str) {
        if self.context != context {
            self.reset_scores(context);
        }
        self.keys = keys.to_owned();
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn get(&self, text: &str) -> Option<f64> {
        self.scores.get(&self.keys)?.get(text).copied().flatten()
    }

    /// 先保留槽位，达到上限就退回静态排序；不能清缓存后马上重复算同一批。
    pub fn reserve(&mut self, text: &str) -> bool {
        if self
            .scores
            .get(&self.keys)
            .is_some_and(|s| s.contains_key(text))
        {
            return true;
        }
        if self.entries >= MAX_ENTRIES {
            return false;
        }
        self.scores
            .entry(self.keys.clone())
            .or_default()
            .insert(text.to_owned(), None);
        self.entries += 1;
        true
    }

    pub fn insert(&mut self, text: &str, score: f64) {
        let keys = self.keys.clone();
        self.insert_for(&keys, text, score);
    }

    /// 后台结果按任务自己的读法填入，不能写进查询最后访问的另一种读法。
    pub fn insert_for(&mut self, keys: &str, text: &str, score: f64) {
        if let Some(slot) = self.scores.get_mut(keys).and_then(|s| s.get_mut(text)) {
            *slot = Some(score);
        }
    }

    pub fn want(&mut self, text: &str) {
        if self.get(text).is_some() || !self.reserve(text) {
            return;
        }
        let wanted = self.wanted.entry(self.keys.clone()).or_default();
        if !wanted.iter().any(|w| w == text) {
            wanted.push(text.to_owned());
        }
    }

    pub fn has_wanted(&self) -> bool {
        !self.wanted.is_empty()
    }

    /// 一次提交全部读法，不能让任务通道的「仅保留最新输入」丢掉兄弟读法。
    pub fn take_wanted(&mut self) -> Vec<(String, Vec<String>)> {
        std::mem::take(&mut self.wanted).into_iter().collect()
    }

    /// 这段按键上生成好的整句；还没生成过返回 `None`。
    pub fn generated(&self, keys: &str) -> Option<&[String]> {
        self.generated
            .as_ref()
            .filter(|(cached, _)| cached == keys)
            .map(|(_, texts)| texts.as_slice())
    }

    pub fn insert_generated(&mut self, keys: &str, texts: Vec<String>) {
        self.generation_wanted = None;
        self.generated = Some((keys.to_owned(), texts));
    }

    /// 记下要给这段按键生成；已经生成过的不重复记。
    pub fn want_generation(&mut self, keys: &str) {
        if self.generated(keys).is_none() {
            self.generation_wanted = Some(keys.to_owned());
        }
    }

    pub fn wanted_generation(&self) -> Option<&str> {
        self.generation_wanted.as_deref()
    }

    /// 取走等着送去后台生成的按键。
    pub fn take_wanted_generation(&mut self) -> Option<String> {
        self.generation_wanted.take()
    }
}

impl Default for NeuralCache {
    fn default() -> Self {
        Self {
            context: String::new(),
            keys: String::new(),
            scope: String::new(),
            epoch: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed),
            entries: 0,
            scores: HashMap::new(),
            wanted: BTreeMap::new(),
            generated: None,
            generation_wanted: None,
        }
    }
}
