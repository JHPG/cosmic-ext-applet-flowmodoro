//! The popup's two tabs. The bar is a segmented model, so the widget owns which
//! tab is selected and `active` reads it back.

use crate::tr::tr;
use cosmic::widget::segmented_button::{Entity, SingleSelectModel};
use cosmic::widget::tab_bar;
use cosmic::Element;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Tab {
    Timer,
    Stats,
}

pub(crate) fn model() -> SingleSelectModel {
    let t = tr();
    SingleSelectModel::builder()
        .insert(|b| b.text(t.tab_timer).data(Tab::Timer).activate())
        .insert(|b| b.text(t.tab_stats).data(Tab::Stats))
        .build()
}

pub(crate) fn active(model: &SingleSelectModel) -> Tab {
    model
        .iter()
        .find(|&e| model.is_active(e))
        .and_then(|e| model.data::<Tab>(e).copied())
        .unwrap_or(Tab::Timer)
}

pub(crate) fn bar<'a, M: Clone + 'static>(
    model: &'a SingleSelectModel,
    on_activate: impl Fn(Entity) -> M + 'static,
) -> Element<'a, M> {
    tab_bar::horizontal(model).on_activate(on_activate).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selected_tab_is_the_one_activated() {
        let mut m = model();
        assert_eq!(active(&m), Tab::Timer);
        let stats = m.iter().nth(1).unwrap();
        m.activate(stats);
        assert_eq!(active(&m), Tab::Stats);
    }
}
