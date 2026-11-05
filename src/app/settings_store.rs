use toml::{Table, Value};

use crate::app::settings::Category;
use crate::app::settings_entries::{Control, ENTRIES, Variant};
use crate::core::config::Config;
use crate::core::rgba::Rgba;
use crate::error::ConfigError;

pub fn load(config: &Config) -> (Vec<Control>, Option<String>) {
    let mut first_problem = None;
    let controls = ENTRIES
        .iter()
        .map(|entry| {
            let stored = config
                .section(entry.category.section())
                .and_then(|section| section.get(entry.setting.key()));
            match stored.map(|value| read(&entry.control, value)) {
                Some(Some(control)) => control,
                Some(None) => {
                    first_problem.get_or_insert_with(|| {
                        format!("ignored invalid setting {}", entry.setting.key())
                    });
                    entry.control.clone()
                }
                None => entry.control.clone(),
            }
        })
        .collect();
    (controls, first_problem)
}

pub fn store(config: &mut Config, controls: &[Control]) -> Result<(), ConfigError> {
    for category in Category::ALL {
        let changed = ENTRIES
            .iter()
            .zip(controls)
            .filter(|(entry, control)| entry.category == category && entry.control != **control);
        let section: Table = changed
            .map(|(entry, control)| (entry.setting.key().to_owned(), write(control)))
            .collect();
        config.replace_section(category.section(), section);
    }
    config.save()
}

fn is_pair(options: &[Variant]) -> bool {
    options.len() == 2
}

fn read(template: &Control, value: &Value) -> Option<Control> {
    match template {
        Control::Toggle(_) => value.as_bool().map(Control::Toggle),
        Control::Choice { options, .. } if is_pair(options) => Some(Control::Choice {
            options,
            selected: usize::from(value.as_bool()?),
        }),
        Control::Choice { options, .. } => {
            let token = value.as_str()?;
            let selected = options.iter().position(|variant| variant.token == token)?;
            Some(Control::Choice { options, selected })
        }
        Control::Number {
            min, max, suffix, ..
        } => {
            let number = u32::try_from(value.as_integer()?).ok()?;
            (*min..=*max).contains(&number).then_some(Control::Number {
                value: number,
                min: *min,
                max: *max,
                suffix,
            })
        }
        Control::Color(_) => {
            Rgba::parse_hex(value.as_str()?).map(|color| Control::Color(color.opaque()))
        }
    }
}

fn write(control: &Control) -> Value {
    match control {
        Control::Toggle(on) => Value::Boolean(*on),
        Control::Choice { options, selected } if is_pair(options) => Value::Boolean(*selected == 1),
        Control::Choice { options, selected } => Value::String(
            options
                .get(*selected)
                .map(|variant| variant.token)
                .unwrap_or_default()
                .to_owned(),
        ),
        Control::Number { value, .. } => Value::Integer(i64::from(*value)),
        Control::Color(color) => Value::String(color.hex()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::settings_entries::defaults;

    #[test]
    fn every_control_survives_a_write_and_read() {
        for control in defaults() {
            assert_eq!(read(&control, &write(&control)), Some(control));
        }
    }

    #[test]
    fn tokens_are_short_lowercase_and_unique() {
        for entry in ENTRIES {
            let Control::Choice { options, .. } = &entry.control else {
                continue;
            };
            for (position, variant) in options.iter().enumerate() {
                let valid = |character: char| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                };
                assert!(variant.token.chars().all(valid), "{}", variant.token);
                assert!(
                    options[..position]
                        .iter()
                        .all(|other| other.token != variant.token),
                    "{}",
                    variant.token
                );
            }
        }
    }

    #[test]
    fn two_option_choices_are_stored_as_bools() {
        for entry in ENTRIES {
            let Control::Choice { options, .. } = &entry.control else {
                continue;
            };
            let stored = write(&entry.control);
            assert_eq!(stored.is_bool(), options.len() == 2, "{:?}", entry.setting);
        }
    }

    #[test]
    fn out_of_range_numbers_and_unknown_choices_are_refused() {
        for (control, bad) in ENTRIES
            .iter()
            .map(|entry| &entry.control)
            .filter_map(|control| match control {
                Control::Number { max, .. } => Some((control, Value::Integer(i64::from(*max) + 1))),
                Control::Choice { .. } => Some((control, Value::String("nope".to_owned()))),
                _ => None,
            })
        {
            assert_eq!(read(control, &bad), None);
        }
    }
}
