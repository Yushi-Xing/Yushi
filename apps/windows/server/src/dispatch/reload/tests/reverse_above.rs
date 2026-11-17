//! 反向排列开关热加载时，画法、当前窗口和方向键必须同步更新。

use std::sync::{Arc, Mutex};

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::Config;
use qingjian_platform::protocol::{Frame, KeyEvent, ScreenRect};
use qingjian_render::Mode;

use crate::dispatch::{
    CandidateSink, DataDirs, KeyHintView, PanelFrame, RenderSettings, Router, RouterConfig,
};

#[derive(Clone, Default)]
struct RecordingSink {
    settings: Arc<Mutex<Vec<RenderSettings>>>,

    frames: Arc<Mutex<Vec<Frame>>>,
}

impl CandidateSink for RecordingSink {
    fn show(&self, frame: Frame, _mode: Mode, _rect: ScreenRect, _hint: Option<KeyHintView>) {
        self.frames.lock().unwrap().push(frame);
    }

    fn hide(&self) {}

    fn show_panel(&self, _frame: PanelFrame, _rect: Option<ScreenRect>) {}

    fn hide_panel(&self) {}

    fn configure(&self, settings: RenderSettings) {
        self.settings.lock().unwrap().push(settings);
    }
}

#[test]
fn changing_reversal_redraws_unchanged_candidates_and_updates_arrow_navigation() {
    let dir = std::env::temp_dir().join(format!("qingjian-reverse-reload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "[general]\nlayout = \"vertical\"\n[update]\ncheck = false\n",
    )
    .unwrap();
    super::modified_at(&path, 100);
    let config = Config::load(&path).unwrap();
    let mut router = Router::new(
        Engine::new(Dictionary::parse("你\tni\t100\n尼\tni\t90\n拟\tni\t80\n").unwrap()),
        RouterConfig::from(&config),
    );
    let sink = RecordingSink::default();
    router.set_candidate_sink(Box::new(sink.clone()));
    router.engine.set_input("ni");
    router.recompose();
    router.candidate_upward = true;
    router.last_rect = Some(ScreenRect {
        left: 0,
        top: 900,
        right: 10,
        bottom: 920,
    });
    let frame = router.self_drawn_frame();
    router.reconcile_candidates(&frame);
    assert!(
        sink.settings
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .vertical_reverse_above
    );
    assert_eq!(sink.frames.lock().unwrap().len(), 1);
    router.watch_config(&config, path.clone(), dir.clone(), DataDirs::default());

    for (enabled, seconds, key) in [(false, 200, 0x28), (true, 300, 0x26)] {
        Config::set_bool(&path, "general", "vertical_reverse_above", enabled).unwrap();
        super::modified_at(&path, seconds);
        let previous_frames = sink.frames.lock().unwrap().len();
        super::poll(&mut router);
        assert_eq!(router.config.vertical_reverse_above, enabled);
        assert_eq!(
            sink.settings
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .vertical_reverse_above,
            enabled
        );
        let frames = sink.frames.lock().unwrap();
        assert_eq!(
            frames.len(),
            previous_frames + 1,
            "setting must redraw without typing another key"
        );
        assert_eq!(frames.last().unwrap().typed_keys, "ni");
        drop(frames);
        router.highlight = 0;
        router.apply_key(&KeyEvent::new(key, None, Default::default()));
        assert_eq!(router.highlight, 1);
    }
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}
