use minijinja::{path_loader, Environment};

use super::click::{collect_hitboxes, Hitbox};
use super::filters::add_template_helpers;
use super::model::RenderModel;

pub(super) fn render_template(
    model: &RenderModel,
    viewport_rows: usize,
    viewport_cols: usize,
) -> Result<(String, Vec<Hitbox>), minijinja::Error> {
    let mut env = Environment::new();
    add_template_helpers(&mut env);
    env.add_global("viewport_rows", viewport_rows);
    env.add_global("viewport_cols", viewport_cols);

    let captured = if let Some(template_dir) = &model.template_dir {
        env.set_loader(path_loader(template_dir));
        env.get_template(&model.template_name)?
            .render_captured(model)?
    } else {
        env.template_from_str(&model.template)?
            .render_captured(model)?
    };

    Ok(collect_hitboxes(captured.state(), captured.output()))
}
