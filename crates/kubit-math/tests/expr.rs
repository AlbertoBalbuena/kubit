//! Behavioural tests of the expression language.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_math::expr::{
    BinOp, CycleError, EmptyContext, ExprErrorKind, ExprKind, MAX_NESTING_DEPTH, ParamError,
    ParameterSet, Span, format_number,
};
use kubit_math::{Dim, Input, Quantity, evaluate, parse, parse_definition, parse_input};
use std::collections::BTreeMap;

fn ctx(items: &[(&str, Quantity)]) -> BTreeMap<String, Quantity> {
    items.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn ev(src: &str) -> Quantity {
    evaluate(src, &EmptyContext).unwrap()
}

fn mm(src: &str) -> f64 {
    let q = ev(src);
    assert_eq!(q.dim, Dim::LENGTH, "{src}");
    q.value
}

fn scalar(src: &str) -> f64 {
    let q = ev(src);
    assert_eq!(q.dim, Dim::SCALAR, "{src}");
    q.value
}

fn code(src: &str) -> &'static str {
    evaluate(src, &EmptyContext).unwrap_err().code()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(scalar("1 + 2 * 3"), 7.0);
    assert_eq!(scalar("(1 + 2) * 3"), 9.0);
    assert_eq!(scalar("10 - 4 - 3"), 3.0);
    assert_eq!(scalar("100 / 10 / 5"), 2.0);
    assert_eq!(scalar("2 ^ 3 ^ 2"), 512.0);
    assert_eq!(scalar("-2 ^ 2"), -4.0);
    assert_eq!(scalar("2 ^ -1"), 0.5);
    assert_eq!(scalar("--3"), 3.0);
    assert_eq!(scalar("+3"), 3.0);
    assert_eq!(scalar("- -3 - -3"), 6.0);
    assert_eq!(scalar(".5 + 1."), 1.5);
}

#[test]
fn units_convert_to_canonical() {
    assert_eq!(mm("25.4 mm"), 25.4);
    assert_eq!(mm("1 in"), 25.4);
    assert_eq!(mm("1\""), 25.4);
    assert_eq!(mm("2.54 cm"), 25.4);
    assert!(close(mm("0.0254 m"), 25.4));
    assert!(close(mm("1 ft"), 304.8));
    let a = ev("180 deg");
    assert_eq!(a.dim, Dim::ANGLE);
    assert!(close(a.value, std::f64::consts::PI));
    assert!(close(ev("90°").value, std::f64::consts::FRAC_PI_2));
    assert_eq!(ev("2 rad"), Quantity::angle_rad(2.0));
    assert_eq!(mm("3mm"), 3.0);
}

#[test]
fn fractions_and_mixed_numbers() {
    assert!(close(mm("3/4 in"), 19.05));
    assert!(close(mm("3/4\""), 19.05));
    assert!(close(mm("1 1/2 in"), 38.1));
    assert!(close(mm("-1 1/2 in"), -38.1));
    assert_eq!(code("2 + 1/2 in"), "math.expr.dimension_mismatch");
    assert_eq!(scalar("1 1/2"), 1.5);
    assert_eq!(scalar("3/4"), 0.75);
    assert!(close(mm("1/4 in + 1/4 in"), 12.7));
    // A fraction is not a division by a unit quantity, and it does not capture exponents.
    assert_eq!(scalar("2^3/4 * 1"), 2.0);
    assert_eq!(
        ev("2^3/4 in").dim,
        Dim {
            length: -1,
            angle: 0
        }
    );
    // Division by a dimensioned value keeps its dimension algebra.
    let q = ev("1 / (4 in)");
    assert_eq!(
        q.dim,
        Dim {
            length: -1,
            angle: 0
        }
    );
}

