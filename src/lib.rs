use typed_igo::conjugation::ConjugationForm;
use typed_igo::Morpheme;

pub fn to_polite_word(morpheme: Morpheme) -> String {
    let orig = morpheme.surface;
    let kind = morpheme.conjugation.kind;
    let from = morpheme.conjugation.form;

    conjugation::convert(orig, kind, from, ConjugationForm::Continuous)
        .map(|cont| format!("{}ます", cont))
        .unwrap_or_else(|_| format!("{}です", orig))
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_igo::conjugation::{ConjugationForm, ConjugationKind};
    use typed_igo::wordclass::*;
    use typed_igo::*;

    #[test]
    fn it_works() {
        let morpheme = Morpheme {
            surface: "書く",
            word_class: WordClass::Verb(Verb::Independent),
            conjugation: Conjugation {
                kind: ConjugationKind::GodanKaIonbin,
                form: ConjugationForm::Basic,
            },
            original_form: "",
            reading: "",
            pronunciation: "",
            start: 0,
        };

        assert_eq!(to_polite_word(morpheme), "書きます");
    }
}
