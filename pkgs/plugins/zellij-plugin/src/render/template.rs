use minijinja::Environment;

use super::filters::add_filters;
use super::model::RenderModel;

pub(super) fn render_template(model: &RenderModel) -> Result<String, minijinja::Error> {
    let mut env = Environment::new();
    add_filters(&mut env);
    env.render_str(&model.template, model)
}
