#[test]
fn hover_focus_follows_the_pointer_between_panes() {
    in_own_process("hover_focus");
}

/// `focus_pane_on_hover`: a split tab of two terminals and a mod pane, the
/// pointer moving between them, and every case that must leave focus alone.
#[gpui::test]
fn hover_focus(cx: &mut gpui::TestAppContext) {
    use gpui::{Modifiers, MouseButton, Point, Pixels};
    use serde_json::json;
    if !running_alone("hover_focus") { return; }
    // A click puts the keyboard in the mod pane's own handle, like an input.
    let probe = r#"import { View, div } from "gpui-kit";
export default class Probe extends View {
  init(_props, cx) { this.handle = cx.focus_handle(); }
  render() {
    return div().size_full().track_focus(this.handle).on_mouse_down("left", () => this.handle.focus());
  }
}
"#;
    let (view, cx) = open_window(cx, "hover-probe", probe,
        json!({"id": "context", "type": "block", "content": {"tab": "active"}}));
    let mut root = crate::model::pane("a", "probe-a");
    root.split("a", super::Axis::Row, crate::model::pane("b", "probe-b"), "s0".into());
    let modpane: super::Node = serde_json::from_value(json!({"id": "m", "type": "block", "content": {"mod": "hover-probe"}})).unwrap();
    root.split("b", super::Axis::Row, modpane, "s1".into());
    view.update_in(cx, |this, window, cx| {
        this.folders.retain(|id, _| id == "hover-probe");
        let mut tab = super::Tab::new("tab", "", "a", "probe-a");
        tab.root = root;
        tab.focus("a");
        this.model.add_space("space", "test", "/", tab);
        this.sync(window, cx);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // In settings.json too: the window reloads it as time passes.
    let set_hover = |on: bool| {
        let mut person = crate::settings::load_person().unwrap_or_default();
        person.focus_pane_on_hover = on;
        crate::settings::write_person(&person).unwrap();
        let mut s = (*crate::settings::get()).clone();
        s.focus_pane_on_hover = on;
        crate::settings::set(s);
    };
    // Points inside each pane, a few px apart, so every move is a real one.
    let bounds = view.read_with(cx, |this, _| this.bounds.borrow().clone());
    let at = |id: &str, dx: f32| -> Point<Pixels> {
        let b = bounds[id];
        let c = b.center();
        gpui::point(c.x + gpui::px(dx), b.origin.y + gpui::px(20.0))
    };
    let (a, b, m) = (at("a", 0.0), at("b", 0.0), at("m", 0.0));
    let move_to = |cx: &mut gpui::VisualTestContext, p: Point<Pixels>, button: Option<MouseButton>| {
        cx.simulate_mouse_move(p, button, Modifiers::default());
        cx.run_until_parked();
    };
    let focused = |cx: &mut gpui::VisualTestContext| view.read_with(cx, |this, _| this.model.focused_pane());
    let keys_in = |cx: &mut gpui::VisualTestContext, id: &str| {
        cx.update(|window, cx| view.read(cx).terms[id].focus.is_focused(window))
    };
    let reset = |cx: &mut gpui::VisualTestContext, p: Point<Pixels>| {
        view.update_in(cx, |this, window, cx| {
            this.model.focus_pane("a");
            this.focused_pane = None;
            this.sync(window, cx);
        });
        move_to(cx, p, None);
        assert_eq!(focused(cx).as_deref(), Some("a"));
    };
    move_to(cx, a, None);

    // Off (the default): the pointer never moves focus.
    set_hover(false);
    move_to(cx, b, None);
    assert_eq!(focused(cx).as_deref(), Some("a"), "off");

    // On: the pane under a moving pointer takes focus and the keyboard.
    set_hover(true);
    move_to(cx, a, None);
    move_to(cx, b, None);
    assert_eq!(focused(cx).as_deref(), Some("b"), "hovered terminal");
    assert!(keys_in(cx, "b"), "keyboard follows the hover");
    move_to(cx, a, None);
    assert_eq!(focused(cx).as_deref(), Some("a"), "back");
    assert!(keys_in(cx, "a"));

    // A mod pane focuses too; the window holds the keyboard until a click.
    move_to(cx, m, None);
    assert_eq!(focused(cx).as_deref(), Some("m"), "hovered mod pane");

    // Focus moved by keys stays put under a still pointer, even if the
    // platform repeats the move at the same spot; real movement takes over.
    move_to(cx, b, None);
    view.update_in(cx, |this, window, cx| this.go("left", window, cx));
    assert_eq!(focused(cx).as_deref(), Some("a"), "pane.go");
    move_to(cx, b, None);
    assert_eq!(focused(cx).as_deref(), Some("a"), "same position is not a hover");
    move_to(cx, at("b", 4.0), None);
    assert_eq!(focused(cx).as_deref(), Some("b"), "real movement after pane.go");

    // A held button (a selection or drag) leaves focus where it started.
    reset(cx, a);
    move_to(cx, b, Some(MouseButton::Left));
    assert_eq!(focused(cx).as_deref(), Some("a"), "button held");

    // A popup holds the keyboard: hovering beside it changes nothing.
    reset(cx, a);
    view.update_in(cx, |this, window, cx| {
        this.open_overlay("popup".into(), json!({"mod": "hover-probe", "kind": "popup", "width": 40, "height": 40, "at": {"x": 10, "y": 300}}), window, cx);
    });
    cx.run_until_parked();
    move_to(cx, at("b", 8.0), None);
    assert_eq!(focused(cx).as_deref(), Some("a"), "popup open");
    view.update_in(cx, |this, window, cx| this.remove_overlay("popup", window, cx));
    cx.run_until_parked();

    // A focusable mod block driving the keys keeps them.
    reset(cx, a);
    view.update(cx, |this, _| this.focused_block = Some("m".into()));
    move_to(cx, at("b", 12.0), None);
    assert_eq!(focused(cx).as_deref(), Some("a"), "mod block has the keys");
    view.update(cx, |this, _| this.focused_block = None);

    // A mod's input or editor with the keyboard keeps it until a click.
    reset(cx, a);
    move_to(cx, m, None);
    cx.simulate_mouse_down(m, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(m, MouseButton::Left, Modifiers::default());
    for _ in 0..3 {
        cx.executor().advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    assert!(cx.update(|window, cx| {
        let this = view.read(cx);
        window.focused(cx).is_some() && !this.focus.is_focused(window) && !this.terms.values().any(|t| t.focus.is_focused(window))
    }), "the probe holds the keyboard");
    move_to(cx, at("b", 16.0), None);
    assert_eq!(focused(cx).as_deref(), Some("m"), "mod input has the keys");
}