#[test]
fn identifiers_and_dimensions() {
    let c = ctx(&[
        ("ancho", Quantity::length_mm(800.0)),
        ("Body1.height", Quantity::length_mm(720.0)),
        ("n", Quantity::scalar(3.0)),
    ]);
    let q = evaluate("ancho/2 + 3 mm", &c).unwrap();
    assert_eq!(q, Quantity::length_mm(403.0));
    assert_eq!(
        evaluate("=ancho/2", &c).unwrap(),
        Quantity::length_mm(400.0)
    );
    assert_eq!(evaluate("Body1.height - 18mm", &c).unwrap().value, 702.0);
    assert_eq!(
        evaluate("ancho * n", &c).unwrap(),
        Quantity::length_mm(2400.0)
    );
    assert_eq!(
        evaluate("ancho * ancho", &c).unwrap().dim,
        Dim {
            length: 2,
            angle: 0
        }
    );
    assert_eq!(
        evaluate("ancho / ancho", &c).unwrap(),
        Quantity::scalar(1.0)
    );
    assert_eq!(evaluate("18mm + ancho", &c).unwrap().value, 818.0);
    let e = evaluate("ancho + 1", &c).unwrap_err();
    assert_eq!(e.code(), "math.expr.dimension_mismatch");
    let e = evaluate("fondo", &c).unwrap_err();
    assert_eq!(
        e.kind,
        ExprErrorKind::UnknownIdentifier {
            name: "fondo".into()
        }
    );
    assert_eq!(e.span, Span::new(0, 5));
}

#[test]
fn dimension_errors() {
    assert_eq!(code("3 mm + 2 deg"), "math.expr.dimension_mismatch");
    assert_eq!(code("3 mm + 2"), "math.expr.dimension_mismatch");
    assert_eq!(code("(3 mm) mm"), "math.expr.invalid_operand_dimension");
    assert_eq!(code("2 ^ (3 mm)"), "math.expr.invalid_operand_dimension");
    assert_eq!(code("(2 mm) ^ 0.5"), "math.expr.invalid_operand_dimension");
    assert_eq!(code("(2 mm) ^ 7"), "math.expr.invalid_operand_dimension");
    assert_eq!(
        code("(2 mm) ^ 3 * (2 mm) ^ 4"),
        "math.expr.exponent_overflow"
    );
    assert_eq!(code("sin(3 mm)"), "math.expr.invalid_operand_dimension");
    assert_eq!(code("max(1 mm, 1 deg)"), "math.expr.dimension_mismatch");
    assert_eq!(code("sqrt(3 mm)"), "math.expr.invalid_operand_dimension");
}

#[test]
fn power_with_dimensions() {
    assert_eq!(
        ev("(3 mm) ^ 2"),
        Quantity::new(
            9.0,
            Dim {
                length: 2,
                angle: 0
            }
        )
    );
    assert_eq!(
        ev("(2 mm) ^ -1").dim,
        Dim {
            length: -1,
            angle: 0
        }
    );
    assert_eq!(mm("sqrt((3 mm) ^ 2)"), 3.0);
    assert_eq!(ev("2 ^ 0.5").value, 2f64.sqrt());
    assert_eq!(code("(-8) ^ 0.5"), "math.expr.domain_error");
    assert_eq!(code("(0 mm) ^ -1"), "math.expr.division_by_zero");
}

#[test]
fn functions() {
    assert!(close(scalar("sin(30 deg)"), 0.5));
    assert!(close(scalar("cos(60 deg)"), 0.5));
    assert!(close(scalar("tan(45 deg)"), 1.0));
    assert!(close(scalar("sin(pi/2)"), 1.0));
    assert!(close(scalar("cos(pi)"), -1.0));
    assert_eq!(scalar("sqrt(16)"), 4.0);
    assert_eq!(mm("abs(-5 mm)"), 5.0);
    assert_eq!(mm("min(3 mm, 1 in, 10 mm)"), 3.0);
    assert_eq!(mm("max(3 mm, 1 in, 10 mm)"), 25.4);
    assert_eq!(scalar("min(7)"), 7.0);
    assert_eq!(scalar("round(2.5)"), 3.0);
    assert_eq!(scalar("round(-2.5)"), -3.0);
    assert_eq!(scalar("floor(-1.5)"), -2.0);
    assert_eq!(scalar("ceil(1.2)"), 2.0);
    assert_eq!(mm("ceil(1.2 mm)"), 2.0);
    assert_eq!(scalar("pi"), std::f64::consts::PI);
    assert_eq!(code("sqrt(-1)"), "math.expr.domain_error");
}

