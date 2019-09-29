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

pub fn to_impolite_sentence(parser: &Parser, orig: &str) -> String {
    use typed_igo::conjugation::ConjugationForm as F;

    parser
        .parse(orig)
        .transform(Splitter::new)
        .break_into_parts()
        .into_iter()
        .map(|part| part.into_impolite(&[F::Basic]))
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
        use typed_igo::Morpheme as M;
        use typed_igo::WordClass as W;

        let Part { mut morphs, sep } = self;
        let sep_surface = sep.map(|x| x.surface).unwrap_or("");

        // まず終助詞を取り出す。
        let ends = take_ends(&mut morphs);

        // 次に最後の単語を取り出す。もし単語がなければ即 String へ
        let last = match morphs.pop() {
            Some(last) => last,
            None => return ends + sep_surface,
        };

        // 文末を処理するもの
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
                basic: "です",
                surface,
                ..
            }
            | M {
                basic: "ます",
                surface,
                ..
            } => morphs_to_string(&morphs) + surface,

            // 助動詞の「だ」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "だ",
                ..
            } => morphs_to_string(&morphs) + fixlast("です"),

            // 動詞
            M {
                wordclass: W::Verb(_),
                basic,
                surface,
                conjugation,
                ..
            } => {
                morphs_to_string(&morphs)
                    + &make_continuous(basic, surface, conjugation)
                    + fixlast("ます")
            }

            // 「ある」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "ある",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "だ",
                    ..
                }) => morphs_to_string(&morphs) + fixlast("です"),
                Some(M { surface, .. }) => {
                    morphs_to_string(&morphs) + surface + "あり" + fixlast("ます")
                }
                None => "あり".to_string() + fixlast("ます"),
            },

            // 「ない」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "ない",
                ..
            }
            | M {
                wordclass: W::Adjective(_),
                basic: "ない",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "で",
                    ..
                }) => morphs_to_string(&morphs) + "ではありません",
                Some(M {
                    wordclass: W::Verb(_),
                    basic,
                    surface,
                    conjugation,
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &make_continuous(basic, surface, conjugation)
                        + "ません"
                }
                Some(M {
                    wordclass: W::Adjective(_),
                    surface,
                    ..
                }) => morphs_to_string(&morphs) + surface + "ありません",
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface + "ありません",
                None => "ありません".into(),
            },

            // 過去の「た」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "た",
                ..
            } => match morphs.pop() {
                Some(M {
                    basic: "です",
                    surface,
                    ..
                })
                | Some(M {
                    basic: "ます",
                    surface,
                    ..
                }) => morphs_to_string(&morphs) + surface + "た",
                Some(M {
                    wordclass: W::Verb(_),
                    basic,
                    surface,
                    conjugation,
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &make_continuous(basic, surface, conjugation)
                        + "ました"
                }
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "だ",
                    ..
                }) => morphs_to_string(&morphs) + "でした",
                // である -> でした
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "ある",
                    ..
                }) => morphs_to_string(&morphs) + "した",
                Some(
                    morph @ M {
                        basic: "ない", ..
                    },
                ) => {
                    morphs
                        .modify(|ms| ms.push(morph))
                        .transform(Part::new)
                        .into_polite(F::Basic)
                        + "でした"
                }
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface + "たです",
                None => "たです".to_string(),
            },

            // 「しよう」などの 「う」
            M { basic: "う", .. } => Part::new(morphs).into_polite(F::NegativeU) + "う",

            // 否定の「ん」
            M { basic: "ん", .. } => Part::new(morphs).into_polite(F::Negative) + "ん",

            // それ以外
            M { surface, .. } => morphs_to_string(&morphs) + surface + "です",
        };

        without_sep + &ends + sep_surface
    }

    fn into_impolite(self, last_forms: &[ConjugationForm]) -> String {
        use typed_igo::conjugation::{ConjugationForm as F, ConjugationKind as K};
        use typed_igo::Morpheme as M;
        use typed_igo::WordClass as W;

        let Part { mut morphs, sep } = self;
        let sep_surface = sep.map(|x| x.surface).unwrap_or("");

        // まず終助詞を取り出す。
        let ends = take_ends(&mut morphs);

        // 最後の単語を取り出す。単語がなければ即 String にして終わり。
        let last = match morphs.pop() {
            Some(last) => last,
            None => return ends + sep_surface,
        };

        // 活用を処理するもの
        let fix = |orig: &str, kind: K, from: F, to: &[F]| {
            to.iter()
                .find_map(|&to| conjugation::convert(orig, kind, from, to).ok())
                .unwrap_or_else(|| orig.to_string())
        };

        let fixlast = |orig: &str| match orig {
            "だ" => fix("だ", K::SpecialDa, F::Basic, last_forms),
            "た" => fix("た", K::SpecialTa, F::Basic, last_forms),
            "ある" => fix("ある", K::GodanRaAru, F::Basic, last_forms),
            "です" => fix("です", K::SpecialDesu, F::Basic, last_forms),
            "ます" => fix("ます", K::SpecialMasu, F::Basic, last_forms),
            "ない" => fix("ない", K::SpecialNai, F::Basic, last_forms),
            _ => panic!("unsupported conversion"),
        };

        // とりあえず基本的には最後の単語を変換していけばよいが、いくつか例外もある。
        //
        // - 助動詞の「だ」 : 変換の必要なし
        // - 助動詞の「ある」 : 変換の必要なし
        // - 「です」 : 一つ前で場合分け
        //   - 形容詞 : 単に消す
        //   - 過去「た」 : 単に消す
        //   - それ以外 : 終助詞がなければ「だ」に変換
        // - 「ます」 : 一つ前で場合分け
        //   - 動詞 : 消して終止形にする
        //   - それ以外 : FIXME: 単に消す
        // - Let's の「う」 : 一つ前で場合分け
        //   - 「です」 : まとめて「だろう」に変換
        //   - 「ます」 : 一つ前を連用ウ接続にして「う」に変換
        // - 否定の「ん」 : 一つ前で場合分け
        //   - 「ます」 : 一つ前で場合分け
        //     - 「ある」 : まとめて「ない」に変換
        //     - それ以外 : 一つ前を未然形に変換し、「ます」「ん」を「ない」に変換
        //   - それ以外 : FIXME: 「ない」に変換
        // - 過去の「た」 : 一つ前で場合分け
        //   - 「です」 : 一つ前までで再変換し連用タ接続、「た」を追加する。
        //   - 「ます」 : 一つ前を連用タ接続、「た」を追加する。
        // - それ以外 : 変換の必要なし
        let without_sep = match last {
            // 助動詞の「だ」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "だ",
                ..
            } => morphs_to_string(&morphs) + &fixlast("だ"),

            // 助動詞の「ある」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "ある",
                ..
            } => morphs_to_string(&morphs) + &fixlast("ある"),

            // 「です」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "です",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::Adjective(_),
                    surface,
                    ..
                }) => morphs_to_string(&morphs) + surface,
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "た",
                    ..
                }) => morphs_to_string(&morphs) + &fixlast("た"),
                Some(M { surface, .. }) => {
                    morphs_to_string(&morphs)
                        + surface
                        + &if ends.is_empty() {
                            fixlast("だ")
                        } else {
                            "".into()
                        }
                }
                None => {
                    if ends.is_empty() {
                        fixlast("だ")
                    } else {
                        "".into()
                    }
                }
            },

            // 「ます」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "ます",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::Verb(_),
                    surface,
                    conjugation: Conjugation { form, kind },
                    ..
                }) => morphs_to_string(&morphs) + &fix(surface, kind, form, last_forms),
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface,
                None => "".into(),
            },

            // 「う」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "う",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "です",
                    ..
                }) => morphs_to_string(&morphs) + "だろう",
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "ます",
                    ..
                }) => match morphs.pop() {
                    Some(M {
                        surface,
                        conjugation: Conjugation { kind, form },
                        ..
                    }) => {
                        morphs_to_string(&morphs)
                            + &fix(surface, kind, form, &[F::NegativeU, F::Negative])
                            + "う"
                    }
                    None => "う".into(),
                },
                Some(M { surface, .. }) => morphs_to_string(&morphs) + surface + "う",
                None => "う".into(),
            },

            // 「ん」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "ん",
                ..
            } => match morphs.pop() {
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "ます",
                    ..
                }) => match morphs.pop() {
                    Some(M {
                        wordclass: W::Verb(_),
                        basic: "ある",
                        ..
                    }) => morphs_to_string(&morphs) + &fixlast("ない"),
                    Some(M {
                        surface,
                        conjugation: Conjugation { kind, form },
                        ..
                    }) => {
                        morphs_to_string(&morphs)
                            + &fix(surface, kind, form, &[F::Negative])
                            + &fixlast("ない")
                    }
                    None => "".into(),
                },
                Some(M {
                    surface,
                    conjugation: Conjugation { kind, form },
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &fix(surface, kind, form, &[F::Negative])
                        + &fixlast("ない")
                }
                None => fixlast("ない"),
            },

            // 過去の「た」
            M {
                wordclass: W::AuxiliaryVerb,
                basic: "た",
                ..
            } => match morphs.pop() {
                Some(
                    morph @ M {
                        wordclass: W::AuxiliaryVerb,
                        basic: "です",
                        ..
                    },
                ) => {
                    morphs.push(morph);
                    Part::new(morphs).into_impolite(&[F::ContinuousTa, F::Continuous])
                        + &fixlast("た")
                }
                Some(M {
                    wordclass: W::AuxiliaryVerb,
                    basic: "ます",
                    ..
                }) => match morphs.pop() {
                    Some(M {
                        surface,
                        conjugation: Conjugation { kind, form },
                        ..
                    }) => {
                        morphs_to_string(&morphs)
                            + &fix(surface, kind, form, &[F::ContinuousTa, F::Continuous])
                            + &fixlast("た")
                    }
                    None => fixlast("た"),
                },
                Some(M {
                    surface,
                    conjugation: Conjugation { kind, form },
                    ..
                }) => {
                    morphs_to_string(&morphs)
                        + &fix(surface, kind, form, &[F::ContinuousTa, F::Continuous])
                        + &fixlast("た")
                }
                None => fixlast("た"),
            },

            // それ以外
            M {
                surface,
                conjugation: Conjugation { kind, form },
                ..
            } => morphs_to_string(&morphs) + &fix(surface, kind, form, last_forms),
        };

        without_sep + &ends + sep_surface
    }
}

