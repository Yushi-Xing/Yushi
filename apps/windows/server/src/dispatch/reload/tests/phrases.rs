//! 自定义短语配置热加载的增删、禁用与非法规则回归。

use super::{Config, DataDirs, Dictionary, Engine, Router, RouterConfig, modified_at, poll};

#[test]
fn custom_phrases_reload_replace_disable_and_reject_invalid_rules() {
    let dir = std::env::temp_dir().join(format!("qingjian-reload-phrases-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    modified_at(&path, 100);
    let config = Config::load(&path).unwrap();
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    router.watch_config(&config, path.clone(), dir.clone(), DataDirs::default());
    let mut now = 200;
    for (text, position, enabled, expected) in [
        ("第一条", 1, true, Some("第一条")),
        ("替换短语", 1, true, Some("替换短语")),
        ("非法位置", 0, true, Some("替换短语")),
        ("停用短语", 1, false, None),
        ("重新启用", 1, true, Some("重新启用")),
    ] {
        let source = format!(
            "[[custom_phrases]]\ncode = \"ii\"\ntext = \"{text}\"\nposition = {position}\nenabled = {enabled}\n"
        );
        std::fs::write(&path, &source).unwrap();
        modified_at(&path, now);
        now += 100;
        poll(&mut router);
        router.engine.set_input("ii");
        let actual = router.engine.query().ok().and_then(|query| {
            query
                .candidates
                .items
                .into_iter()
                .find(|item| matches!(item.kind, qingjian_core::CandidateKind::Custom(_)))
                .map(|item| item.text)
        });
        assert_eq!(actual.as_deref(), expected, "{source}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    std::fs::write(&path, "").unwrap();
    modified_at(&path, now);
    poll(&mut router);
    router.engine.set_input("ii");
    assert!(router.engine.query().is_err());
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_uses_the_same_atomic_phrase_configuration() {
    use crate::dispatch::attach_custom_phrases;
    use qingjian_core::CustomPhrase;

    let mut engine = Engine::new(Dictionary::default());
    let mut config = Config::default();
    config.custom_phrases.push(CustomPhrase {
        code: "ii".into(),
        text: "第一行\n第二行 🙂".into(),
        position: 1,
        enabled: true,
    });
    attach_custom_phrases(&mut engine, &config);
    engine.set_input("ii");
    let candidate = engine.query().unwrap().candidates.items[0].clone();
    assert_eq!(candidate.text, "第一行\n第二行 🙂");
    engine.commit(&candidate);
    assert!(engine.composition().is_empty());
    config.custom_phrases[0].position = 0;
    attach_custom_phrases(&mut engine, &config);
    engine.set_input("ii");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        candidate.text
    );
}