#[test]
fn function_call_errors() {
    assert_eq!(code("foo(1)"), "math.expr.unknown_function");
    assert_eq!(code("pi(1)"), "math.expr.unknown_function");
    assert_eq!(code("sin()"), "math.expr.wrong_argument_count");
    assert_eq!(code("sin(1, 2)"), "math.expr.wrong_argument_count");
    assert_eq!(code("min()"), "math.expr.wrong_argument_count");
    let many = format!("max({})", vec!["1"; 17].join(","));
    assert_eq!(code(&many), "math.expr.wrong_argument_count");
    let ok = format!("max({})", vec!["1"; 16].join(","));
    assert_eq!(scalar(&ok), 1.0);
}

#[test]
fn arithmetic_failures() {
    assert_eq!(code("1/0"), "math.expr.division_by_zero");
    assert_eq!(code("1/(2-2)"), "math.expr.division_by_zero");
    assert_eq!(code("10^15 * 10"), "math.expr.out_of_range");
    assert_eq!(code("10^400"), "math.expr.out_of_range");
    assert_eq!(code("0^-1"), "math.expr.division_by_zero");
    assert_eq!(code("(-8)^0.5"), "math.expr.domain_error");
    assert_eq!(code("tan(pi/2) * 1"), "math.expr.out_of_range");
}

#[test]
fn syntax_errors_have_spans() {
    let e = parse("1 +").unwrap_err();
    assert_eq!(
        e.kind,
        ExprErrorKind::UnexpectedEnd {
            expected: "an expression"
        }
    );
    assert_eq!(e.span, Span::new(3, 3));
    let e = parse("(1 + 2").unwrap_err();
    assert_eq!(e.code(), "math.expr.unexpected_end");
    let e = parse("1 2").unwrap_err();
    assert_eq!(e.code(), "math.expr.unexpected_token");
    assert_eq!(e.span, Span::new(2, 3));
    let e = parse("1 + * 2").unwrap_err();
    assert_eq!(e.span, Span::new(4, 5));
    assert_eq!(parse("").unwrap_err().code(), "math.expr.unexpected_end");
    assert_eq!(parse("   ").unwrap_err().code(), "math.expr.unexpected_end");
    assert_eq!(
        parse("1 $ 2").unwrap_err().code(),
        "math.expr.unexpected_char"
    );
    assert_eq!(
        parse("1 = 2").unwrap_err().code(),
        "math.expr.unexpected_token"
    );
    assert_eq!(
        parse("3 foo").unwrap_err().code(),
        "math.expr.unexpected_token"
    );
    assert_eq!(
        parse("sin 3").unwrap_err().code(),
        "math.expr.unexpected_token"
    );
    let e = parse("2 * foo(1)").unwrap_err();
    assert_eq!(e.span, Span::new(4, 7));
    let msg = e.to_string();
    assert!(msg.contains("unknown function"), "{msg}");
    // Spans are byte offsets, not char offsets.
    let e = parse("é + $").unwrap_err();
    assert_eq!(e.span, Span::new(5, 6));
}

#[test]
fn eval_error_spans_point_at_the_operation() {
    let e = evaluate("1 + (3 mm + 2 deg)", &EmptyContext).unwrap_err();
    assert_eq!(e.code(), "math.expr.dimension_mismatch");
    assert_eq!(e.span, Span::new(4, 18));
}

