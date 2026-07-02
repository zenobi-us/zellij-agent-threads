use minijinja::value::ValueKind;
use minijinja::{Environment, Error, ErrorKind, Value};

pub(super) fn add_filters(env: &mut Environment<'_>) {
    env.add_filter("remap", remap);
}

fn remap(value: Value, mapping: Value) -> Result<Value, Error> {
    if mapping.kind() != ValueKind::Map {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "remap expects a map",
        ));
    }

    let mapped = mapping.get_item(&Value::from(value.to_string()))?;
    if mapped.is_undefined() {
        Ok(value)
    } else {
        Ok(mapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[test]
    fn remaps_matching_word_and_keeps_misses() {
        let mut env = Environment::new();
        add_filters(&mut env);

        let rendered = env
            .render_str(
                "{{ hit | remap({\"running\": \"RUN\"}) }} {{ miss | remap({\"idle\": \"IDLE\"}) }}",
                context! { hit => "running", miss => "other" },
            )
            .unwrap();

        assert_eq!(rendered, "RUN other");
    }
}
