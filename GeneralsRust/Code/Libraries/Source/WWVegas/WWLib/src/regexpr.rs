use regex::Regex;

#[derive(Debug, Clone)]
pub struct RegularExpressionClass {
    expr_string: String,
    regex: Option<Regex>,
    is_valid: bool,
}

impl RegularExpressionClass {
    pub fn new(expression: Option<&str>) -> Self {
        let mut instance = Self {
            expr_string: String::new(),
            regex: None,
            is_valid: false,
        };
        if let Some(expr) = expression {
            instance.compile(expr);
        }
        instance
    }

    pub fn compile(&mut self, expression: &str) -> bool {
        self.clear_expression();
        match Regex::new(expression) {
            Ok(regex) => {
                self.is_valid = true;
                self.expr_string = expression.to_string();
                self.regex = Some(regex);
                true
            }
            Err(_) => false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn matches(&self, string: &str) -> bool {
        if !self.is_valid {
            return false;
        }
        if let Some(regex) = &self.regex {
            if let Some(mat) = regex.find(string) {
                return mat.start() == 0;
            }
        }
        false
    }

    fn clear_expression(&mut self) {
        self.expr_string.clear();
        self.regex = None;
        self.is_valid = false;
    }
}

impl PartialEq for RegularExpressionClass {
    fn eq(&self, other: &Self) -> bool {
        if self.is_valid != other.is_valid {
            return false;
        }
        if self.is_valid {
            self.expr_string == other.expr_string
        } else {
            true
        }
    }
}

impl Eq for RegularExpressionClass {}
