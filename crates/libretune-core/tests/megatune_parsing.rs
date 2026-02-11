#[cfg(test)]
mod tests {
    use libretune_core::ini::EcuDefinition;

    #[test]
    fn test_parse_megatune_with_whitespace() {
        let content = r#"
[MegaTune]
   signature      =   "test_signature"   
   queryCommand   =   "Q"

"#;
        let def = EcuDefinition::from_str(content).unwrap();
        assert_eq!(def.signature, "test_signature");
        assert_eq!(def.query_command, "Q");
    }

    #[test]
    fn test_parse_megatune_with_comments() {
        let content = r#"
[MegaTune]
   signature = "test_signature" ; This is a comment
   queryCommand = "Q"
"#;
        let def = EcuDefinition::from_str(content).unwrap();
        assert_eq!(def.signature, "test_signature");
        assert_eq!(def.query_command, "Q");
    }

    #[test]
    fn test_parse_megatune_case_insensitive() {
        let content = r#"
[MEGATUNE]
   signature = "test_signature"
   queryCommand = "Q"
"#;
        let def = EcuDefinition::from_str(content).unwrap();
        assert_eq!(def.signature, "test_signature");
        assert_eq!(def.query_command, "Q");
    }
}
