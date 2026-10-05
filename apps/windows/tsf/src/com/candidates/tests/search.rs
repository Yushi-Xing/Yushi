//! 搜索候选接口的快照、索引、枚举与过期查询边界。

use super::super::Candidates;
use super::frame;
use qingjian_platform::protocol::Frame;
use windows::Win32::Foundation::S_FALSE;
use windows::Win32::UI::TextServices::{ITfCandidateString, ITfFnSearchCandidateProvider};
use windows::core::{BSTR, Interface};

#[test]
fn search_snapshots_survive_later_edits_and_enumerators_clone_the_cursor() {
    let candidates = Candidates::new();
    let mut input = frame();
    input.typed_keys = "huantaipingyang".into();
    input.candidates.items[0].syllables =
        ["huan", "tai", "ping", "yang"].map(str::to_owned).to_vec();
    candidates.update(&input, None).unwrap();
    let provider: ITfFnSearchCandidateProvider = candidates.search_provider().cast().unwrap();
    unsafe {
        let list = provider
            .GetSearchCandidates(&BSTR::from("huantaipingyang"), &BSTR::new())
            .unwrap();
        assert_eq!(list.GetCandidateNum().unwrap(), 1);
        assert!(list.GetCandidate(u32::MAX).is_err());
        assert_eq!(
            list.GetCandidate(0)
                .unwrap()
                .GetString()
                .unwrap()
                .to_string(),
            "环太平洋"
        );
        candidates.update(&Frame::default(), None).unwrap();
        assert_eq!(
            provider
                .GetSearchCandidates(&BSTR::from("huantaipingyang"), &BSTR::new())
                .unwrap()
                .GetCandidateNum()
                .unwrap(),
            0
        );
        let cursor = list.EnumCandidates().unwrap();
        let mut items: [Option<ITfCandidateString>; 2] = [None, None];
        let mut fetched = 0;
        let status = (Interface::vtable(&cursor).Next)(
            Interface::as_raw(&cursor),
            2,
            items.as_mut_ptr().cast(),
            &mut fetched,
        );
        assert_eq!(status, S_FALSE);
        assert_eq!(fetched, 1);
        assert_eq!(items[0].as_ref().unwrap().GetIndex().unwrap(), 0);
        let clone = cursor.Clone().unwrap();
        items.fill(None);
        clone.Next(&mut items[..1], &mut fetched).unwrap();
        assert_eq!(fetched, 0);
        cursor.Reset().unwrap();
        cursor.Next(&mut items[..1], &mut fetched).unwrap();
        assert_eq!(fetched, 1);
        assert_eq!(list.GetCandidateNum().unwrap(), 1);
    }
    assert!(candidates.take_actions().is_empty());
}
