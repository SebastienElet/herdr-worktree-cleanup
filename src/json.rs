use std::collections::BTreeMap;

const MAXIMUM_LENGTH_COERCION_DEPTH: usize = 4096;

pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<usize>),
    Object(BTreeMap<String, usize>),
}

impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(n) if *n == f64::INFINITY => "Infinity",
            Self::Number(n) if *n == f64::NEG_INFINITY => "-Infinity",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

enum Token {
    Value(Value),
    Punctuation(char),
    Identifier(String),
    End,
}

enum Parent {
    Object(usize, String),
    Array(usize),
}

pub struct Document {
    values: Vec<Value>,
    root: usize,
}

impl Document {
    pub fn parse(input: &str) -> Result<Self, String> {
        Parser {
            input: input.encode_utf16().collect(),
            position: 0,
            token: Token::End,
            values: Vec::new(),
        }
        .parse()
        .map_err(|error| format!("JSON Parse error: {error}"))
    }

    pub fn root(&self) -> &Value {
        &self.values[self.root]
    }

    pub fn property(&self, value: &Value, key: &str) -> Option<&Value> {
        match value {
            Value::Object(properties) => properties.get(key).map(|id| &self.values[*id]),
            _ => None,
        }
    }

    pub fn elements(&self, value: &Value) -> Option<impl Iterator<Item = &Value>> {
        let Value::Array(items) = value else {
            return None;
        };
        Some(items.iter().map(|id| &self.values[*id]))
    }

    pub fn number(&self, value: &Value) -> Result<f64, String> {
        match value {
            Value::Null => Ok(0.0),
            Value::Bool(b) => Ok(f64::from(u8::from(*b))),
            Value::Number(n) => Ok(*n),
            _ => {
                let text = self.primitive_string(value)?;
                let text = trim(&text);
                if text.is_empty() {
                    return Ok(0.0);
                }
                for (prefix, radix) in [
                    ("0x", 16),
                    ("0X", 16),
                    ("0o", 8),
                    ("0O", 8),
                    ("0b", 2),
                    ("0B", 2),
                ] {
                    if let Some(digits) = text.strip_prefix(prefix) {
                        if digits.is_empty() {
                            return Ok(f64::NAN);
                        }
                        return Ok(digits
                            .chars()
                            .try_fold(0.0, |number, c| {
                                c.to_digit(radix)
                                    .map(|digit| number * f64::from(radix) + f64::from(digit))
                            })
                            .unwrap_or(f64::NAN));
                    }
                }
                if matches!(
                    text.to_ascii_lowercase().as_str(),
                    "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity"
                ) && !matches!(text, "Infinity" | "+Infinity" | "-Infinity")
                {
                    return Ok(f64::NAN);
                }
                Ok(text.parse().unwrap_or(f64::NAN))
            }
        }
    }

    fn primitive_string(&self, value: &Value) -> Result<String, String> {
        let mut pending = vec![(Some(value), 0)];
        let mut output = String::new();
        while let Some((value, depth)) = pending.pop() {
            match value {
                None => output.push(','),
                Some(Value::Null) => {}
                Some(Value::Bool(b)) => output.push_str(if *b { "true" } else { "false" }),
                Some(Value::Number(n)) if *n == f64::INFINITY => output.push_str("Infinity"),
                Some(Value::Number(n)) if *n == f64::NEG_INFINITY => output.push_str("-Infinity"),
                Some(Value::Number(n)) => output.push_str(&n.to_string()),
                Some(Value::String(s)) => output.push_str(s),
                Some(Value::Array(items)) => {
                    if depth >= MAXIMUM_LENGTH_COERCION_DEPTH {
                        return Err("Maximum call stack size exceeded.".to_owned());
                    }
                    for (position, id) in items.iter().enumerate().rev() {
                        pending.push((Some(&self.values[*id]), depth + 1));
                        if position > 0 {
                            pending.push((None, depth));
                        }
                    }
                }
                Some(value @ Value::Object(_)) => {
                    if self.property(value, "toString").is_some() {
                        return Err("No default value".to_owned());
                    }
                    output.push_str("[object Object]");
                }
            }
        }
        Ok(output)
    }
}

