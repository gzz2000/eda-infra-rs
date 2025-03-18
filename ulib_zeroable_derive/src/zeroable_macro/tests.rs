use super::derive;

use regex::Regex;

fn derive_from_str(s: &str) -> Result<String, String> {
    match syn::parse_str(s).and_then(derive) {
        Ok(x) => Ok(x.to_string()),
        Err(e) => Err(e.to_compile_error().to_string()),
    }
}

#[derive(Debug)]
struct TestCase {
    code: String,
    has_errors: bool,
    expected: String,
}

#[test]
fn test_compilation() {
    fn struct_with_attr(struct_attr: &str, field_attr: &str) -> String {
        format!(
            "
            {}
            pub struct Struct<T,U,const V:()> {{
                {}
                pub left: T,
                pub right: U,
            }}
        ",
            struct_attr, field_attr,
        )
    }

    fn enum_with_attr(struct_attr: &str, field_attr: &str) -> String {
        format!(
            "
            {}
            #[repr(u8)]
            pub enum Enum<T> {{
                None,
                Some({} T)
            }}
        ",
            struct_attr, field_attr,
        )
    }

    let testcases = {
        let mut testcases = Vec::new();

        testcases.push(TestCase {
            code: "
                enum Hello{
                    A
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"Expected .*#\[repr\("#.into(),
        });
        testcases.push(TestCase {
            code: "
                enum Hello{}
            "
            .to_string(),
            has_errors: true,
            expected: r#"cannot implement Zeroable"#.into(),
        });
        testcases.push(TestCase {
            code: "
                #[repr(C)]
                enum Hello{
                    A=10,
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"0.*discriminant"#.into(),
        });
        testcases.push(TestCase {
            code: "
                pub union Union {
                    #[zero(nonzero)]
                    pub left: (),
                    #[zero(nonzero)]
                    pub right: (),
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"Expected.*fields.*without.*\(nonzero\)"#.into(),
        });
        testcases.push(TestCase {
            code: "
                #[zero(nonzero_fields)]
                pub union Union {
                    pub left: (),
                    pub right: (),
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"Expected.*more.*fields.*with.*\(zeroable\)"#.into(),
        });
        testcases.push(TestCase {
            code: "
                union Hello{}
            "
            .to_string(),
            has_errors: true,
            expected: r#"cannot implement Zeroable"#.into(),
        });
        testcases.push(TestCase {
            code: "
                #[repr(transparent)]
                union Hello{
                    a:()
                }
            "
            .to_string(),
            has_errors: false,
            expected: r#"impl.*Zeroable.*for.*Hello"#.into(),
        });
        testcases.push(TestCase {
            code: "
                struct Hello{}
            "
            .to_string(),
            has_errors: false,
            expected: r#"impl.*Zeroable.*for.*Hello"#.into(),
        });
        testcases.push(TestCase {
            code: "
                #[zero(bound=\" A:Foo \")]
                struct Hello{}
            "
            .to_string(),
            has_errors: false,
            expected: r#"A *: *Foo"#.into(),
        });

        testcases.push(TestCase {
            code: struct_with_attr("#[zero(not_zeroable(A))]", ""),
            has_errors: true,
            expected: r#"Expected.*type.*parameter"#.into(),
        });

        // Testing that const parameters are not treated as type parameters
        testcases.push(TestCase {
            code: struct_with_attr("#[zero(not_zeroable(V))]", ""),
            has_errors: true,
            expected: r#"Expected.*type.*parameter"#.into(),
        });

        testcases.push(TestCase {
            code: "
                #[zero(nonzero_fields)]
                pub struct Struct {
                    pub left: (),
                    pub right: (),
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"Cannot.*use.*attribute.*struct"#.into(),
        });
        testcases.push(TestCase {
            code: "
                #[zero(nonzero_fields)]
                pub enum Enum {
                    L,
                }
            "
            .to_string(),
            has_errors: true,
            expected: r#"Cannot.*use.*attribute.*enum"#.into(),
        });

        testcases.push(TestCase {
            code: struct_with_attr("", "#[zero(nonzero)]"),
            has_errors: true,
            expected: r#"Cannot.*use.*\(nonzero\).*attribute.*struct"#.into(),
        });
        testcases.push(TestCase {
            code: struct_with_attr("", "#[zero(zeroable)]"),
            has_errors: true,
            expected: r#"Cannot.*use.*\(zeroable\).*attribute.*struct"#.into(),
        });

        testcases.push(TestCase {
            code: enum_with_attr("", "#[zero(nonzero)]"),
            has_errors: true,
            expected: r#"Cannot.*use.*\(nonzero\).*attribute.*enum"#.into(),
        });
        testcases.push(TestCase {
            code: enum_with_attr("", "#[zero(zeroable)]"),
            has_errors: true,
            expected: r#"Cannot.*use.*\(zeroable\).*attribute.*enum"#.into(),
        });

        testcases
    };
    let mut errors = Vec::new();

    for example in testcases.iter() {
        let expected = Regex::new(&example.expected).unwrap();

        let (is_error, output) = match derive_from_str(&example.code) {
            Ok(v) => (false, v),
            Err(e) => (true, e),
        };

        if example.has_errors != is_error || !expected.is_match(&output) {
            errors.push((output, example));
        }
    }

    if !errors.is_empty() {
        panic!("{:#?}", errors);
    }
}