fn take_ends<'t, 'd>(morphs: &mut Vec<Morpheme<'t, 'd>>) -> String {
    use typed_igo::wordclass::Postpositional as P;
    use typed_igo::Morpheme as M;
    use typed_igo::WordClass as W;
    let mut ends = Vec::new();
    loop {
        match morphs.pop() {
            Some(M {
                wordclass: W::Postpositional(P::End),
                surface,
                ..
            }) => ends.push(surface),

            Some(M {
                wordclass: W::Postpositional(P::SupplementaryParallelEnd),
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

    ends.into_iter().rev().collect()
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
        match self.unwrap_curr().wordclass {
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

        match self.unwrap_curr().wordclass {
            // 基本は句点での分割
            W::Symbol(S::Period) => true,

            // だいたい他はそのままでよさそうだったが、接続助詞の「が」の前だけはなんか丁寧語にしな
            // いと違和感があるのでそこでも分割。
            //
            // - (OK) 確認したところ問題ありませんでした。
            // - (OK) 言ったからには実行します。
            // - (NG) 今日は良い天気だが明日は雨のようです。 (「今日は良い天気でしたが」にしたい)
            W::Postpositional(P::Conjunction) => self.unwrap_curr().basic == "が",

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
        wordclass: W::Symbol(S::Period),
        conjugation: Conjugation {
            kind: K::None,
            form: F::None,
        },
        basic,
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
        ($($testname:ident >> $from:literal => $to:literal => $inv:literal)*) => {
            $(
                #[test]
                fn $testname() {
                    assert_eq!(to_polite_sentence(&*PARSER, $from), $to);
                    assert_eq!(to_impolite_sentence(&*PARSER, $to), $inv);
                }
            )*
        };
    }

    check! {
        simple >>
        "今日は晴天だ。"
        => "今日は晴天です。"
        => "今日は晴天だ。"

        longer >>
        "前進をしない人は、後退をしているのだ。"
        => "前進をしない人は、後退をしているのです。"
        => "前進をしない人は、後退をしているのだ。"

        multiple_sentences >>
        "どんなに悔いても過去は変わらない。どれほど心配したところで未来もどうなるものでもない。いま、現在に最善を尽くすことである。"
        => "どんなに悔いても過去は変わりません。どれほど心配したところで未来もどうなるものでもありません。いま、現在に最善を尽くすことです。"
        => "どんなに悔いても過去は変わらない。どれほど心配したところで未来もどうなるものでもない。いま、現在に最善を尽くすことだ。"

        quote1 >>
        "最も重要な決定とは、何をするかではなく、何をしないかを決めることだ。"
        => "最も重要な決定とは、何をするかではなく、何をしないかを決めることです。"
        => "最も重要な決定とは、何をするかではなく、何をしないかを決めることだ。"

        quote2 >>
        "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
        => "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
        => "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんだ。"

        quote3 >>
        "善人はこの世で多くの害をなす。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことだ。"
        => "善人はこの世で多くの害をなします。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことです。"
        => "善人はこの世で多くの害をなす。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことだ。"

        adjective_past1 >>
        "今日は寒かった。"
        => "今日は寒かったです。"
        => "今日は寒かった。"

        end1 >>
        "今日はいい天気か。"
        => "今日はいい天気ですか。"
        => "今日はいい天気か。"

        lets1 >>
        "今日は勉強をしよう。"
        => "今日は勉強をしましょう。"
        => "今日は勉強をしよう。"

        nn >>
        "許さん。"
        => "許しません。"
        => "許さない。"

        already1 >>
        "京都大学は1897年の創立以来、「自重自敬」の精神に基づき自由な学風を育み、創造的な学問の世界を切り開いてきました。また、地球社会の調和ある共存に貢献することも京都大学の重要な目標です。"
        => "京都大学は1897年の創立以来、「自重自敬」の精神に基づき自由な学風を育み、創造的な学問の世界を切り開いてきました。また、地球社会の調和ある共存に貢献することも京都大学の重要な目標です。"
        => "京都大学は1897年の創立以来、「自重自敬」の精神に基づき自由な学風を育み、創造的な学問の世界を切り開いてきた。また、地球社会の調和ある共存に貢献することも京都大学の重要な目標だ。"

        already2 >>
        "一方で今、世界は20世紀には想像もしなかったような急激な変化を体験しつつあります。東西冷戦の終結によって解消するはずだった世界の対立構造は、民族間、宗教間の対立によってますます複雑かつ過酷になっています。他方、地球環境の悪化は加速し、想定外の大規模な災害や致死性の感染症が各地で猛威をふるい、金融危機は国の経済や人々の生活を根本から揺さぶっています。その荒波の中で、大学はどうあるべきかを真摯に考えて行かなければなりません。そして、国は産官学連携を推進してグローバルに活躍できる人材育成を奨励し、国際的に競争力のある大学改革を要請しています。京都大学が建学の精神に立ちつつ、どのようにこの国や社会の要請にこたえていけるかが今問われています。"
        => "一方で今、世界は20世紀には想像もしなかったような急激な変化を体験しつつあります。東西冷戦の終結によって解消するはずだった世界の対立構造は、民族間、宗教間の対立によってますます複雑かつ過酷になっています。他方、地球環境の悪化は加速し、想定外の大規模な災害や致死性の感染症が各地で猛威をふるい、金融危機は国の経済や人々の生活を根本から揺さぶっています。その荒波の中で、大学はどうあるべきかを真摯に考えて行かなければなりません。そして、国は産官学連携を推進してグローバルに活躍できる人材育成を奨励し、国際的に競争力のある大学改革を要請しています。京都大学が建学の精神に立ちつつ、どのようにこの国や社会の要請にこたえていけるかが今問われています。"
        => "一方で今、世界は20世紀には想像もしなかったような急激な変化を体験しつつある。東西冷戦の終結によって解消するはずだった世界の対立構造は、民族間、宗教間の対立によってますます複雑かつ過酷になっている。他方、地球環境の悪化は加速し、想定外の大規模な災害や致死性の感染症が各地で猛威をふるい、金融危機は国の経済や人々の生活を根本から揺さぶっている。その荒波の中で、大学はどうあるべきかを真摯に考えて行かなければならない。そして、国は産官学連携を推進してグローバルに活躍できる人材育成を奨励し、国際的に競争力のある大学改革を要請している。京都大学が建学の精神に立ちつつ、どのようにこの国や社会の要請にこたえていけるかが今問われている。"

        longlong >>
        "2019年現在、定期列車は大阪駅-金沢駅間で25往復が運転されている。うち1往復は和倉温泉駅まで延長運転されている。所要時間は大阪駅-金沢駅間が2時間35-40分である。最速列車が下り37号（2時間31分）で、表定速度が日本最速である。全列車が湖西線経由で大阪駅を発着として運転されるが、強風などで湖西線が運転見合わせになった場合は、米原駅経由で迂回運転される。米原駅では原則として運転停車だが、事情により客扱いをすることもある。2000年代に入ってからは比良おろしとよばれる強風による運転規制の強化により迂回運転が増えていたが、防風柵の設置工事により迂回運転は減少するとしている。迂回運転による所要時間の増加は約30分だが、折り返しとなる列車がさらに遅れる場合も多い。風が小康状態となり、かつ運転規制が解除されると湖西線経由に戻される。なお、何らかの理由で湖西線が不通になった事態を想定して、米原駅経由のダイヤもあらかじめ設定されている。なお北陸新幹線金沢開業以前の2015年3月13日までは、14往復が大阪駅-富山駅間、1往復が大阪駅-魚津駅間での運行であり、大阪駅-富山駅間の平均所要時間は3時間20分であった。富山駅・魚津駅発着系統は増結により12両編成で運転される場合、列車によっては金沢駅で1-9号車と10-12号車の増解結を行うことがあった。"
        => "2019年現在、定期列車は大阪駅-金沢駅間で25往復が運転されています。うち1往復は和倉温泉駅まで延長運転されています。所要時間は大阪駅-金沢駅間が2時間35-40分です。最速列車が下り37号（2時間31分）で、表定速度が日本最速です。全列車が湖西線経由で大阪駅を発着として運転されますが、強風などで湖西線が運転見合わせになった場合は、米原駅経由で迂回運転されます。米原駅では原則として運転停車ですが、事情により客扱いをすることもあります。2000年代に入ってからは比良おろしとよばれる強風による運転規制の強化により迂回運転が増えていましたが、防風柵の設置工事により迂回運転は減少するとしています。迂回運転による所要時間の増加は約30分ですが、折り返しとなる列車がさらに遅れる場合も多いです。風が小康状態となり、かつ運転規制が解除されると湖西線経由に戻されます。なお、何らかの理由で湖西線が不通になった事態を想定して、米原駅経由のダイヤもあらかじめ設定されています。なお北陸新幹線金沢開業以前の2015年3月13日までは、14往復が大阪駅-富山駅間、1往復が大阪駅-魚津駅間での運行であり、大阪駅-富山駅間の平均所要時間は3時間20分でした。富山駅・魚津駅発着系統は増結により12両編成で運転される場合、列車によっては金沢駅で1-9号車と10-12号車の増解結を行うことがありました。"
        => "2019年現在、定期列車は大阪駅-金沢駅間で25往復が運転されている。うち1往復は和倉温泉駅まで延長運転されている。所要時間は大阪駅-金沢駅間が2時間35-40分だ。最速列車が下り37号（2時間31分）で、表定速度が日本最速だ。全列車が湖西線経由で大阪駅を発着として運転されるが、強風などで湖西線が運転見合わせになった場合は、米原駅経由で迂回運転される。米原駅では原則として運転停車だが、事情により客扱いをすることもある。2000年代に入ってからは比良おろしとよばれる強風による運転規制の強化により迂回運転が増えていたが、防風柵の設置工事により迂回運転は減少するとしている。迂回運転による所要時間の増加は約30分だが、折り返しとなる列車がさらに遅れる場合も多い。風が小康状態となり、かつ運転規制が解除されると湖西線経由に戻される。なお、何らかの理由で湖西線が不通になった事態を想定して、米原駅経由のダイヤもあらかじめ設定されている。なお北陸新幹線金沢開業以前の2015年3月13日までは、14往復が大阪駅-富山駅間、1往復が大阪駅-魚津駅間での運行であり、大阪駅-富山駅間の平均所要時間は3時間20分だった。富山駅・魚津駅発着系統は増結により12両編成で運転される場合、列車によっては金沢駅で1-9号車と10-12号車の増解結を行うことがあった。"
    }
}