#[test]
fn leading_equals_is_optional() {
    assert_eq!(scalar("=1+1"), 2.0);
    assert_eq!(scalar("1+1"), 2.0);
    assert_eq!(scalar("= 1+1"), 2.0);
}

#[test]
fn definitions() {
    let d = parse_definition("=alto_zoclo = 100 mm").unwrap();
    assert_eq!(d.name, "alto_zoclo");
    assert_eq!(d.name_span, Span::new(1, 11));
    assert_eq!(
        d.expr.eval(&EmptyContext).unwrap(),
        Quantity::length_mm(100.0)
    );
    assert_eq!(parse_definition("ancho = 3/4 in").unwrap().name, "ancho");
    match parse_input("=h = a + 1").unwrap() {
        Input::Definition(d) => assert_eq!(d.expr.references().len(), 1),
        Input::Expression(_) => panic!("expected definition"),
    }
    assert!(matches!(parse_input("=a/2").unwrap(), Input::Expression(_)));
    assert!(matches!(parse_input("a").unwrap(), Input::Expression(_)));
    assert_eq!(
        parse_definition("=a/2").unwrap_err().code(),
        "math.expr.not_a_definition"
    );
    for bad in ["=pi = 3", "=sin = 3", "=Body1.height = 3"] {
        assert_eq!(
            parse_definition(bad).unwrap_err().code(),
            "math.expr.invalid_definition_name",
            "{bad}"
        );
    }
    assert_eq!(
        parse_definition("=a =").unwrap_err().code(),
        "math.expr.unexpected_end"
    );
    assert_eq!(
        parse_definition("=a = b = c").unwrap_err().code(),
        "math.expr.unexpected_token"
    );
}

#[test]
fn references_are_sorted_unique_and_skip_pi_and_functions() {
    let e = parse("max(b, a) + Body1.height * b + sin(pi) + 2 mm").unwrap();
    let refs: Vec<_> = e.references().into_iter().collect();
    assert_eq!(refs, ["Body1.height", "a", "b"]);
    assert_eq!(kubit_math::expr::references(&e), e.references());
    assert!(parse("1 + 2").unwrap().references().is_empty());
}

#[test]
fn ast_shape_for_mixed_numbers() {
    let e = parse("1 1/2 in").unwrap();
    let ExprKind::Unit { expr, .. } = &e.kind else {
        panic!("{e:?}")
    };
    assert!(matches!(
        &expr.kind,
        ExprKind::Binary { op: BinOp::Add, .. }
    ));
    assert_eq!(e.span, Span::new(0, 8));
}

#[test]
fn limits_are_enforced() {
    let deep = format!(
        "{}1{}",
        "(".repeat(MAX_NESTING_DEPTH + 1),
        ")".repeat(MAX_NESTING_DEPTH + 1)
    );
    assert_eq!(parse(&deep).unwrap_err().code(), "math.expr.too_deep");
    let ok = format!(
        "{}1{}",
        "(".repeat(MAX_NESTING_DEPTH - 1),
        ")".repeat(MAX_NESTING_DEPTH - 1)
    );
    assert_eq!(scalar(&ok), 1.0);
    assert_eq!(
        parse(&"-".repeat(200)).unwrap_err().code(),
        "math.expr.too_deep"
    );
    assert_eq!(
        parse(&format!("1{}", "^2".repeat(100))).unwrap_err().code(),
        "math.expr.too_deep"
    );
    assert_eq!(
        parse(&"sin(".repeat(100)).unwrap_err().code(),
        "math.expr.too_deep"
    );
    assert_eq!(
        parse(&"1".repeat(2000)).unwrap_err().code(),
        "math.expr.input_too_long"
    );
    assert_eq!(
        parse(&"1+".repeat(300)).unwrap_err().code(),
        "math.expr.too_many_tokens"
    );
    // A long left-associative chain is within limits and evaluates without recursion trouble.
    let chain = vec!["1"; 256].join("+");
    assert_eq!(scalar(&chain), 256.0);
}

