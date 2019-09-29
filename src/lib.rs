use scopefunc::ScopeFunc;
use std::iter::FusedIterator;
use typed_igo::conjugation::ConjugationForm;
use typed_igo::{Conjugation, Morpheme, Parser};

pub fn to_polite_sentence(parser: &Parser, orig: &str) -> String {
    use typed_igo::conjugation::ConjugationForm as F;

    parser
        .parse(orig)
        .transform(Splitter::new)
        .break_into_parts()
        .into_iter()
        .map(|part| part.into_polite(F::Basic))
        .collect()
}

struct Part<'t, 'd> {
    morphs: Vec<Morpheme<'t, 'd>>,
    sep: Option<Morpheme<'t, 'd>>,
}

impl<'t, 'd> Part<'t, 'd> {
    fn new(morphs: Vec<Morpheme<'t, 'd>>) -> Part<'t, 'd> {
        Part { morphs, sep: None }
    }

    fn with_sep(morphs: Vec<Morpheme<'t, 'd>>, sep: Morpheme<'t, 'd>) -> Part<'t, 'd> {
        Part {
            morphs,
            sep: Some(sep),
        }
    }

    fn into_polite(self, last_form: ConjugationForm) -> String {
        use typed_igo::conjugation::ConjugationForm as F;
        use typed_igo::wordclass::Postpositional as P;
        use typed_igo::Morpheme as M;
        use typed_igo::WordClass as W;

        let Part { mut morphs, sep } = self;
        let sep_surface = sep.map(|x| x.surface).unwrap_or("");

        // まず終助詞を取り出す。
        let mut ends = Vec::new();
        loop {
            match morphs.pop() {
                Some(M {
                    word_class: W::Postpositional(P::End),
                    surface,
                    ..
                }) => ends.push(surface),

                Some(M {
                    word_class: W::Postpositional(P::SupplementaryParallelEnd),
                    surface,
                    ..
                }) => ends.push(surface),

                Some(other) => {
                    morphs.push(other);
                    break;
                }

                None => {
                    break;
                }
            }
        }
        let ends: String = ends.into_iter().rev().collect();

        // まずは最後の単語を取り出す。もし単語がなければ即 String へ
        let last = match morphs.pop() {
            Some(last) => last,
            None => return ends + sep_surface,
        };

        let fixlast = |orig: &str| match (orig, last_form) {
            ("です", F::Basic) => "です",
            ("です", F::NegativeU) => "でしょ",

            ("ます", F::Basic) => "ます",
            ("ます", F::Negative) => "ませ",
            ("ます", F::NegativeU) => "ましょ",

            other => panic!("unsupported pair: {:?}", other),
        };

        // とりあえず基本的には最後の単語を変換していけばよいが、いくつか例外もある。
        //
        // - 「です」「ます」 : 変換の必要なし
        // - 助動詞の「だ」 : 「です」へ変換
        // - 動詞 : 連用形に変換して「ます」を追加
        // - 「ある」
        //   - 「である」 : 合わせて「です」へ変換
        //   - それ以外 : 「あります」に変換
        // - 「ない」
        //   - 「でない」 : 合わせて「ではありません」に変換
        //   - 動詞の否定 : 動詞を連用形に変換して「ません」に変換
        //   - 形容詞の否定 : (形容詞を連用形に変換して)「ありません」に変換
        //   - それ以外 : 「ありません」に変換
        // - 過去の「た」 : 一つ前で分ける
        //   - 「です」「ます」 : 変換の必要なし
        //   - 動詞 (動いた) : 動詞を連用形に変換し、合わせて「ました」に変換
        //   - 助動詞の「だ」 : 合わせて「でした」に変換
        //   - 「ない」 : 「ない」を戻して sep を無にしてもう一回 into_polite() し「でした」を追加
        //   - それ以外 : 「です」を追加
        // - 「しよう」などの 「う」 : 未然ウ接続終わりの into_polite() して「う」を追加
        // - 否定の「ん」未然終わりの into_polite() して「ん」を追加
        // - それ以外 : 「です」を追加
        let without_sep = match last {
            // 「です」「ます」
            M {
                original_form: "です",
                surface,
                ..
            }
            | M {
                original_form: "ます",
                surface,
                ..
            } => morphs_to_string(&morphs) + surface,

            // 助動詞の「だ」
            M {
                word_class: W::AuxiliaryVerb,
                original_form: "だ",
                ..
            } => morphs_to_string(&morphs) + &fixlast("です"),

            // 動詞
            M {
                word_class: W::Verb(_),
                original_form: basic,
                surface,
                conjugation,
                ..
            } => {
                morphs_to_string(&morphs)
                    + &make_continuous(basic, surface, conjugation)
                    + &fixlast("ます")
            }

            // 「ある」
            M {
                word_class: W::AuxiliaryVerb,
                original_form: "ある",
                ..
            } => match morphs.pop() {
                Some(M {
                    word_class: W::AuxiliaryVerb,
                    original_form: "だ",
                    ..
                }) => morphs_to_string(&morphs) + &fixlast("です"),
                Some(M { surface, .. }) => {
                    morphs_to_string(&morphs) + surface + "あり" + &fixlast("ます")
                }
                None => morphs_to_string(&morphs) + "あり" + &fixlast("ます"),
            },

            // 「ない」
            M {
                word_class: W::AuxiliaryVerb,
                original_form: "ない",
                ..
            }
            | M {
                word_class: W::Adjective(_),
                original_form: "ない",
                ..
            } => match morphs.pop() {
                Some(M {
                    word_class: W::AuxiliaryVerb,
                    original_form: "で",
                    ..
                }) => morphs_to_string(&morphs) + "ではありません",
                Some(M {
                    word_class: W::Verb(_),
                    original_form: basic,
                    surface,
                    conjugation,
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &make_continuous(basic, surface, conjugation)
                        + "ません"
                }
                Some(M {
                    word_class: W::Adjective(_),
                    surface,
                    ..
                }) => morphs_to_string(&morphs) + surface + "ありません",
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface + "ありません",
                None => morphs_to_string(&morphs) + "ありません",
            },

            // 過去の「た」
            M {
                word_class: W::AuxiliaryVerb,
                original_form: "た",
                ..
            } => match morphs.pop() {
                Some(M {
                    original_form: "です",
                    surface,
                    ..
                })
                | Some(M {
                    original_form: "ます",
                    surface,
                    ..
                }) => morphs_to_string(&morphs) + surface + "た",
                Some(M {
                    word_class: W::Verb(_),
                    original_form: basic,
                    surface,
                    conjugation,
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &make_continuous(basic, surface, conjugation)
                        + "ました"
                }
                Some(M {
                    word_class: W::AuxiliaryVerb,
                    original_form: "だ",
                    ..
                }) => morphs_to_string(&morphs) + "でした",
                Some(
                    morph @ M {
                        original_form: "ない",
                        ..
                    },
                ) => {
                    morphs
                        .modify(|ms| ms.push(morph))
                        .transform(Part::new)
                        .into_polite(F::Basic)
                        + "でした"
                }
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface + "たです",
                None => morphs_to_string(&morphs) + "たです",
            },

            // 「しよう」などの 「う」
            M {
                original_form: "う",
                ..
            } => Part::new(morphs).into_polite(F::NegativeU) + "う",

            // 否定の「ん」
            M {
                original_form: "ん",
                ..
            } => Part::new(morphs).into_polite(F::Negative) + "ん",

            // それ以外
            M { surface, .. } => morphs_to_string(&morphs) + surface + "です",
        };

        without_sep + &ends + sep_surface
    }
}

fn morphs_to_string<'t, 'd>(morphs: &[Morpheme<'t, 'd>]) -> String {
    morphs.iter().map(|m| m.surface).collect()
}

fn make_continuous(basic: &str, surface: &str, conjugation: Conjugation) -> String {
    use conjugation::convert;
    use typed_igo::conjugation::{ConjugationForm as F, ConjugationKind as K};
    let Conjugation { kind, form } = conjugation;

    match kind {
        K::SahenSuruConnected => convert(surface, kind, form, F::Negative)
            .expect("failed to convert (sahen-suru connected)"),

        K::SahenZuruConnected => format!("{}じ", &basic[0..basic.len() - "ずる".len()]),

        // FIXME: なにをすればいいんだ？何が 一段・ル なんだ？
        K::IchidanRu => basic.to_string(),

        K::SpecialNai | K::SpecialTai => convert(surface, kind, form, F::ContinuousDe)
            .expect("failed to convert (special nai/tai)"),

        _ => convert(surface, kind, form, F::Continuous).expect("failed to convert"),
    }
}

struct Splitter<'t, 'd, I> {
    rest: I,
    curr: Option<Morpheme<'t, 'd>>,
    next: Option<Morpheme<'t, 'd>>,
    parts: Vec<Part<'t, 'd>>,
    part: Vec<Morpheme<'t, 'd>>,
    paren_level: u32,
}

impl<'t, 'd, I> Splitter<'t, 'd, I>
where
    I: Iterator<Item = Morpheme<'t, 'd>>,
    I: FusedIterator,
{
    fn new<IntoIter>(orig: IntoIter) -> Splitter<'t, 'd, I>
    where
        IntoIter: IntoIterator<Item = Morpheme<'t, 'd>, IntoIter = I>,
    {
        let mut iter = orig.into_iter();
        let first = iter.next();
        let second = iter.next();

        Splitter {
            rest: iter,
            curr: first,
            next: second,
            parts: Vec::new(),
            part: Vec::new(),
            paren_level: 0,
        }
    }

    fn is_finished(&self) -> bool {
        self.curr.is_none()
    }

    fn step_once(&mut self) -> Option<Morpheme<'t, 'd>> {
        use std::mem::replace;
        replace(&mut self.curr, replace(&mut self.next, self.rest.next()))
    }

    fn break_part(&mut self) {
        use std::mem::replace;
        let part = replace(&mut self.part, Vec::new());
        let sep = self.step_once().expect("unexpected end");
        self.parts.push(Part::with_sep(part, sep));
    }

    fn unwrap_curr(&self) -> &Morpheme<'t, 'd> {
        self.curr.as_ref().expect("unwrap_curr() called on None")
    }

    fn push_curr(&mut self) {
        let curr = self.step_once().expect("unexpected end of iterator");
        self.part.push(curr);
    }

    fn push_last(&mut self) {
        if !self.part.is_empty() {
            self.curr = Some(create_period("。"));
            self.break_part();
        }
    }

    fn break_into_parts(&mut self) -> Vec<Part<'t, 'd>> {
        while !self.is_finished() {
            self.handle_paren_count();
            if self.should_be_break() {
                self.break_part();
            } else {
                self.push_curr();
            }
        }

        self.push_last();

        std::mem::replace(&mut self.parts, Vec::new())
    }

    fn handle_paren_count(&mut self) {
        use typed_igo::wordclass::Symbol as S;
        use typed_igo::WordClass as W;
        match self.unwrap_curr().word_class {
            W::Symbol(S::OpenParen) => self.paren_level += 1,
            W::Symbol(S::CloseParen) => self.paren_level -= 1,
            _ => {}
        }
    }

    fn should_be_break(&self) -> bool {
        use typed_igo::wordclass::{Postpositional as P, Symbol as S};
        use typed_igo::WordClass as W;
        // 括弧深度が 1 以上の場合は引用または発言とみなし、何も変換しない。つまり区切る必要もない。
        if self.paren_level >= 1 {
            return false;
        }

        match self.unwrap_curr().word_class {
            // 基本は句点での分割
            W::Symbol(S::Period) => true,

            // だいたい他はそのままでよさそうだったが、接続助詞の「が」の前だけはなんか丁寧語にしな
            // いと違和感があるのでそこでも分割。
            //
            // - (OK) 確認したところ問題ありませんでした。
            // - (OK) 言ったからには実行します。
            // - (NG) 今日は良い天気だが明日は雨のようです。 (「今日は良い天気でしたが」にしたい)
            W::Postpositional(P::Conjunction) => self.unwrap_curr().original_form == "が",

            // それ以外は切らない
            _ => false,
        }
    }
}

fn create_period(basic: &'static str) -> Morpheme<'static, 'static> {
    use typed_igo::conjugation::{ConjugationForm as F, ConjugationKind as K};
    use typed_igo::wordclass::Symbol as S;
    use typed_igo::WordClass as W;
    Morpheme {
        surface: basic,
        word_class: W::Symbol(S::Period),
        conjugation: Conjugation {
            kind: K::None,
            form: F::None,
        },
        original_form: basic,
        reading: basic,
        pronunciation: basic,
        start: !0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static::lazy_static! {
        static ref PARSER: Parser = Parser::new();
    }

    macro_rules! check {
        ($($testname:ident >> $from:literal => $to:literal)*) => {
            $(
                #[test]
                fn $testname() {
                    assert_eq!(to_polite_sentence(&*PARSER, $from), $to);
                }
            )*
        };
    }

    check! {
        simple >>
        "今日は晴天だ。"
        => "今日は晴天です。"

        longer >>
        "前進をしない人は、後退をしているのだ。"
        => "前進をしない人は、後退をしているのです。"

        multiple_sentences >>
        "どんなに悔いても過去は変わらない。どれほど心配したところで未来もどうなるものでもない。いま、現在に最善を尽くすことである。"
        => "どんなに悔いても過去は変わりません。どれほど心配したところで未来もどうなるものでもありません。いま、現在に最善を尽くすことです。"

        quote1 >>
        "最も重要な決定とは、何をするかではなく、何をしないかを決めることだ。"
        => "最も重要な決定とは、何をするかではなく、何をしないかを決めることです。"

        quote2 >>
        "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
        => "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"

        quote3 >>
        "善人はこの世で多くの害をなす。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことだ。"
        =>"善人はこの世で多くの害をなします。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことです。"

        adjective_past1 >>
        "今日は寒かった。"
        => "今日は寒かったです。"

        end1 >>
        "今日はいい天気か。"
        => "今日はいい天気ですか。"

        lets1 >>
        "今日は勉強をしよう。"
        => "今日は勉強をしましょう。"

        nn >>
        "許さん。"
        => "許しません。"

        already1 >>
        "京都大学は1897年の創立以来、「自重自敬」の精神に基づき自由な学風を育み、創造的な学問の世界を切り開いてきました。また、地球社会の調和ある共存に貢献することも京都大学の重要な目標です。"
        => "京都大学は1897年の創立以来、「自重自敬」の精神に基づき自由な学風を育み、創造的な学問の世界を切り開いてきました。また、地球社会の調和ある共存に貢献することも京都大学の重要な目標です。"

        already2 >>
        "一方で今、世界は20世紀には想像もしなかったような急激な変化を体験しつつあります。東西冷戦の終結によって解消するはずだった世界の対立構造は、民族間、宗教間の対立によってますます複雑かつ過酷になっています。他方、地球環境の悪化は加速し、想定外の大規模な災害や致死性の感染症が各地で猛威をふるい、金融危機は国の経済や人々の生活を根本から揺さぶっています。その荒波の中で、大学はどうあるべきかを真摯に考えて行かなければなりません。そして、国は産官学連携を推進してグローバルに活躍できる人材育成を奨励し、国際的に競争力のある大学改革を要請しています。京都大学が建学の精神に立ちつつ、どのようにこの国や社会の要請にこたえていけるかが今問われています。"
        => "一方で今、世界は20世紀には想像もしなかったような急激な変化を体験しつつあります。東西冷戦の終結によって解消するはずだった世界の対立構造は、民族間、宗教間の対立によってますます複雑かつ過酷になっています。他方、地球環境の悪化は加速し、想定外の大規模な災害や致死性の感染症が各地で猛威をふるい、金融危機は国の経済や人々の生活を根本から揺さぶっています。その荒波の中で、大学はどうあるべきかを真摯に考えて行かなければなりません。そして、国は産官学連携を推進してグローバルに活躍できる人材育成を奨励し、国際的に競争力のある大学改革を要請しています。京都大学が建学の精神に立ちつつ、どのようにこの国や社会の要請にこたえていけるかが今問われています。"
    }
}
