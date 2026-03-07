use gpui::{Axis, Entity, EntityId};
use terminal::view::TerminalView;
use ui::prelude::*;

/// Ported from Zed's `workspace::pane_group`, stripped down for Dif.
/// Manages a recursive tree of terminal split panes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SplitDirection {
    pub fn axis(self) -> Axis {
        match self {
            Self::Up | Self::Down => Axis::Vertical,
            Self::Left | Self::Right => Axis::Horizontal,
        }
    }

    pub fn increasing(self) -> bool {
        matches!(self, Self::Down | Self::Right)
    }
}

/// A tree of split panes. Single-pane is just `Member::Pane`.
pub struct PaneGroup {
    pub root: Member,
}

#[derive(Clone)]
pub enum Member {
    Pane(Entity<TerminalView>),
    Axis(PaneAxis),
}

#[derive(Clone)]
pub struct PaneAxis {
    pub axis: Axis,
    pub members: Vec<Member>,
    pub flexes: Vec<f32>,
}

// ── PaneGroup ────────────────────────────────────────────────────────

impl PaneGroup {
    pub fn new(pane: Entity<TerminalView>) -> Self {
        Self {
            root: Member::Pane(pane),
        }
    }

    /// Insert `new_pane` next to `old_pane` in the given direction.
    pub fn split(
        &mut self,
        old_pane: &Entity<TerminalView>,
        new_pane: Entity<TerminalView>,
        direction: SplitDirection,
    ) {
        let found = match &mut self.root {
            Member::Pane(pane) => {
                if pane.entity_id() == old_pane.entity_id() {
                    self.root = Member::new_axis(old_pane.clone(), new_pane.clone(), direction);
                    true
                } else {
                    false
                }
            }
            Member::Axis(axis) => axis.split(old_pane, new_pane.clone(), direction),
        };

        if !found {
            let first = self.root.first_pane();
            match &mut self.root {
                Member::Pane(_) => {
                    self.root = Member::new_axis(first, new_pane, direction);
                }
                Member::Axis(axis) => {
                    let _ = axis.split(&first, new_pane, direction);
                }
            }
        }
    }

    /// Remove a pane from the tree. Returns `false` if the pane is the only
    /// remaining root (caller should decide what to do).
    pub fn remove(&mut self, pane: &Entity<TerminalView>) -> bool {
        match &mut self.root {
            Member::Pane(p) => p.entity_id() != pane.entity_id(),
            Member::Axis(axis) => {
                match axis.remove(pane) {
                    Ok(Some(last)) => {
                        self.root = last;
                    }
                    Ok(None) => {}
                    Err(()) => return false, // not found
                }
                true
            }
        }
    }

    /// Collect every pane in depth-first order.
    pub fn panes(&self) -> Vec<Entity<TerminalView>> {
        let mut out = Vec::new();
        self.root.collect_panes(&mut out);
        out
    }

    pub fn first_pane(&self) -> Entity<TerminalView> {
        self.root.first_pane()
    }

    pub fn contains(&self, pane: &Entity<TerminalView>) -> bool {
        self.root.contains(pane)
    }

    pub fn is_split(&self) -> bool {
        matches!(&self.root, Member::Axis(_))
    }

    /// Find the adjacent pane in the given direction from `from`.
    pub fn find_pane_in_direction(
        &self,
        from: &Entity<TerminalView>,
        direction: SplitDirection,
    ) -> Option<Entity<TerminalView>> {
        let panes = self.panes();
        if panes.len() < 2 {
            return None;
        }
        self.root.find_adjacent(from, direction)
    }

    /// Render the pane tree. `active_id` highlights the focused pane.
    pub fn render(&self, active_id: Option<EntityId>) -> AnyElement {
        self.root.render(active_id)
    }
}

// ── Member ───────────────────────────────────────────────────────────

impl Member {
    fn new_axis(
        old_pane: Entity<TerminalView>,
        new_pane: Entity<TerminalView>,
        direction: SplitDirection,
    ) -> Self {
        let axis = direction.axis();
        let members = if direction.increasing() {
            vec![Member::Pane(old_pane), Member::Pane(new_pane)]
        } else {
            vec![Member::Pane(new_pane), Member::Pane(old_pane)]
        };
        Member::Axis(PaneAxis::new(axis, members))
    }

    fn first_pane(&self) -> Entity<TerminalView> {
        match self {
            Member::Pane(p) => p.clone(),
            Member::Axis(axis) => axis.members[0].first_pane(),
        }
    }

    fn last_pane(&self) -> Entity<TerminalView> {
        match self {
            Member::Pane(p) => p.clone(),
            Member::Axis(axis) => axis.members.last().unwrap().last_pane(),
        }
    }

