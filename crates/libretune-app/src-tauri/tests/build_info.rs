//! Build information tests
//!
//! Verifies that the build ID is properly generated at compile time
//! with the expected format: YYYY.MM.DD+g<short-sha>

#[test]
fn test_build_id_format() {
    // Get the build ID from the environment variable set by build.rs
    let build_id = option_env!("LIBRETUNE_BUILD_ID").unwrap_or("unknown");

    // Build ID should be in format: YYYY.MM.DD+g<short-sha>
    // Examples: 2026.01.31+g3ab2a3f, unknown (if git unavailable)

    println!("Build ID: {}", build_id);

    if build_id != "unknown" {
        // Parse the format: should have date+git parts
        let parts: Vec<&str> = build_id.split('+').collect();
        assert_eq!(
            parts.len(),
            2,
            "Build ID should have date and git parts separated by +"
        );

        let date_part = parts[0];
        let git_part = parts[1];

        // Date should be YYYY.MM.DD
        let date_pieces: Vec<&str> = date_part.split('.').collect();
        assert_eq!(
            date_pieces.len(),
            3,
            "Date should have 3 components (YYYY.MM.DD)"
        );

        // Year should be 4 digits
        assert_eq!(date_pieces[0].len(), 4, "Year should be 4 digits");
        date_pieces[0]
            .parse::<u16>()
            .expect("Year should be valid number");

        // Month should be 2 digits
        assert_eq!(date_pieces[1].len(), 2, "Month should be 2 digits");
        let month: u8 = date_pieces[1]
            .parse()
            .expect("Month should be valid number");
        assert!((1..=12).contains(&month), "Month should be 01-12");

        // Day should be 2 digits
        assert_eq!(date_pieces[2].len(), 2, "Day should be 2 digits");
        let day: u8 = date_pieces[2].parse().expect("Day should be valid number");
        assert!((1..=31).contains(&day), "Day should be 01-31");

        // Git part should be 'g' followed by hex characters (short SHA)
        assert!(git_part.starts_with('g'), "Git part should start with 'g'");
        let sha = &git_part[1..];
        assert!(!sha.is_empty(), "Short SHA should not be empty");
        assert!(
            sha.chars().all(|c| c.is_ascii_hexdigit()),
            "Short SHA should contain only hex characters"
        );
    } else {
        println!("Build ID is 'unknown' - git may not be available in build environment");
    }
}

#[test]
fn test_build_id_not_empty() {
    let build_id = option_env!("LIBRETUNE_BUILD_ID").unwrap_or("unknown");
    assert!(!build_id.is_empty(), "Build ID should not be empty");
}
