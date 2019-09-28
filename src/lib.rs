use scopefunc::ScopeFunc;
use std::iter::FusedIterator;
use typed_igo::conjugation::{ConjugationForm, ConjugationKind};
use typed_igo::wordclass::{Noun, Postpositional, Symbol};
use typed_igo::{Conjugation, Morpheme, Parser, WordClass};

pub fn to_polite_sentence(parser: &Parser, orig: &str) -> String {
    let parsed = parser.parse(orig);
    let parts = Splitter::new(parsed).break_into_parts();
    parts.into_iter().map(to_polite_part).collect()
}

struct Part<'t, 'd> {
    morphs: Vec<Morpheme<'t, 'd>>,
    sep: Morpheme<'t, 'd>,
}

impl<'t, 'd> Part<'t, 'd> {
    fn new(morphs: Vec<Morpheme<'t, 'd>>, sep: Morpheme<'t, 'd>) -> Part<'t, 'd> {
        Part { morphs, sep }
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
        self.parts.push(Part::new(part, sep));
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
            let period = Morpheme {
                surface: "。",
                word_class: WordClass::Symbol(Symbol::Period),
                conjugation: Conjugation {
                    kind: ConjugationKind::None,
                    form: ConjugationForm::None,
                },
                original_form: "。",
                reading: "。",
                pronunciation: "。",
                start: !0,
            };

            self.curr = Some(period);
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
        match self.unwrap_curr().word_class {
            WordClass::Symbol(Symbol::OpenParen) => self.paren_level += 1,
            WordClass::Symbol(Symbol::CloseParen) => self.paren_level -= 1,
            _ => {}
        }
    }

    fn should_be_break(&self) -> bool {
        use typed_igo::wordclass::{Postpositional as P, Symbol as S};
        // 括弧深度が 1 以上の場合は引用または発言とみなし、何も変換しない。つまり区切る必要もない。
        if self.paren_level >= 1 {
            return false;
        }

        match self.unwrap_curr().word_class {
            // 基本は句点での分割
            WordClass::Symbol(S::Period) => true,

            // だいたい他はそのままでよさそうだったが、接続助詞の「が」の前だけはなんか丁寧語にしな
            // いと違和感があるのでそこでも分割。
            //
            // - (OK) 確認したところ問題ありませんでした。
            // - (OK) 言ったからには実行します。
            // - (NG) 今日は良い天気だが明日は雨のようです。 (「今日は良い天気でしたが」にしたい)
            WordClass::Postpositional(P::Conjunction) => self.unwrap_curr().original_form == "が",

            // それ以外は切らない
            _ => false,
        }
    }
}

fn to_polite_part(part: Part) -> String {
    let Part { morphs, sep } = part;
    let (last_orig, last_surface, last_class, last_conj) = match morphs.last() {
        Some(last) => (
            last.original_form,
            last.surface,
            last.word_class,
            last.conjugation,
        ),
        None => return sep.original_form.into(),
    };

    // 既に丁寧語
    if last_orig == "です" || last_orig == "ます" {
        return morphs
            .modify(|morphs| morphs.push(sep))
            .into_iter()
            .map(|x| x.surface)
            .collect::<String>();
    }

    let desumasu_form = match sep.word_class {
        WordClass::Postpositional(Postpositional::Conjunction) => match sep.original_form {
            "て" => Some(ConjugationForm::Continuous),
            _ => None,
        },
        WordClass::Postpositional(_) => None,
        WordClass::AuxiliaryVerb => match sep.original_form {
            "た" => Some(ConjugationForm::Continuous),
            _ => None,
        },
        _ => Some(ConjugationForm::Basic),
    };

    let desu = desumasu_form.map(|form| {
        conjugation::convert(
            "です",
            ConjugationKind::SpecialDesu,
            ConjugationForm::Basic,
            form,
        )
        .expect("failed to convert です")
    });

    let masu = desumasu_form.map(|form| {
        conjugation::convert(
            "ます",
            ConjugationKind::SpecialMasu,
            ConjugationForm::Basic,
            form,
        )
        .expect("failed to convert ます")
    });

    let mut words: Vec<String> = morphs.into_iter().map(|x| x.surface.to_string()).collect();

    if let (Some(desu), Some(masu)) = (desu, masu) {
        if last_conj.form != ConjugationForm::Basic && last_conj.form != ConjugationForm::None {
            // 終止形以外のものにですますをつけるのは違和感があるかくどいかになる
        } else if last_class == WordClass::AuxiliaryVerb && last_orig == "だ" {
            *words.last_mut().unwrap() = desu;
        } else if last_class == WordClass::AuxiliaryVerb && last_orig == "ある" {
            // であるを想定
            words.pop();
            if let Some(last) = words.last_mut() {
                *last = desu;
            } else {
                words.push("あり".into());
                words.push(masu);
            }
        } else if let WordClass::Noun(Noun::CanBeAdverb) = last_class {
            // 単独で「いま」などのパターンがあるので単独で何もしない
        } else if let WordClass::Verb(_) = last_class {
            match last_conj.kind {
                ConjugationKind::SahenSuruConnected => {
                    // WORKAROUND
                    *words.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::Negative,
                    )
                    .expect("failed to convert (sahen-suru connected)");
                }
                ConjugationKind::SahenZuruConnected => {
                    let last_base = &last_orig[0..last_orig.len() - "ずる".len()];
                    *words.last_mut().unwrap() = format!("{}じ", last_base);
                }
                ConjugationKind::IchidanRu => {
                    // FIXME: なにをすればいいんだ？何が 一段・ル なんだ？
                }
                ConjugationKind::SpecialNai | ConjugationKind::SpecialTai => {
                    // WORKAROUND
                    *words.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::ContinuousDe,
                    )
                    .expect("failed to convert (special nai/tai)")
                }
                _ => {
                    *words.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::Continuous,
                    )
                    .expect("failed to convert");
                }
            }
            words.push(masu);
        } else {
            words.push(desu);
        }
    }

    words.push(sep.surface.to_string());
    words.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static::lazy_static! {
        static ref PARSER: Parser = Parser::new();
    }

    macro_rules! check {
        ($(# $testname:ident $from:literal => $to:literal)*) => {
            $(
                #[test]
                fn $testname() {
                    assert_eq!(to_polite_sentence(&*PARSER, $from), $to);
                }
            )*
        };
    }

    check! {
        # simple
        "今日は晴天だ。"
        => "今日は晴天です。"

        # longer
        "前進をしない人は、後退をしているのだ。"
        => "前進をしない人は、後退をしているのです。"

        # multiple_sentences
        "どんなに悔いても過去は変わらない。どれほど心配したところで未来もどうなるものでもない。いま、現在に最善を尽くすことである。"
        => "どんなに悔いても過去は変わらないです。どれほど心配したところで未来もどうなるものでもないです。いま、現在に最善を尽くすことです。"

        # quote1
        "最も重要な決定とは、何をするかではなく、何をしないかを決めることだ。"
        => "最も重要な決定とは、何をするかではなく、何をしないかを決めることです。"

        # quote2
        "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
        => "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"

        # quote3
        "善人はこの世で多くの害をなす。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことだ。"
        =>"善人はこの世で多くの害をなします。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことです。"
    }
}
