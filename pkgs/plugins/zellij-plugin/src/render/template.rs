use minijinja::Environment;

use super::click::{collect_hitboxes, Hitbox};
use super::filters::add_template_helpers;
use super::model::RenderModel;

pub(super) fn render_template(
    model: &RenderModel,
) -> Result<(String, Vec<Hitbox>), minijinja::Error> {
    let mut env = Environment::new();
    add_template_helpers(&mut env);
    let captured = env
        .template_from_str(&model.template)?
        .render_captured(model)?;
    Ok(collect_hitboxes(captured.state(), captured.output()))
}
