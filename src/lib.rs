use scopefunc::ScopeFunc;
use typed_igo::conjugation::{ConjugationForm, ConjugationKind};
use typed_igo::wordclass::{Noun, Postpositional, Symbol, Verb};
use typed_igo::{Conjugation, Morpheme, Parser, WordClass};

pub fn to_polite_sentence(parser: &Parser, orig: &str) -> String {
    parser
        .parse(orig)
        .transform(break_into_parts)
        .into_iter()
        .map(|(part, pp)| to_polite_part(part, pp))
        .collect()
}

fn break_into_parts<'text, 'dict>(
    orig: Vec<Morpheme<'text, 'dict>>,
) -> Vec<(Vec<Morpheme<'text, 'dict>>, Morpheme<'text, 'dict>)> {
    let mut parts = Vec::new();
    let mut part = Vec::new();
    let mut paren_level = 0;

    fn add<'t, 'd>(
        parts: &mut Vec<(Vec<Morpheme<'t, 'd>>, Morpheme<'t, 'd>)>,
        part: &mut Vec<Morpheme<'t, 'd>>,
        sep: Morpheme<'t, 'd>,
    ) {
        use std::mem::replace;
        parts.push((replace(part, Vec::new()), sep));
    }

    let mut iter = orig.into_iter().peekable();
    while let Some(morph) = iter.next() {
        match morph.word_class {
            WordClass::Symbol(Symbol::OpenParen) => {
                paren_level += 1;
                part.push(morph);
            }
            WordClass::Symbol(Symbol::CloseParen) => {
                paren_level -= 1;
                part.push(morph);
            }
            WordClass::Symbol(Symbol::Period) | WordClass::Symbol(Symbol::Comma)
                if paren_level == 0 =>
            {
                add(&mut parts, &mut part, morph);
            }
            WordClass::Postpositional(_) if paren_level == 0 => {
                if iter.peek().map(|m| m.word_class) == Some(WordClass::Verb(Verb::Dependent)) {
                    part.push(morph);
                } else {
                    add(&mut parts, &mut part, morph);
                }
            }
            WordClass::AuxiliaryVerb if paren_level == 0 => {
                if morph.original_form == "た" {
                    if let Some(Morpheme {
                        word_class: WordClass::Noun(_),
                        ..
                    }) = part.last()
                    {
                        add(&mut parts, &mut part, morph);
                        continue;
                    }
                }
                part.push(morph);
            }
            _ => part.push(morph),
        }
    }

    if !part.is_empty() {
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
        parts.push((part, period));
    }

    parts
}

fn to_polite_part(part: Vec<Morpheme>, sep: Morpheme) -> String {
    let (last_orig, last_surface, last_class, last_conj) = match part.last() {
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
        return part
            .modify(|part| part.push(sep))
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

    let mut part: Vec<String> = part.into_iter().map(|x| x.surface.to_string()).collect();

    if let (Some(desu), Some(masu)) = (desu, masu) {
        if last_conj.form != ConjugationForm::Basic && last_conj.form != ConjugationForm::None {
            // 終止形以外のものにですますをつけるのは違和感があるかくどいかになる
        } else if last_class == WordClass::AuxiliaryVerb && last_orig == "だ" {
            *part.last_mut().unwrap() = desu;
        } else if last_class == WordClass::AuxiliaryVerb && last_orig == "ある" {
            // であるを想定
            part.pop();
            if let Some(last) = part.last_mut() {
                *last = desu;
            } else {
                part.push("あり".into());
                part.push(masu);
            }
        } else if let WordClass::Noun(Noun::CanBeAdverb) = last_class {
            // 単独で「いま」などのパターンがあるので単独で何もしない
        } else if let WordClass::Verb(_) = last_class {
            match last_conj.kind {
                ConjugationKind::SahenSuruConnected => {
                    // WORKAROUND
                    *part.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::Negative,
                    )
                    .expect("failed to convert (sahen-suru connected)");
                }
                ConjugationKind::SahenZuruConnected => {
                    let last_base = &last_orig[0..last_orig.len() - "ずる".len()];
                    *part.last_mut().unwrap() = format!("{}じ", last_base);
                }
                ConjugationKind::IchidanRu => {
                    // FIXME: なにをすればいいんだ？何が 一段・ル なんだ？
                }
                ConjugationKind::SpecialNai | ConjugationKind::SpecialTai => {
                    // WORKAROUND
                    *part.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::ContinuousDe,
                    )
                    .expect("failed to convert (special nai/tai)")
                }
                _ => {
                    *part.last_mut().unwrap() = conjugation::convert(
                        last_surface,
                        last_conj.kind,
                        last_conj.form,
                        ConjugationForm::Continuous,
                    )
                    .expect("failed to convert");
                }
            }
            part.push(masu);
        } else {
            part.push(desu);
        }
    }

    part.push(sep.surface.to_string());
    part.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let parser = Parser::new();
        assert_eq!(
            to_polite_sentence(&parser, "今日は晴天だ。"),
            "今日は晴天です。"
        );
        assert_eq!(
            to_polite_sentence(&parser, "前進をしない人は、後退をしているのだ。"),
            "前進をしない人は、後退をしているのです。"
        );
        assert_eq!(
            to_polite_sentence(
                &parser,
                "どんなに悔いても過去は変わらない。どれほど心配したところで未来もどうなるものでもない。いま、現在に最善を尽くすことである。"
            ),
            "どんなに悔いても過去は変わらないです。どれほど心配したところで未来もどうなるものでもないです。いま、現在に最善を尽くすことです。"
        );
        assert_eq!(
            to_polite_sentence(
                &parser,
                "最も重要な決定とは、何をするかではなく、何をしないかを決めることだ。"
            ),
            "最も重要な決定とは、何をするかではなく、何をしないかを決めることです。"
        );
        assert_eq!(
            to_polite_sentence(
                &parser,
                "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
            ),
            "数えきれないほど、悔しい思いをしてきたけれどその度にお袋の「我慢しなさい」って言葉を思い浮かべて、なんとか笑ってきたんです。"
        );
        assert_eq!(
            to_polite_sentence(
                &parser,
                "善人はこの世で多くの害をなす。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことだ。"
            ),
            "善人はこの世で多くの害をなします。彼らがなす最大の害は、人びとを善人と悪人に分けてしまうことです。"
        );
    }
}