struct Parser {
    input: Vec<u16>,
    position: usize,
    token: Token,
    values: Vec<Value>,
}

impl Parser {
    fn parse(mut self) -> Result<Document, String> {
        self.advance()?;
        let mut parents = Vec::new();
        'value: loop {
            let token = std::mem::replace(&mut self.token, Token::End);
            let mut id = self.values.len();
            match token {
                Token::Value(value) => {
                    self.values.push(value);
                    self.advance()?;
                }
                Token::Punctuation('{') => {
                    self.values.push(Value::Object(BTreeMap::new()));
                    self.advance()?;
                    if matches!(self.token, Token::Value(Value::String(_))) {
                        parents.push(Parent::Object(id, self.key()?));
                        continue;
                    }
                    self.expect('}', "Expected '}'")?;
                }
                Token::Punctuation('[') => {
                    self.values.push(Value::Array(Vec::new()));
                    self.advance()?;
                    if !matches!(self.token, Token::Punctuation(']')) {
                        parents.push(Parent::Array(id));
                        continue;
                    }
                    self.advance()?;
                }
                Token::Punctuation(c) => return Err(format!("Unexpected token '{c}'")),
                Token::Identifier(name) => {
                    let ellipsis = if name.chars().count() > 200 {
                        "..."
                    } else {
                        ""
                    };
                    return Err(format!(
                        "Unexpected identifier \"{}{ellipsis}\"",
                        name.chars().take(200).collect::<String>()
                    ));
                }
                Token::End => return Err("Unexpected EOF".to_owned()),
            }
            loop {
                match parents.pop() {
                    None => {
                        if !matches!(self.token, Token::End) {
                            return Err("Unable to parse JSON string".to_owned());
                        }
                        return Ok(Document {
                            values: self.values,
                            root: id,
                        });
                    }
                    Some(Parent::Array(parent)) => {
                        let Value::Array(items) = &mut self.values[parent] else {
                            unreachable!()
                        };
                        items.push(id);
                        if matches!(self.token, Token::Punctuation(',')) {
                            self.advance()?;
                            if matches!(self.token, Token::Punctuation(']')) {
                                return Err(
                                    "Unexpected comma at the end of array expression".to_owned()
                                );
                            }
                            parents.push(Parent::Array(parent));
                            continue 'value;
                        }
                        self.expect(']', "Expected ']'")?;
                        id = parent;
                    }
                    Some(Parent::Object(parent, key)) => {
                        let Value::Object(properties) = &mut self.values[parent] else {
                            unreachable!()
                        };
                        properties.insert(key, id);
                        if matches!(self.token, Token::Punctuation(',')) {
                            self.advance()?;
                            if !matches!(self.token, Token::Value(Value::String(_))) {
                                return Err("Property name must be a string literal".to_owned());
                            }
                            parents.push(Parent::Object(parent, self.key()?));
                            continue 'value;
                        }
                        self.expect('}', "Expected '}'")?;
                        id = parent;
                    }
                }
            }
        }
    }

    fn key(&mut self) -> Result<String, String> {
        let Token::Value(Value::String(key)) = std::mem::replace(&mut self.token, Token::End)
        else {
            unreachable!()
        };
        self.advance()?;
        self.expect(
            ':',
            "Expected ':' before value in object property definition",
        )?;
        Ok(key)
    }

    fn expect(&mut self, c: char, error: &str) -> Result<(), String> {
        if !matches!(self.token, Token::Punctuation(actual) if actual == c) {
            return Err(error.to_owned());
        }
        self.advance()
    }

    fn peek(&self) -> Option<u16> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) -> Result<(), String> {
        while matches!(self.peek(), Some(9 | 10 | 13 | 32)) {
            self.position += 1;
        }
        let Some(unit) = self.peek() else {
            self.token = Token::End;
            return Ok(());
        };
        self.token = match unit {
            34 => Token::Value(Value::String(self.string()?)),
            39 => return Err("Single quotes (') are not allowed in JSON".to_owned()),
            45 | 48..=57 => Token::Value(Value::Number(self.number()?)),
            65..=90 | 97..=122 | 36 | 95 => {
                for (literal, value) in [
                    ("true", Value::Bool(true)),
                    ("false", Value::Bool(false)),
                    ("null", Value::Null),
                ] {
                    let units: Vec<u16> = literal.encode_utf16().collect();
                    if self.input[self.position..].starts_with(&units) {
                        self.position += units.len();
                        self.token = Token::Value(value);
                        return Ok(());
                    }
                }
                let start = self.position;
                while matches!(
                    self.peek(),
                    Some(65..=90 | 97..=122 | 48..=57 | 36 | 95 | 0x200c | 0x200d)
                ) {
                    self.position += 1;
                }
                Token::Identifier(String::from_utf16_lossy(&self.input[start..self.position]))
            }
            91 | 93 | 123 | 125 | 58 | 40 | 41 | 44 | 46 | 61 | 59 => {
                self.position += 1;
                Token::Punctuation(char::from_u32(u32::from(unit)).unwrap())
            }
            _ => {
                return Err(format!(
                    "Unrecognized token '{}'",
                    String::from_utf16_lossy(&[unit])
                ));
            }
        };
        Ok(())
    }

    fn string(&mut self) -> Result<String, String> {
        self.position += 1;
        let mut units = Vec::new();
        loop {
            match self.peek() {
                Some(34) => {
                    self.position += 1;
                    return Ok(String::from_utf16_lossy(&units));
                }
                Some(0..=31) | None => return Err("Unterminated string".to_owned()),
                Some(92) => {
                    self.position += 1;
                    let Some(escape) = self.peek() else {
                        return Err("Unterminated string".to_owned());
                    };
                    let unit = match escape {
                        34 | 47 | 92 => escape,
                        98 => 8,
                        102 => 12,
                        110 => 10,
                        114 => 13,
                        116 => 9,
                        117 => {
                            let Some(digits) = self.input.get(self.position + 1..self.position + 5)
                            else {
                                return Err("\\u must be followed by 4 hex digits".to_owned());
                            };
                            let text = String::from_utf16_lossy(digits);
                            if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                                return Err(format!("\"\\u{text}\" is not a valid unicode escape"));
                            }
                            let number = u16::from_str_radix(&text, 16).map_err(|_| {
                                format!("\"\\u{text}\" is not a valid unicode escape")
                            })?;
                            self.position += 4;
                            number
                        }
                        _ => {
                            return Err(format!(
                                "Invalid escape character {}",
                                String::from_utf16_lossy(&[escape])
                            ));
                        }
                    };
                    units.push(unit);
                    self.position += 1;
                }
                Some(unit) => {
                    units.push(unit);
                    self.position += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        let start = self.position;
        if self.peek() == Some(45) {
            self.position += 1;
        }
        match self.peek() {
            Some(48) => self.position += 1,
            Some(49..=57) => {
                while matches!(self.peek(), Some(48..=57)) {
                    self.position += 1;
                }
            }
            _ => return Err("Invalid number".to_owned()),
        }
        if self.peek() == Some(46) {
            self.position += 1;
            if !matches!(self.peek(), Some(48..=57)) {
                return Err("Invalid digits after decimal point".to_owned());
            }
            while matches!(self.peek(), Some(48..=57)) {
                self.position += 1;
            }
        }
        let exponent = self.position;
        if matches!(self.peek(), Some(69 | 101)) {
            self.position += 1;
            if matches!(self.peek(), Some(43 | 45)) {
                self.position += 1;
            }
            if !matches!(self.peek(), Some(48..=57)) {
                self.position = exponent;
            } else {
                while matches!(self.peek(), Some(48..=57)) {
                    self.position += 1;
                }
            }
        }
        String::from_utf16_lossy(&self.input[start..self.position])
            .parse()
            .map_err(|_| "Invalid number".to_owned())
    }
}

pub fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}
