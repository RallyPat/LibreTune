//! The diagnostic loggers, read from a real INI rather than a fixture.
//!
//! A fixture proves the parser; only a real INI proves the app will find
//! anything to send. Nothing read these before, which is how the app came to
//! use invented command bytes.

use libretune_core::ini::{set_default_symbols, EcuDefinition};
use std::path::PathBuf;

fn demo_ini() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../libretune-app/src-tauri/resources/demo.ini")
}

#[test]
fn a_real_ini_yields_drivable_diagnostic_loggers() {
    let path = demo_ini();
    if !path.exists() {
        eprintln!("demo.ini not present; skipping");
        return;
    }
    set_default_symbols(Vec::<String>::new());
    let def = EcuDefinition::from_file(&path).expect("demo.ini parses");
    if def.diagnostic_loggers.is_empty() {
        eprintln!("this INI declares no diagnostic loggers; skipping");
        return;
    }

    for l in &def.diagnostic_loggers {
        assert!(!l.name.is_empty(), "every logger needs a name");
        assert!(
            !l.start_command.is_empty(),
            "{} has no startCommand",
            l.name
        );
        assert!(
            !l.data_read_command.is_empty(),
            "{} has no dataReadCommand - the app previously substituted the \
             start command here, which is why it read nothing",
            l.name
        );
        assert_ne!(
            l.data_read_command, l.start_command,
            "{}: reading with the start command is the original bug",
            l.name
        );
        assert!(
            l.record_len > 0,
            "{} has no recordDef, so a payload cannot be split into records",
            l.name
        );
        assert!(!l.fields.is_empty(), "{} declares no recordField", l.name);
    }

    // A tooth logger, specifically, is what the crank-irregularity work needs.
    if let Some(t) = def
        .diagnostic_loggers
        .iter()
        .find(|l| l.kind == "tooth" || l.name.contains("tooth"))
    {
        assert!(
            t.continuous_read,
            "the tooth logger declares continuousRead; without honouring it a \
             capture is one buffer, about half a second"
        );
        assert!(
            t.fields.iter().any(|f| f.bit_count == 32),
            "tooth time is a 32-bit value, not the 16-bit field the old parser assumed"
        );
    }
}