    fn contains(&self, pane: &Entity<TerminalView>) -> bool {
        match self {
            Member::Pane(p) => p.entity_id() == pane.entity_id(),
            Member::Axis(axis) => axis.members.iter().any(|m| m.contains(pane)),
        }
    }

    fn collect_panes(&self, out: &mut Vec<Entity<TerminalView>>) {
        match self {
            Member::Pane(p) => out.push(p.clone()),
            Member::Axis(axis) => {
                for member in &axis.members {
                    member.collect_panes(out);
                }
            }
        }
    }

    /// Find adjacent pane in `direction` from `from`.
    fn find_adjacent(
        &self,
        from: &Entity<TerminalView>,
        direction: SplitDirection,
    ) -> Option<Entity<TerminalView>> {
        match self {
            Member::Pane(_) => None,
            Member::Axis(axis) => axis.find_adjacent(from, direction),
        }
    }

    fn render(&self, active_id: Option<EntityId>) -> AnyElement {
        match self {
            Member::Pane(view) => {
                let is_active = active_id == Some(view.entity_id());
                let t = theme();
                div()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .when(is_active, |el| el.border_t_2().border_color(t.accent_blue))
                    .child(view.clone())
                    .into_any_element()
            }
            Member::Axis(axis) => axis.render(active_id),
        }
    }
}

// ── PaneAxis ─────────────────────────────────────────────────────────

impl PaneAxis {
    fn new(axis: Axis, members: Vec<Member>) -> Self {
        let len = members.len();
        PaneAxis {
            axis,
            members,
            flexes: vec![1.0; len],
        }
    }

