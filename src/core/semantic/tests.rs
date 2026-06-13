use {super::*, tower_lsp::lsp_types::Range};

mod queries;

fn field(name: &str) -> AccountField {
    AccountField {
        name: name.to_string(),
        source_range: Range::default(),
        account_type: AccountType::Unknown,
        constraints: Vec::new(),
        token_interface_candidate: false,
    }
}

fn accounts_struct(name: &str, fields: Vec<AccountField>) -> AccountsStruct {
    AccountsStruct {
        name: name.to_string(),
        fields,
        composite_refs: Vec::new(),
    }
}

fn populated_accounts(accounts_struct: AccountsStruct) -> Populated<AccountsStruct> {
    Populated::new(accounts_struct, ExtractionConfidence::MacroAnnounced)
}

#[test]
fn all_fields_for_struct_returns_flat_fields() {
    let model = SemanticModel {
        accounts_structs: vec![populated_accounts(accounts_struct(
            "Root",
            vec![field("payer"), field("state")],
        ))],
        ..SemanticModel::default()
    };

    let names = model
        .all_fields_for_struct("Root")
        .into_iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["payer", "state"]);
}

#[test]
fn all_fields_for_struct_follows_composites() {
    let mut root = accounts_struct("Root", vec![field("state")]);
    root.composite_refs.push(CompositeRef {
        field_name: "trade".to_string(),
        target_struct_name: "Trade".to_string(),
        resolved: None,
    });
    let model = SemanticModel {
        accounts_structs: vec![
            populated_accounts(root),
            populated_accounts(accounts_struct("Trade", vec![field("taker")])),
        ],
        ..SemanticModel::default()
    };

    let names = model
        .all_fields_for_struct("Root")
        .into_iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["state", "taker"]);
}

#[test]
fn all_fields_for_struct_guards_circular_composites() {
    let mut root = accounts_struct("Root", vec![field("state")]);
    root.composite_refs.push(CompositeRef {
        field_name: "next".to_string(),
        target_struct_name: "Root".to_string(),
        resolved: None,
    });
    let model = SemanticModel {
        accounts_structs: vec![populated_accounts(root)],
        ..SemanticModel::default()
    };

    let names = model
        .all_fields_for_struct("Root")
        .into_iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["state"]);
}

#[test]
fn populated_fields_has_each_defined_flag() {
    let flags = [
        PopulatedFields::COMPOSITE_REFS_RESOLVED,
        PopulatedFields::SIGNER_CHECKS,
        PopulatedFields::OWNER_CHECKS,
        PopulatedFields::DISCRIMINATOR_CHECKS,
        PopulatedFields::PDA_SEEDS,
        PopulatedFields::CPI_CALLS,
        PopulatedFields::ERROR_DISCRIMINANTS,
    ];
    let mut populated_fields = PopulatedFields::default();

    for flag in flags {
        assert!(!populated_fields.has(flag));
        populated_fields.set(flag);
        assert!(populated_fields.has(flag));
    }
}

#[test]
fn constraint_value_account_ref_round_trips() {
    let constraint = Constraint {
        key: "payer".to_string(),
        value: ConstraintValue::AccountRef("payer".to_string()),
        source_range: Range::default(),
    };

    let ConstraintValue::AccountRef(account_ref) = constraint.value else {
        panic!("expected account reference");
    };
    assert_eq!(account_ref, "payer");
}
