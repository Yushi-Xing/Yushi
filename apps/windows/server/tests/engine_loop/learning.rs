//! 带旧学习文件的 Windows 按键与鼠标闭环，避免空学习器掩盖改选不生效的问题。

use std::path::{Path, PathBuf};

use qingjian_platform::UserDirs;
use qingjian_platform::protocol::{ClientMessage, ServerMessage};
use qingjian_render::HitTarget;

use crate::support::{
    AssemblySpec, Router, RouterConfig, SESSION, assembly, letter, open_session, press,
};

fn folder() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder = std::env::temp_dir().join(format!(
        "qingjian-legacy-learning-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("dict.tsv"),
        "没\tmei\t10000\n买\tmai\t9000\n嘛\tma\t8000\n吗\tma\t100\n",
    )
    .unwrap();
    std::fs::write(folder.join("user.tsv"), "没\t100\n").unwrap();
    std::fs::write(folder.join("user-choices.tsv"), "m\t没\t100\n").unwrap();
    folder
}

fn router(folder: &Path) -> Router {
    let user = UserDirs::at(folder.to_path_buf());
    user.prepare();
    let engine = assembly::assemble(&AssemblySpec {
        user: Some(user),
        ..AssemblySpec::new(folder.join("dict.tsv"))
    })
    .unwrap();
    let mut router = Router::new(engine, RouterConfig::default());
    open_session(&mut router, SESSION, Some("notepad.exe".to_owned()));
    router
}

#[test]
fn clicking_ma_overrides_legacy_counts_immediately_and_after_restart() {
    let folder = folder();
    let mut router = router(&folder);
    let (_, _, before) = press(&mut router, letter('m'));
    assert_eq!(before.candidates.items[0].text, "没");
    let index = before
        .candidates
        .items
        .iter()
        .position(|c| c.text == "吗")
        .unwrap();
    router.handle_click(HitTarget::Candidate(index));
    let update = router.handle(ClientMessage::Poll { session: SESSION });
    assert!(
        matches!(update, Some(ServerMessage::Update { commit: Some(ref text), .. }) if text == "吗")
    );
    let (_, _, after) = press(&mut router, letter('m'));
    assert_eq!(after.candidates.items[0].text, "吗");
    assert_eq!(router.engine_mut().learner().choice_weight("m", "没"), 100);
    router.engine_mut().flush_learning();
    drop(router);
    let mut restarted = self::router(&folder);
    let (_, _, after) = press(&mut restarted, letter('m'));
    assert_eq!(after.candidates.items[0].text, "吗");
    drop(restarted);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn number_selection_has_the_same_learning_as_a_mouse_click() {
    let folder = folder();
    let mut router = router(&folder);
    let (_, _, before) = press(&mut router, letter('m'));
    let index = before
        .candidates
        .items
        .iter()
        .position(|c| c.text == "吗")
        .unwrap();
    assert!(index < 9);
    let (_, commit, _) = press(&mut router, letter(char::from(b'1' + index as u8)));
    assert_eq!(commit.as_deref(), Some("吗"));
    let (_, _, after) = press(&mut router, letter('m'));
    assert_eq!(after.candidates.items[0].text, "吗");
    drop(router);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn disabled_or_private_clicks_do_not_replace_previous_preferences() {
    for private in [false, true] {
        let folder = folder();
        let mut router = router(&folder);
        if private {
            router.handle(ClientMessage::Privacy {
                session: SESSION,
                private: true,
            });
        } else {
            router.engine_mut().set_learning(false);
        }
        let (_, _, before) = press(&mut router, letter('m'));
        let index = before
            .candidates
            .items
            .iter()
            .position(|c| c.text == "吗")
            .unwrap();
        router.handle_click(HitTarget::Candidate(index));
        router.handle(ClientMessage::Privacy {
            session: SESSION,
            private: false,
        });
        router.engine_mut().set_learning(true);
        let (_, _, after) = press(&mut router, letter('m'));
        assert_eq!(after.candidates.items[0].text, "没");
        assert_eq!(router.engine_mut().learner().preferred_choice("m"), None);
        drop(router);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
