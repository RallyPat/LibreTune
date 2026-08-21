//! The AFR-target role, resolved from a real INI rather than a fixture.
//!
//! The unit tests around resolution build a synthetic 2x2 definition, which is
//! exactly the shape that let a units bug pass review: a fixture proves the
//! plumbing, not that a real INI declares what the code expects. These parse
//! the shipped `demo.ini` and assert the declaration is actually found, on both
//! arms of its `#if LAMBDA`.

use libretune_core::ini::{set_default_symbols, EcuDefinition, TableRole};
use std::path::PathBuf;

fn demo_ini() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../libretune-app/src-tauri/resources/demo.ini")
}

fn declared_target(def: &EcuDefinition) -> Vec<String> {
    let mut v: Vec<String> = def
        .tables
        .values()
        .filter(|t| t.role == TableRole::AfrTarget)
        .map(|t| t.name.clone())
        .collect();
    v.sort();
    v
}

#[test]
fn a_real_ini_declares_an_afr_target_table() {
    let path = demo_ini();
    if !path.exists() {
        eprintln!("demo.ini not present; skipping");
        return;
    }
    // The `#else` arm: an AFR-target project.
    set_default_symbols(Vec::<String>::new());
    let def = EcuDefinition::from_file(&path).expect("demo.ini parses");

    let targets = declared_target(&def);
    assert!(
        !targets.is_empty(),
        "a real INI must declare an AFR target; auto-discovery has nothing to \
         consult otherwise and falls back to a flat target"
    );
    assert!(
        targets.iter().any(|n| n.contains("afrTable")),
        "the non-LAMBDA arm should declare the AFR table, got {targets:?}"
    );
}

#[test]
fn the_lambda_arm_declares_the_lambda_table() {
    let path = demo_ini();
    if !path.exists() {
        return;
    }
    set_default_symbols(vec!["LAMBDA".to_string()]);
    let def = EcuDefinition::from_file(&path).expect("demo.ini parses");
    let targets = declared_target(&def);
    set_default_symbols(Vec::<String>::new()); // don't leak into other tests

    assert!(
        targets.iter().any(|n| n.contains("lambda") || n.contains("Lambda")),
        "under #if LAMBDA the declared target should be the lambda table, got \
         {targets:?} - and its values are lambda, which is why the target must \
         be normalised before it reaches the correction"
    );
}

/// The correction is `current_ve * (measured_afr / target_afr)`. Feeding it a
/// lambda target un-normalised is a ~14.8x demand on every cell. This asserts
/// the magnitude, not just that a table was found.
#[test]
fn a_lambda_target_yields_a_sane_correction_factor() {
    use libretune_core::autotune::normalise_to_afr;

    let measured_afr = 13.0_f64;
    let lambda_target = 0.88_f64;

    let naive = measured_afr / lambda_target;
    assert!(
        naive > 14.0,
        "sanity: un-normalised really is enormous ({naive:.1}x)"
    );

    let corrected = measured_afr / normalise_to_afr(lambda_target);
    assert!(
        (0.9..1.1).contains(&corrected),
        "normalised correction must be near unity, got {corrected:.3}x"
    );
}
