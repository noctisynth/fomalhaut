//! Strict Desktop Entry `Exec` conversion without shell interpretation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecParseError {
    Empty,
    UnmatchedQuote,
    InvalidEscape,
    ReservedCharacter,
    UnsupportedFieldCode,
    InvalidExecutable,
}

pub(crate) fn parse_exec(input: &str) -> Result<Vec<String>, ExecParseError> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut characters = input.chars().peekable();
    let mut in_quotes = false;
    let mut token_started = false;
    let mut quote_closed = false;

    while let Some(character) = characters.next() {
        if quote_closed && !character.is_ascii_whitespace() {
            return Err(ExecParseError::ReservedCharacter);
        }

        match character {
            '"' if !in_quotes => {
                if token_started {
                    return Err(ExecParseError::ReservedCharacter);
                }
                in_quotes = true;
                token_started = true;
            }
            '"' => {
                in_quotes = false;
                quote_closed = true;
            }
            '\\' => {
                let escaped = characters.next().ok_or(ExecParseError::InvalidEscape)?;
                if in_quotes && !matches!(escaped, '"' | '`' | '$' | '\\') {
                    return Err(ExecParseError::InvalidEscape);
                }
                push_character(escaped, &mut characters, &mut current)?;
                token_started = true;
            }
            character if character.is_ascii_whitespace() && !in_quotes => {
                if token_started {
                    args.push(std::mem::take(&mut current));
                    token_started = false;
                    quote_closed = false;
                }
            }
            character if !in_quotes && is_reserved(character) => {
                return Err(ExecParseError::ReservedCharacter);
            }
            '`' | '$' if in_quotes => return Err(ExecParseError::ReservedCharacter),
            character => {
                push_character(character, &mut characters, &mut current)?;
                token_started = true;
            }
        }
    }

    if in_quotes {
        return Err(ExecParseError::UnmatchedQuote);
    }
    if token_started {
        args.push(current);
    }
    if args.is_empty() {
        return Err(ExecParseError::Empty);
    }
    if args[0].is_empty() || args[0].contains('=') || args[0].contains('\0') {
        return Err(ExecParseError::InvalidExecutable);
    }

    Ok(args)
}

fn push_character<I>(
    character: char,
    characters: &mut std::iter::Peekable<I>,
    output: &mut String,
) -> Result<(), ExecParseError>
where
    I: Iterator<Item = char>,
{
    if character == '%' {
        match characters.next() {
            Some('%') => output.push('%'),
            Some(_) | None => return Err(ExecParseError::UnsupportedFieldCode),
        }
    } else if character == '\0' {
        return Err(ExecParseError::InvalidExecutable);
    } else {
        output.push(character);
    }
    Ok(())
}

const fn is_reserved(character: char) -> bool {
    matches!(
        character,
        '\'' | '>' | '<' | '~' | '|' | '&' | ';' | '$' | '*' | '?' | '#' | '(' | ')' | '`'
    )
}

#[cfg(test)]
mod tests {
    use super::{ExecParseError, parse_exec};

    #[test]
    fn parses_quoted_arguments_and_literal_percent() {
        assert_eq!(
            parse_exec(r#"/usr/bin/session --name "Fomalhaut Session" --ratio 100%%"#)
                .expect("the fixture follows the supported Exec grammar"),
            [
                "/usr/bin/session",
                "--name",
                "Fomalhaut Session",
                "--ratio",
                "100%"
            ]
        );
    }

    #[test]
    fn rejects_shell_syntax_and_field_codes() {
        assert!(matches!(
            parse_exec("session; reboot"),
            Err(ExecParseError::ReservedCharacter)
        ));
        assert!(matches!(
            parse_exec("session %U"),
            Err(ExecParseError::UnsupportedFieldCode)
        ));
        assert!(matches!(
            parse_exec("\"unterminated"),
            Err(ExecParseError::UnmatchedQuote)
        ));
    }
}
