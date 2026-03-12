//! Expander Module
//! 
//! Corresponds to C++ file: Tools/CRCDiff/expander.cpp
//! 
//! This module provides key/value pair template expansion functionality.

use std::collections::HashMap;

/// Key/value pair expansion map
pub type ExpansionMap = HashMap<String, String>;

/// Template expander for key/value pair substitution
pub struct Expander {
    /// Expansion mappings
    expansions: ExpansionMap,
    /// Left marker for template variables
    left_marker: String,
    /// Right marker for template variables
    right_marker: String,
}

impl Expander {
    /// Create new expander with custom markers
    pub fn new(left_marker: String, right_marker: String) -> Self {
        Self {
            expansions: HashMap::new(),
            left_marker,
            right_marker,
        }
    }
    
    /// Add expansion mapping
    pub fn add_expansion(&mut self, key: String, value: String) {
        self.expansions.insert(key, value);
    }
    
    /// Clear all expansions
    pub fn clear(&mut self) {
        self.expansions.clear();
    }
    
    /// Expand template with current mappings
    pub fn expand(&self, input: &str, strip_unknown: bool) -> String {
        let mut output = String::new();
        let mut last_pos = 0;
        
        while let Some(pos) = input[last_pos..].find(&self.left_marker) {
            let pos = pos + last_pos;
            
            // Append text before the marker
            output.push_str(&input[last_pos..pos]);
            
            // Find the end marker
            let start_pos = pos + self.left_marker.len();
            if let Some(end_pos_relative) = input[start_pos..].find(&self.right_marker) {
                let end_pos = start_pos + end_pos_relative;
                let key = &input[start_pos..end_pos];
                
                // Look up the key
                if let Some(value) = self.expansions.get(key) {
                    // Recursively expand the value
                    let expanded_value = self.expand(value, strip_unknown);
                    output.push_str(&expanded_value);
                } else if !strip_unknown {
                    // Unknown key - include the original token
                    output.push_str(&input[pos..end_pos + self.right_marker.len()]);
                }
                // If strip_unknown is true, we simply skip unknown tokens
                
                last_pos = end_pos + self.right_marker.len();
            } else {
                // No closing marker found - include the opening marker and continue
                output.push_str(&self.left_marker);
                last_pos = pos + self.left_marker.len();
            }
        }
        
        // Append remaining text
        output.push_str(&input[last_pos..]);
        output
    }
}

impl Default for Expander {
    fn default() -> Self {
        Self::new("{{".to_string(), "}}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_expansion() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("NAME".to_string(), "World".to_string());
        expander.add_expansion("GREETING".to_string(), "Hello".to_string());
        
        let result = expander.expand("((GREETING)) ((NAME))!", false);
        assert_eq!(result, "Hello World!");
    }
    
    #[test]
    fn test_recursive_expansion() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("INNER".to_string(), "World".to_string());
        expander.add_expansion("OUTER".to_string(), "Hello ((INNER))".to_string());
        
        let result = expander.expand("((OUTER))!", false);
        assert_eq!(result, "Hello World!");
    }
    
    #[test]
    fn test_unknown_keys() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("KNOWN".to_string(), "value".to_string());
        
        let result = expander.expand("((KNOWN)) ((UNKNOWN))", false);
        assert_eq!(result, "value ((UNKNOWN))");
        
        let result_stripped = expander.expand("((KNOWN)) ((UNKNOWN))", true);
        assert_eq!(result_stripped, "value ");
    }
    
    #[test]
    fn test_no_expansions() {
        let expander = Expander::new("((".to_string(), "))".to_string());
        let input = "This has no expansions";
        let result = expander.expand(input, false);
        assert_eq!(result, input);
    }
    
    #[test]
    fn test_incomplete_markers() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("TEST".to_string(), "value".to_string());
        
        let result = expander.expand("((TEST", false);
        assert_eq!(result, "((TEST");
        
        let result2 = expander.expand("((TEST)) and ((INCOMPLETE", false);
        assert_eq!(result2, "value and ((INCOMPLETE");
    }
    
    #[test]
    fn test_empty_key() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("".to_string(), "empty".to_string());
        
        let result = expander.expand("(())", false);
        assert_eq!(result, "empty");
    }
    
    #[test]
    fn test_html_template_expansion() {
        let mut expander = Expander::new("((".to_string(), "))".to_string());
        expander.add_expansion("LEFTCLASS".to_string(), "error".to_string());
        expander.add_expansion("LEFTLINE".to_string(), "Error message".to_string());
        expander.add_expansion("RIGHTCLASS".to_string(), "normal".to_string());
        expander.add_expansion("RIGHTLINE".to_string(), "Normal message".to_string());
        
        let template = r#"<tr><td class="((LEFTCLASS))">((LEFTLINE))</td><td class="((RIGHTCLASS))">((RIGHTLINE))</td></tr>"#;
        let result = expander.expand(template, false);
        assert_eq!(result, r#"<tr><td class="error">Error message</td><td class="normal">Normal message</td></tr>"#);
    }
}
