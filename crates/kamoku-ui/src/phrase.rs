// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Japanese line-break opportunities, independent of the browser DOM.

fn hiragana(c: char) -> bool {
    ('ぁ'..='ゟ').contains(&c)
}
fn kanji(c: char) -> bool {
    ('一'..='鿿').contains(&c) || c == '々'
}
fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.-".contains(c)
}

pub(crate) fn chunks(text: &str) -> Vec<String> {
    let chars: Vec<_> = text.chars().collect();
    let mut output = Vec::new();
    let mut current: Vec<char> = Vec::new();
    for (index, &c) in chars.iter().enumerate() {
        let previous = current.last().copied().unwrap_or('\0');
        let mut cut = false;
        let mut carry = 0;
        if !current.is_empty() {
            let joins = "、。，．！？!?：:）」』】］〉》)]".contains(c);
            if " 　".contains(c) {
                // Keep the space with its preceding word.
            } else if " 　".contains(previous) {
                cut = true;
            } else if "（「『【［〈《([".contains(c) {
                cut = !chars.get(index + 1).copied().is_some_and(hiragana) || current.len() > 4;
            } else if "、。，．！？!?：:".contains(previous) {
                cut = !joins;
            } else if "）」』】］〉》)]".contains(previous) {
                cut = !joins && !hiragana(c);
            } else if hiragana(previous) {
                let sticky = ["この", "その", "あの", "どの"];
                let part: String = current.iter().collect();
                let okurigana = current.len() >= 2
                    && kanji(current[current.len() - 2])
                    && kanji(c)
                    && !"のをにはがとでもへやか".contains(previous);
                cut = "をにがはのでとへやもからてたなるばだ".contains(previous)
                    && !joins
                    && !hiragana(c)
                    && c != 'ー'
                    && !sticky.contains(&part.as_str())
                    && !okurigana;
                if cut && sticky.iter().any(|s| part.ends_with(s) && part != *s) {
                    carry = 2;
                }
            }
        }
        if !cut && current.len() >= 14 && word(previous) && word(c) {
            let count = current.iter().rev().take_while(|&&c| word(c)).count();
            if count < current.len() {
                carry = count;
            }
        }
        if cut || current.len() >= 14 {
            let tail = current.split_off(current.len() - carry);
            output.push(current.iter().collect());
            current = tail;
        }
        current.push(c);
    }
    if !current.is_empty() {
        output.push(current.iter().collect());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::chunks;

    #[test]
    fn particles_quotes_sticky_words_and_okurigana() {
        for (text, expected) in [
            ("帳簿の保存", vec!["帳簿の", "保存"]),
            ("「現金」口座", vec!["「現金」", "口座"]),
            ("「現金」の口座", vec!["「現金」の", "口座"]),
            (
                "事業主貸は、その年の動きだけです。",
                vec!["事業主貸は、", "その年の", "動きだけです。"],
            ),
            (
                "超えた分にはこの特例を使えない",
                vec!["超えた", "分には", "この特例を", "使えない"],
            ),
            ("UTF-8 と Shift_JIS", vec!["UTF-8 ", "と ", "Shift_JIS"]),
            (
                "abcdefghijklmnopqrstuvwxyz0123",
                vec!["abcdefghijklmn", "opqrstuvwxyz01", "23"],
            ),
        ] {
            assert_eq!(chunks(text), expected, "{text}");
        }
        assert_eq!(chunks(""), [] as [std::string::String; 0]);
    }

    #[test]
    fn screen_notices_keep_their_words_and_round_trip() {
        for (text, whole) in [
            (
                "確定すると、仕訳帳と帳票の集計に入ります。確定した後は、訂正と取消の手続きでしか変えられません。",
                vec!["確定すると、", "変えられません。"],
            ),
            (
                "決算や申告が済んだ期間を締めると、その期間の登録・訂正・取消を断ります。空にすると締めません。",
                vec!["締めません。", "断ります。", "締めると、"],
            ),
            (
                "ふつうは開業日（または、記帳を始める年の1月1日）です。最初の会計年度の中の日付にしてください。",
                vec!["1月1日）です。", "日付にしてください。", "開業日（または、"],
            ),
            (
                "金融機関が付けた番号の列があれば選んでください。重複を見分ける、いちばん確かな手がかりです。",
                vec!["選んでください。", "手がかりです。"],
            ),
            (
                "開業日の時点で、それぞれの口座にいくらあったかを入れます。カードは、まだ払っていない額です。",
                vec!["口座にいくらあったかを", "払っていない額です。"],
            ),
            (
                "型は「現金」「預金」「クレジットカード」「電子マネー」から選び、名前は自由に付けられます。",
                vec!["「電子マネー」から", "「クレジットカード」"],
            ),
            (
                "2026年1月1日から9月30日までの集計です。参考値で、簡易課税と特例は反映していません。",
                vec![
                    "2026年1月1日から",
                    "9月30日までの",
                    "簡易課税と",
                    "反映していません。",
                ],
            ),
            ("負債・カードの未払い額", vec!["負債・カードの", "未払い額"]),
            ("サンプルカードの引き落とし", vec!["引き落とし"]),
        ] {
            let parts = chunks(text);
            assert_eq!(parts.concat(), text);
            assert!(parts.iter().all(|s| s.chars().count() <= 14));
            for word in whole {
                assert!(parts.iter().any(|s| s == word), "{word}: {parts:?}");
            }
            assert!(
                parts
                    .iter()
                    .skip(1)
                    .all(|s| !s.starts_with(['、', '。', '）', '」', '』', 'ー']))
            );
        }
    }
}