#[test]
fn number_formatting_roundtrips() {
    for v in [
        0.0, -0.0, 1.0, -1.5, 0.1, 1e-7, 123456.789, 1e15, -1e15, 5e-324,
    ] {
        let s = format_number(v).unwrap();
        assert_eq!(scalar(&s), v, "{s}");
    }
    assert_eq!(format_number(f64::NAN), None);
    assert_eq!(format_number(f64::INFINITY), None);
    assert_eq!(format_number(1e16), None);
}

fn set(defs: &[&str]) -> ParameterSet {
    let mut s = ParameterSet::new();
    for d in defs {
        s.insert_definition(parse_definition(d).unwrap());
    }
    s
}

#[test]
fn parameter_set_orders_and_evaluates() {
    let s = set(&[
        "=c = b + 1 mm",
        "=b = a * 2",
        "=a = 10 mm",
        "=z = Body1.height",
    ]);
    assert_eq!(s.len(), 4);
    assert!(!s.is_empty());
    assert_eq!(s.names().collect::<Vec<_>>(), ["a", "b", "c", "z"]);
    assert_eq!(s.evaluation_order().unwrap(), ["a", "b", "c", "z"]);
    assert_eq!(s.dependencies("c").into_iter().collect::<Vec<_>>(), ["b"]);
    assert!(s.dependencies("z").is_empty());
    let ext = ctx(&[("Body1.height", Quantity::length_mm(720.0))]);
    let v = s.evaluate(&ext).unwrap();
    assert_eq!(v["a"], Quantity::length_mm(10.0));
    assert_eq!(v["b"], Quantity::scalar(20.0).checked_length());
}

trait Hack {
    fn checked_length(self) -> Quantity;
}
impl Hack for Quantity {
    // `a * 2` where a = 10 mm is a length of 20 mm.
    fn checked_length(self) -> Quantity {
        Quantity::length_mm(self.value)
    }
}

#[test]
fn parameter_set_reports_errors() {
    let s = set(&["=a = missing + 1"]);
    match s.evaluate(&EmptyContext).unwrap_err() {
        ParamError::Eval { name, error } => {
            assert_eq!(name, "a");
            assert_eq!(error.code(), "math.expr.unknown_identifier");
        }
        other => panic!("{other:?}"),
    }
    assert!(set(&[]).evaluate(&EmptyContext).unwrap().is_empty());
}

#[test]
fn cycle_detection() {
    assert_eq!(
        set(&["=a = a + 1"]).check_cycles().unwrap_err(),
        CycleError {
            path: vec!["a".into(), "a".into()]
        }
    );
    let e = set(&["=a = b", "=b = c", "=c = a", "=d = 1"])
        .check_cycles()
        .unwrap_err();
    assert_eq!(e.path, ["a", "b", "c", "a"]);
    assert_eq!(e.to_string(), "parameter cycle: a -> b -> c -> a");
    // A node hanging off a cycle is reported through the cycle, not itself.
    let e = set(&["=a = b", "=b = a", "=x = a"])
        .check_cycles()
        .unwrap_err();
    assert_eq!(e.path, ["a", "b", "a"]);
    assert!(
        set(&["=a = 1", "=b = a", "=c = a + b"])
            .check_cycles()
            .is_ok()
    );
    assert!(matches!(
        set(&["=a = b", "=b = a"]).evaluate(&EmptyContext),
        Err(ParamError::Cycle(_))
    ));
}

#[test]
fn large_parameter_chain_does_not_recurse() {
    let mut s = ParameterSet::new();
    s.insert("p0", parse("1").unwrap());
    for i in 1..5000 {
        s.insert(format!("p{i}"), parse(&format!("p{} + 1", i - 1)).unwrap());
    }
    let v = s.evaluate(&EmptyContext).unwrap();
    assert_eq!(v["p4999"], Quantity::scalar(5000.0));
}
