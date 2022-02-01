use anyhow::{Context, Result};

#[derive(Debug, PartialEq)]
pub enum Address {
    Pattern(String),
    Range(u32, u32),
}

pub struct Script {
    pub address: Option<Address>,
    pub command: Option<char>,
    pub placeholder: Option<String>,
    pub patten: Option<String>,
    pub replace: Option<String>,
}

struct Reader {
    chars: Vec<char>,
    pos: usize,
}

impl Reader {
    fn new(text: &String) -> Reader {
        Reader {
            chars: text.chars().collect(),
            pos: 0,
        }
    }

    /// Get token from next positon
    fn next(&mut self) -> Option<char> {
        self.pos += 1;
        if let Some(s) = self.chars.get(self.pos - 1) {
            Some(*s)
        } else {
            None
        }
    }

    /// Peek a token in current position
    fn peek(&self) -> Option<char> {
        if let Some(s) = self.chars.get(self.pos) {
            Some(*s)
        } else {
            None
        }
    }
}

struct Tokenizer {
    text: String,
    reader: Reader,
}

#[derive(Debug, PartialEq)]
enum Token {
    Number(u32),
    Char(char),
    Symbol(String),
}

fn parse_number(reader: &mut Reader) -> u32 {
    let mut num: u32 = 0;
    while let Some(ch) = reader.peek() {
        if !ch.is_ascii_digit() {
            break;
        }
        num = num * 10 + ch.to_digit(10).unwrap_or(0);
        reader.next();
    }
    num
}

fn parse_symbol(reader: &mut Reader) -> String {
    let mut s = String::new();
    while let Some(ch) = reader.peek() {
        if !ch.is_ascii_alphabetic() {
            break;
        }
        s += &ch.to_string();
        reader.next();
    }
    s
}

impl Tokenizer {
    fn new(text: String) -> Option<Tokenizer> {
        let reader = Reader::new(&text);
        Some(Tokenizer { text, reader })
    }

    fn pos(&self) -> usize {
        self.reader.pos
    }

    fn get_token(&mut self) -> Option<Token> {
        let last_char = match self.reader.peek() {
            Some(ch) => ch,
            None => return None,
        };
        if last_char.is_ascii_digit() {
            return Some(Token::Number(parse_number(&mut self.reader)));
        }
        if last_char.is_ascii_alphabetic() {
            return Some(Token::Symbol(parse_symbol(&mut self.reader)));
        }
        self.reader.next();
        Some(Token::Char(last_char))
    }

    /// Get symbol by spliting with `split`
    fn get_sym(&mut self, split: char) -> Option<Token> {
        let start_pos = self.pos();
        while let Some(token) = self.get_token() {
            match token {
                Token::Char(c) if c == split => break,
                _ => (),
            }
        }
        let end_pos = self.pos();
        let selected = match self.text.get(start_pos..end_pos - 1) {
            Some(s) => s.to_string(),
            None => return None,
        };
        Some(Token::Symbol(selected))
    }
}

/// Parse sed script with a hand-written top-down parser
pub fn parse(script: &str) -> Result<Script> {
    // TODO parse more sed script
    // Script format: [addr]X[options]
    let mut tokenizer = Tokenizer::new(script.to_string()).context("Fail to tokenizer [SCRIPT]")?;
    let mut token = tokenizer.get_token();
    // Parse address (Optional)
    let address = match token {
        Some(Token::Number(start)) => {
            if let Some(Token::Char(ch)) = tokenizer.get_token() {
                if ch != ',' {
                    return Err(anyhow::format_err!("Missing ',' in [SCRIPT]"));
                }
            }
            let end = match tokenizer.get_token() {
                Some(Token::Number(end)) => end,
                _ => return Err(anyhow::format_err!("Missing end address in [SCRIPT]")),
            };
            token = tokenizer.get_token();
            Some(Address::Range(start, end))
        }
        Some(Token::Char(ch)) if ch == '/' => {
            let pattern = tokenizer.get_token();
            match pattern {
                Some(Token::Symbol(s)) => Some(Address::Pattern(s)),
                _ => return Err(anyhow::format_err!("address format error")),
            }
        }
        _ => None,
    };
    // Parse command (Optional)
    let command = match token {
        Some(Token::Symbol(s)) => {
            let next_ch = s.chars().next();
            token = tokenizer.get_token();
            next_ch
        }
        _ => None,
    };
    // Parse placeholder (Extend)
    let placeholder = match token {
        Some(Token::Char(ch)) if ch == '@' => match tokenizer.get_token() {
            Some(Token::Symbol(s)) => {
                token = tokenizer.get_token();
                Some(s)
            }
            _ => return Err(anyhow::format_err!("Missing placeholder")),
        },
        _ => None,
    };
    if let Some(Token::Char(ch)) = token {
        if ch != '/' {
            return Err(anyhow::format_err!("Missing '/' in argument"));
        }
    }
    let patten = match tokenizer.get_sym('/') {
        Some(Token::Symbol(patten)) => Some(patten),
        _ => None,
    };
    let replace = match tokenizer.get_sym('/') {
        Some(Token::Symbol(replace)) => Some(replace),
        _ => None,
    };
    Ok(Script {
        address,
        command,
        placeholder,
        patten,
        replace,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_tokenizer() {
        let mut tokenizer = Tokenizer::new(String::from("1,2s@placeholder/aaa/bbb/")).unwrap();
        let expect_tokens = [
            Token::Number(1),
            Token::Char(','),
            Token::Number(2),
            Token::Symbol(String::from("s")),
            Token::Char('@'),
            Token::Symbol(String::from("placeholder")),
        ];
        for expect in expect_tokens {
            assert_eq!(tokenizer.get_token(), Some(expect));
        }
    }

    #[test]
    fn test_basic_parse() {
        let result = parse("s/aaa/bbb/").unwrap();
        assert_eq!(result.patten, Some(String::from("aaa")));
        assert_eq!(result.replace, Some(String::from("bbb")));
    }

    #[test]
    fn test_address_parse() {
        let result = parse("1,2s/aaa/bbb/").unwrap();
        assert_eq!(result.address, Some(Address::Range(1, 2)));
        assert_eq!(result.command, Some('s'));
        assert_eq!(result.patten, Some(String::from("aaa")));
        assert_eq!(result.replace, Some(String::from("bbb")));
    }

    #[test]
    fn test_extend_parse() {
        let result = parse("1,2s@placeholder/aaa/bbb/").unwrap();
        assert_eq!(result.address, Some(Address::Range(1, 2)));
        assert_eq!(result.command, Some('s'));
        assert_eq!(result.placeholder, Some(String::from("placeholder")));
        assert_eq!(result.patten, Some(String::from("aaa")));
        assert_eq!(result.replace, Some(String::from("bbb")));
    }

    #[test]
    fn test_patten_address() {
        let result = parse("/wow/").unwrap();
        assert_eq!(result.address, Some(Address::Pattern(String::from("wow"))));
    }

    #[test]
    fn test_tree_sitter_query() {
        let query = r#"s/(argument_list (_) @tbr)/"Just Monika"/"#;
        let result = parse(query).unwrap();
        assert_eq!(
            result.patten,
            Some(String::from("(argument_list (_) @tbr)"))
        );
        assert_eq!(result.replace, Some(String::from("\"Just Monika\"")));
    }
}
