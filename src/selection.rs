//! Default component selection for the installer window and `install all`.

pub(crate) const RUST_EXPLORER: &str = "rust-explorer";

pub(crate) fn selected_by_default(name: &str, installed: bool) -> bool {
    name != RUST_EXPLORER || installed
}

/// Removals run in reverse install order; installs run in install order.
pub(crate) fn plan(rows: &[usize], installed: impl Fn(usize) -> bool, selected: impl Fn(usize) -> bool) -> (Vec<usize>, Vec<usize>) {
    (rows.iter().rev().copied().filter(|&i| installed(i) && !selected(i)).collect(),
     rows.iter().copied().filter(|&i| !installed(i) && selected(i)).collect())
}
pub(crate) fn linked(name: &str, on: bool) -> Option<(&'static str, bool)> {
    match (name, on) { (RUST_EXPLORER, true) => Some(("topbar", true)), ("topbar", false) => Some((RUST_EXPLORER, false)), _ => None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_uses_reverse_removal_and_forward_install_order() {
        let installed = [true, true, false, true, false];
        let wanted = [false, false, true, true, true];
        assert_eq!(super::plan(&[0,1,2,3,4], |i| installed[i], |i| wanted[i]), (vec![1,0], vec![2,4]));
        assert_eq!(super::linked("rust-explorer", true), Some(("topbar", true)));
        assert_eq!(super::linked("topbar", false), Some(("rust-explorer", false)));
    }
    #[test]
    fn fresh_install_does_not_replace_explorer_by_default() {
        assert!(!selected_by_default(RUST_EXPLORER, false));
        assert!(selected_by_default("topbar", false));
        assert!(selected_by_default("rust-search", false));
    }

    #[test]
    fn existing_rust_explorer_is_selected_for_gui_updates() {
        assert!(selected_by_default(RUST_EXPLORER, true));
    }
}