    /// Recursively find `old_pane` and split it. Returns true if found.
    fn split(
        &mut self,
        old_pane: &Entity<TerminalView>,
        new_pane: Entity<TerminalView>,
        direction: SplitDirection,
    ) -> bool {
        for (mut idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    if axis.split(old_pane, new_pane.clone(), direction) {
                        return true;
                    }
                }
                Member::Pane(pane) => {
                    if pane.entity_id() == old_pane.entity_id() {
                        if direction.axis() == self.axis {
                            // Same axis: insert adjacent, reset flexes.
                            if direction.increasing() {
                                idx += 1;
                            }
                            self.members.insert(idx, Member::Pane(new_pane));
                            self.flexes = vec![1.0; self.members.len()];
                        } else {
                            // Different axis: create nested axis.
                            *member = Member::new_axis(old_pane.clone(), new_pane, direction);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Remove a pane. Returns:
    /// - `Ok(Some(member))` when the axis collapsed to a single child
    /// - `Ok(None)` when the pane was found and removed but axis still has >1 members
    /// - `Err(())` when the pane was not found
    fn remove(&mut self, pane: &Entity<TerminalView>) -> Result<Option<Member>, ()> {
        let mut found = false;
        let mut remove_idx = None;

        for (idx, member) in self.members.iter_mut().enumerate() {
            match member {
                Member::Axis(axis) => {
                    match axis.remove(pane) {
                        Ok(Some(last)) => {
                            *member = last;
                            found = true;
                            break;
                        }
                        Ok(None) => {
                            found = true;
                            break;
                        }
                        Err(()) => {
                            // Not found in this child, continue
                        }
                    }
                }
                Member::Pane(p) => {
                    if p.entity_id() == pane.entity_id() {
                        found = true;
                        remove_idx = Some(idx);
                        break;
                    }
                }
            }
        }

        if !found {
            return Err(());
        }

        if let Some(idx) = remove_idx {
            self.members.remove(idx);
            self.flexes = vec![1.0; self.members.len()];
        }

        if self.members.len() == 1 {
            Ok(Some(self.members.pop().unwrap()))
        } else {
            Ok(None)
        }
    }

    /// Find adjacent pane to `from` in the given direction.
    fn find_adjacent(
        &self,
        from: &Entity<TerminalView>,
        direction: SplitDirection,
    ) -> Option<Entity<TerminalView>> {
        // If this axis matches the direction's axis, try to find an adjacent sibling
        if direction.axis() == self.axis {
            for (idx, member) in self.members.iter().enumerate() {
                if member.contains(from) {
                    if direction.increasing() {
                        if idx + 1 < self.members.len() {
                            return Some(self.members[idx + 1].first_pane());
                        }
                    } else if idx > 0 {
                        return Some(self.members[idx - 1].last_pane());
                    }
                    return None;
                }
            }
        }

        // Otherwise, recurse into child that contains `from`
        for member in &self.members {
            if member.contains(from) {
                return member.find_adjacent(from, direction);
            }
        }
        None
    }

    fn render(&self, active_id: Option<EntityId>) -> AnyElement {
        // Render each member as an AnyElement
        let children: Vec<AnyElement> = self
            .members
            .iter()
            .map(|member| member.render(active_id))
            .collect();

        // Use our custom element that manually sizes children (like Zed's PaneAxisElement)
        let mut el = element::PaneAxisElement::new(self.axis, self.flexes.clone());
        for child in children {
            el.children.push(child);
        }
        el.into_any_element()
    }
}

// ── Custom Element (ported from Zed) ─────────────────────────────────

mod element {
    use std::mem;

    use gpui::{
        Along, AnyElement, App, Axis, Bounds, Element, GlobalElementId, IntoElement, Pixels, Style,
        Window, px, relative, size,
    };
    use ui::prelude::*;

    const DIVIDER_SIZE: f32 = 1.0;

    pub(super) struct PaneAxisElement {
        axis: Axis,
        flexes: Vec<f32>,
        pub(super) children: Vec<AnyElement>,
    }

    pub(super) struct PaneAxisLayout {
        children: Vec<PaneAxisChildLayout>,
    }

    struct PaneAxisChildLayout {
        bounds: Bounds<Pixels>,
        element: AnyElement,
    }

    impl PaneAxisElement {
        pub fn new(axis: Axis, flexes: Vec<f32>) -> Self {
            PaneAxisElement {
                axis,
                flexes,
                children: Vec::new(),
            }
        }
    }

    impl IntoElement for PaneAxisElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for PaneAxisElement {
        type RequestLayoutState = ();
        type PrepaintState = PaneAxisLayout;

        fn id(&self) -> Option<gpui::ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&gpui::InspectorElementId>,
            window: &mut Window,
            cx: &mut App,
        ) -> (gpui::LayoutId, Self::RequestLayoutState) {
            let style = Style {
                flex_grow: 1.,
                flex_shrink: 1.,
                flex_basis: relative(0.).into(),
                size: size(relative(1.).into(), relative(1.).into()),
                ..Style::default()
            };
            (window.request_layout(style, None, cx), ())
        }

        fn prepaint(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&gpui::InspectorElementId>,
            bounds: Bounds<Pixels>,
            _state: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> PaneAxisLayout {
            let flexes = &self.flexes;
            let len = self.children.len();
            let total_flex: f32 = flexes.iter().sum();

            let divider_count = if len > 1 { len - 1 } else { 0 };
            let total_divider_size = px(DIVIDER_SIZE * divider_count as f32);
            let available = bounds.size.along(self.axis) - total_divider_size;
            let space_per_flex = if total_flex > 0.0 {
                available / total_flex
            } else {
                px(0.)
            };

            let mut origin = bounds.origin;
            let mut layout = PaneAxisLayout {
                children: Vec::with_capacity(len),
            };

            let children: Vec<AnyElement> = mem::take(&mut self.children);
            for (ix, mut child) in children.into_iter().enumerate() {
                if ix > 0 {
                    origin = origin.apply_along(self.axis, |val| val + px(DIVIDER_SIZE));
                }

                let child_flex = flexes.get(ix).copied().unwrap_or(1.0);
                let child_size = bounds
                    .size
                    .apply_along(self.axis, |_| (space_per_flex * child_flex).round());

                let child_bounds = Bounds {
                    origin,
                    size: child_size,
                };

                let available: gpui::Size<gpui::AvailableSpace> = child_size.into();
                child.layout_as_root(available, window, cx);
                child.prepaint_at(origin, window, cx);

                origin = origin.apply_along(self.axis, |val| val + child_size.along(self.axis));

                layout.children.push(PaneAxisChildLayout {
                    bounds: child_bounds,
                    element: child,
                });
            }

            layout
        }

        fn paint(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&gpui::InspectorElementId>,
            bounds: Bounds<Pixels>,
            _state: &mut Self::RequestLayoutState,
            layout: &mut Self::PrepaintState,
            window: &mut Window,
            _cx: &mut App,
        ) {
            let t = theme();

            for child in &mut layout.children {
                child.element.paint(window, _cx);
            }

            for (ix, child) in layout.children.iter().enumerate() {
                if ix < layout.children.len() - 1 {
                    let divider_origin = child
                        .bounds
                        .origin
                        .apply_along(self.axis, |val| val + child.bounds.size.along(self.axis));
                    let divider_size = bounds.size.apply_along(self.axis, |_| px(DIVIDER_SIZE));

                    let divider_bounds = Bounds {
                        origin: divider_origin,
                        size: divider_size,
                    };

                    window.paint_quad(gpui::fill(divider_bounds, t.border_default));
                }
            }
        }
    }
}
