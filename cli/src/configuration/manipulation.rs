use dprint_cli_core::types::ErrBox;
use jsonc_parser::{ast::Value, common::Ranged, parse_to_ast, ParseOptions};
use std::path::Path;

use super::ConfigFileBinary;
use crate::environment::Environment;

// todo: cleanup :)

pub fn add_binary_to_config_file(
  environment: &impl Environment,
  file_path: &Path,
  binary: &ConfigFileBinary,
  replace_index: Option<usize>,
) -> Result<(), ErrBox> {
  // todo: improve jsonc-parser
  let mut file_text = environment.read_file_text(&file_path)?;
  let value = parse_to_ast(
    &file_text,
    &ParseOptions {
      comments: false,
      tokens: false,
    },
  )?
  .value;
  let root_obj = match value {
    Some(Value::Object(obj)) => obj,
    _ => return err!("Expected a root object in the json file."),
  };

  // todo: remove clone here (improve jsonc-parser)
  let binaries_prop = root_obj
    .properties
    .iter()
    .filter(|p| p.name.as_str() == "binaries")
    .next()
    .ok_or_else(|| err_obj!("Expected to find a 'binaries' array."))?;

  let binaries_array = match &binaries_prop.value {
    Value::Array(array) => array,
    _ => return err!("Expected the 'binaries' property to contain an array."),
  };

  let previous_element_end = match replace_index {
    Some(replace_index) => {
      if replace_index == 0 {
        None
      } else {
        binaries_array.elements.get(replace_index - 1)
      }
    }
    None => binaries_array.elements.last(),
  }
  .map(|l| l.end());

  let (mut insert_pos, mut insert_end) = match replace_index {
    Some(replace_index) => {
      let element = binaries_array.elements.get(replace_index).unwrap();
      (element.start(), element.end())
    }
    None => {
      (
        previous_element_end.unwrap_or(binaries_array.start() + 1), // +1 for after bracket
        binaries_array.end() - 1,                                   // end bracket, -1 for before bracket
      )
    }
  };

  let indentation = get_indentation(&file_text, binaries_prop.start());

  // add a comma if necessary
  if let Some(previous_element_end) = previous_element_end {
    let mut has_comma = false;
    for c in file_text[previous_element_end..].chars() {
      match c {
        ' ' | '\t' | '\r' | '\n' => continue,
        ',' => {
          has_comma = true;
          break;
        }
        _ => break, // doesn't have comma
      }
    }

    if !has_comma {
      file_text = format!(
        "{},{}",
        &file_text[..previous_element_end],
        &file_text[previous_element_end..]
      );
      // add +1 for the comma
      insert_pos += 1;
      insert_end += 1;
    }
  }

  // now add the element
  let newline_char = get_newline_char(&file_text);
  let mut insert_text = String::new();
  if replace_index.is_none() {
    insert_text.push_str(&newline_char);
    insert_text.push_str(&indentation.repeat(2));
  }
  insert_text.push_str("{");
  insert_text.push_str(&newline_char);
  insert_text.push_str(&indentation.repeat(3));
  insert_text.push_str("\"path\": \"");
  insert_text.push_str(&escape_quotes(&binary.url.unresolved_path));
  insert_text.push_str("\"");

  if let Some(checksum) = &binary.url.checksum {
    insert_text.push_str(",");
    insert_text.push_str(&newline_char);
    insert_text.push_str(&indentation.repeat(3));
    insert_text.push_str("\"checksum\": \"");
    insert_text.push_str(&escape_quotes(&checksum));
    insert_text.push_str("\"");
  }

  if let Some(version) = &binary.version {
    insert_text.push_str(",");
    insert_text.push_str(&newline_char);
    insert_text.push_str(&indentation.repeat(3));
    insert_text.push_str("\"version\": \"");
    insert_text.push_str(&escape_quotes(version.as_str()));
    insert_text.push_str("\"");
  }

  insert_text.push_str(&newline_char);
  insert_text.push_str(&indentation.repeat(2));
  insert_text.push_str("}");

  if replace_index.is_none() {
    insert_text.push_str(&newline_char);
    insert_text.push_str(&indentation);
  }

  file_text = format!(
    "{}{}{}",
    &file_text[..insert_pos],
    insert_text,
    &file_text[insert_end..]
  );

  environment.write_file_text(&file_path, &file_text)?;

  Ok(())
}

fn get_newline_char(text: &str) -> String {
  // todo: don't search the entire string. Just find the first "\n" occurrence
  if text.find("\r\n").is_some() {
    "\r\n".to_string()
  } else {
    "\n".to_string() // prefer this
  }
}

fn get_indentation(text: &str, end_bracket_pos: usize) -> String {
  let leading_chars = &text[..end_bracket_pos].chars().collect::<Vec<_>>();
  let previous_char = leading_chars.get(leading_chars.len() - 1);
  match previous_char {
    Some(' ') => {
      let mut count = 1;
      for c in leading_chars.iter().rev().skip(1) {
        if c == &' ' {
          count += 1;
        } else {
          break;
        }
      }

      return " ".repeat(count);
    }
    Some('\t') => {
      return "\t".to_string();
    }
    _ => {}
  }

  "  ".to_string() // default
}

fn escape_quotes(text: &str) -> String {
  text.replace("\"", "\\\"")
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn get_indentation_two_spaces() {
    assert_eq!(get_indentation("\n  ]", 3), "  ");
  }

  #[test]
  fn get_indentation_four_spaces() {
    assert_eq!(get_indentation("\n    ]", 5), "    ");
  }

  #[test]
  fn get_indentation_eight_spaces() {
    assert_eq!(get_indentation("\n        ]", 9), "        ");
  }

  #[test]
  fn get_indentation_tabs() {
    assert_eq!(get_indentation("\n\t]", 2), "\t");
  }
}
