//! Property tests: hostile input never panics; formatting round-trips.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_math::expr::{EmptyContext, ParameterSet, format_number};
use kubit_math::units::{FractionRounding, InchFormat, MmFormat};
use kubit_math::{Dim, Length, LengthUnit, evaluate, parse, parse_input};
use proptest::prelude::*;

const PIECES: &[&str] = &[
    "(",
    ")",
    "+",
    "-",
    "*",
    "/",
    "^",
    ",",
    "=",
    "\"",
    "°",
    " ",
    "1",
    "0",
    "3/4",
    "1.5",
    ".5",
    "mm",
    "in",
    "deg",
    "pi",
    "sin",
    "min",
    "sqrt",
    "a",
    "Body1.h",
    "99999999999",
    "é",
    ".",
];

/// Random concatenations of grammar fragments: much more likely to get deep into the parser
/// than uniformly random strings. (Indices instead of `prop_oneof!` keeps generation cheap.)
fn grammar_soup() -> impl Strategy<Value = String> {
    proptest::collection::vec(0..PIECES.len(), 0..120).prop_map(|v| {
        v.into_iter()
            .filter_map(|i| PIECES.get(i).copied())
            .collect::<String>()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn arbitrary_strings_never_panic(s in any::<String>()) {
        let _ = parse(&s);
        let _ = parse_input(&s);
        let _ = evaluate(&s, &EmptyContext);
    }

    #[test]
    fn grammar_soup_never_panics(s in grammar_soup()) {
        if let Ok(e) = parse(&s) {
            let _ = e.references();
            let _ = e.eval(&EmptyContext);
        }
        let _ = parse_input(&s);
    }

    #[test]
    fn errors_have_valid_spans(s in grammar_soup()) {
        if let Err(e) = evaluate(&s, &EmptyContext) {
            prop_assert!(e.span.start <= e.span.end);
            prop_assert!(e.span.end <= s.len());
            prop_assert!(s.is_char_boundary(e.span.start) && s.is_char_boundary(e.span.end));
            prop_assert!(e.code().starts_with("math.expr."));
        }
    }

    #[test]
    fn deep_and_long_inputs_never_panic(n in 0usize..3000, open in "[(\\-+^]|sin\\(|min\\(1,") {
        let s = open.repeat(n);
        let _ = parse(&s);
        let s2 = format!("{}1{}", open.repeat(n), ")".repeat(n));
        let _ = parse(&s2);
    }

    #[test]
    fn evaluated_results_are_finite_and_bounded(s in grammar_soup()) {
        if let Ok(q) = evaluate(&s, &EmptyContext) {
            prop_assert!(q.value.is_finite());
            prop_assert!(q.value.abs() <= kubit_math::expr::MAX_MAGNITUDE);
            prop_assert!(q.dim.length.abs() <= kubit_math::expr::MAX_DIM_EXPONENT);
        }
    }

    #[test]
    fn number_format_parse_roundtrip(x in prop_oneof![
        any::<f64>(),
        -1.0e15f64..1.0e15,
        -1.0e-3f64..1.0e-3,
        any::<i32>().prop_map(f64::from),
    ]) {
        match format_number(x) {
            Some(text) => {
                let q = evaluate(&text, &EmptyContext).unwrap_or_else(|e| panic!("{text}: {e}"));
                prop_assert_eq!(q.dim, Dim::SCALAR);
                prop_assert_eq!(q.value, x);
            }
            None => prop_assert!(!x.is_finite() || x.abs() > kubit_math::expr::MAX_MAGNITUDE),
        }
    }

    #[test]
    fn hostile_numbers_in_unit_conversion_never_panic(x in any::<f64>(), d in any::<u8>()) {
        for u in [LengthUnit::Millimetre, LengthUnit::Centimetre, LengthUnit::Metre, LengthUnit::Inch, LengthUnit::Foot] {
            if let Ok(l) = Length::new(x, u) {
                prop_assert!(l.mm().is_finite() && l.mm().abs() <= 1.0e6);
                let _ = MmFormat { decimals: d, trim_zeros: d % 2 == 0 }.format(l);
                let _ = InchFormat { max_denominator: d, rounding: FractionRounding::Nearest }.format(l);
            }
        }
    }

    #[test]
    fn mm_format_parses_back_within_precision(x in -1.0e6f64..1.0e6, dec in 0u8..=6) {
        let l = Length::from_mm(x).unwrap();
        let text = MmFormat { decimals: dec, trim_zeros: true }.format(l);
        let q = evaluate(&text, &EmptyContext).unwrap();
        prop_assert_eq!(q.dim, Dim::LENGTH);
        prop_assert!((q.value - x).abs() <= 0.5 * 10f64.powi(-i32::from(dec)) + 1e-9);
    }

    #[test]
    fn inch_format_parses_back_within_half_denominator(mm in -1.0e6f64..1.0e6, den in 1u8..=64) {
        let l = Length::from_mm(mm).unwrap();
        for rounding in [FractionRounding::Nearest, FractionRounding::TowardZero, FractionRounding::AwayFromZero] {
            let text = InchFormat { max_denominator: den, rounding }.format(l);
            let q = evaluate(&text, &EmptyContext).unwrap_or_else(|e| panic!("{text}: {e}"));
            prop_assert_eq!(q.dim, Dim::LENGTH);
            let step = 25.4 / f64::from(den);
            let tol = if rounding == FractionRounding::Nearest { step / 2.0 } else { step } + 1e-6;
            prop_assert!((q.value - mm).abs() <= tol, "{} -> {} vs {}", text, q.value, mm);
            if rounding == FractionRounding::TowardZero {
                prop_assert!(q.value.abs() <= mm.abs() + 1e-6);
            }
            if rounding == FractionRounding::AwayFromZero {
                prop_assert!(q.value.abs() >= mm.abs() - 1e-6);
            }
        }
    }

    #[test]
    fn dimensionless_arithmetic_matches_f64(a in -1.0e3f64..1.0e3, b in -1.0e3f64..1.0e3) {
        let (sa, sb) = (format_number(a).unwrap(), format_number(b).unwrap());
        let q = evaluate(&format!("({sa}) + ({sb}) * 2 - ({sa})"), &EmptyContext).unwrap();
        prop_assert_eq!(q.value, a + b * 2.0 - a);
    }

    #[test]
    fn length_unit_sum_commutes_with_conversion(a in -1.0e3f64..1.0e3, b in -1.0e3f64..1.0e3) {
        let (sa, sb) = (format_number(a).unwrap(), format_number(b).unwrap());
        let q = evaluate(&format!("({sa}) in + ({sb}) mm"), &EmptyContext).unwrap();
        prop_assert!((q.value - (a * 25.4 + b)).abs() < 1e-9);
    }

    #[test]
    fn parameter_graph_order_respects_dependencies(
        deps in proptest::collection::vec(proptest::collection::vec(0usize..12, 0..4), 1..12)
    ) {
        let n = deps.len();
        let mut set = ParameterSet::new();
        for (i, ds) in deps.iter().enumerate() {
            let body = ds.iter().map(|d| format!("p{}", d % n)).collect::<Vec<_>>();
            let src = if body.is_empty() { "1".to_owned() } else { body.join(" + ") };
            set.insert(format!("p{i}"), parse(&src).unwrap());
        }
        match set.evaluation_order() {
            Ok(order) => {
                prop_assert_eq!(order.len(), n);
                for (pos, name) in order.iter().enumerate() {
                    for d in set.dependencies(name) {
                        let dpos = order.iter().position(|x| *x == d).unwrap();
                        prop_assert!(dpos < pos);
                    }
                }
            }
            Err(c) => {
                // The reported cycle is real: each consecutive pair is a dependency edge.
                prop_assert!(c.path.len() >= 2);
                prop_assert_eq!(c.path.first(), c.path.last());
                for w in c.path.windows(2) {
                    prop_assert!(set.dependencies(&w[0]).contains(&w[1]));
                }
            }
        }
    }
}
