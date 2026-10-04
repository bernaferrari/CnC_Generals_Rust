//! Ordered field-table dispatch for one INI block.

use super::{FieldParse, INI, INIError, INIResult, parse_field_line};

impl INI {
    /// Initialize structure from INI using field parse table
    pub fn init_from_ini_with_fields<T>(
        &mut self,
        target: &mut T,
        field_parse_table: &[FieldParse<T>],
    ) -> INIResult<()> {
        loop {
            self.read_line()?;

            // C++ INI.cpp:1451-1528 — `readLine` delivers the final partial
            // line even when the file ends without a newline (retail
            // ChallengeMode.ini does), so the buffered token is processed
            // BEFORE the EOF sanity check. Only an unterminated block raises
            // the EOF error (`done == FALSE && isEOF()`).
            let line = self.buffer.clone();
            if let Some((key, value_tokens)) = parse_field_line(&line) {
                if key.eq_ignore_ascii_case("End") {
                    break;
                }

                let mut handled = false;

                for field in field_parse_table {
                    if field.token.eq_ignore_ascii_case(key) {
                        (field.parse)(self, target, &value_tokens)?;
                        handled = true;
                        break;
                    }
                }

                if !handled {
                    return Err(INIError::UnknownToken);
                }
            }

            if self.end_of_file {
                return Err(INIError::EndOfFile);
            }
        }

        Ok(())
    }

    /// Initialize structure from INI using field parse table, ignoring unknown tokens.
    pub fn init_from_ini_with_fields_allow_unknown<T>(
        &mut self,
        target: &mut T,
        field_parse_table: &[FieldParse<T>],
    ) -> INIResult<()> {
        loop {
            self.read_line()?;

            // Same C++ INI.cpp:1451-1528 ordering as `init_from_ini_with_fields`:
            // process the buffered token before the EOF sanity check so a final
            // line without a trailing newline still closes the block.
            let line = self.buffer.clone();
            if let Some((key, value_tokens)) = parse_field_line(&line) {
                if key.eq_ignore_ascii_case("End") {
                    break;
                }

                for field in field_parse_table {
                    if field.token.eq_ignore_ascii_case(key) {
                        (field.parse)(self, target, &value_tokens)?;
                        break;
                    }
                }
            }

            if self.end_of_file {
                return Err(INIError::EndOfFile);
            }
        }

        Ok(())
    }

    /// Read one block using the ordered base and derived field tables.
    /// C++ MultiIniFieldParse visits the inherited table first, without
    /// consuming a separate End token for either table.
    pub fn init_from_ini_with_inherited_fields<T, B>(
        &mut self,
        target: &mut T,
        base: fn(&mut T) -> &mut B,
        base_fields: &[FieldParse<B>],
        fields: &[FieldParse<T>],
    ) -> INIResult<()> {
        loop {
            self.read_line()?;
            let line = self.buffer.clone();
            if let Some((key, tokens)) = parse_field_line(&line) {
                if key.eq_ignore_ascii_case("End") {
                    return Ok(());
                }
                if let Some(field) = base_fields
                    .iter()
                    .find(|f| f.token.eq_ignore_ascii_case(key))
                {
                    (field.parse)(self, base(target), &tokens)?;
                } else if let Some(field) =
                    fields.iter().find(|f| f.token.eq_ignore_ascii_case(key))
                {
                    (field.parse)(self, target, &tokens)?;
                } else {
                    return Err(INIError::UnknownToken);
                }
            }
            // Process a final buffered End before checking EOF, as INI.cpp does.
            if self.end_of_file {
                return Err(INIError::EndOfFile);
            }
        }
    }
}
