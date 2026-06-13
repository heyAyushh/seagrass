use crate::document::ParsedDocument;

#[test]
fn trait_impl_method_resolves_as_associated_value() {
    let source = r#"
pub trait AnchorSerialize {
    fn serialize(&self) -> Result<()>;
}

pub struct MyType;

impl AnchorSerialize for MyType {
    fn serialize(&self) -> Result<()> {
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    assert!(document
        .symbols()
        .type_has_associated_value("MyType", "serialize"));
}
