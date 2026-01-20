use ribir_geom::Size;

use super::{WidgetCtx, WidgetCtxImpl};
use crate::{
  context::VisualCtx,
  prelude::ProviderCtx,
  widget::{BoxClamp, VisualBox, WidgetTree},
  widget_tree::{AnchorX, AnchorY, WidgetId},
};

/// A place to compute the render object's layout.
///
/// Rather than holding children directly, `Layout` perform layout across
/// `LayoutCtx`. `LayoutCtx` provide method to perform child layout and also
/// provides methods to update descendants position.
pub struct LayoutCtx<'a> {
  pub(crate) id: WidgetId,
  /// The widget tree of the window, not borrow it from `wnd` is because a
  /// `LayoutCtx` always in a mutable borrow.
  pub(crate) tree: &'a mut WidgetTree,
  pub(crate) provider_ctx: ProviderCtx,
  pub(crate) laid_out_queue: &'a mut Vec<WidgetId>,
}

impl<'a> WidgetCtxImpl for LayoutCtx<'a> {
  #[inline]
  fn id(&self) -> WidgetId { self.id }

  #[inline]
  fn tree(&self) -> &WidgetTree { self.tree }
}

impl<'a> LayoutCtx<'a> {
  pub(crate) fn new(
    id: WidgetId, tree: &'a mut WidgetTree, laid_out_queue: &'a mut Vec<WidgetId>,
  ) -> Self {
    let provider_ctx = if let Some(p) = id.parent(tree) {
      ProviderCtx::collect_from(p, tree)
    } else {
      ProviderCtx::default()
    };
    Self { id, tree, provider_ctx, laid_out_queue }
  }

  /// Measure the widget of the context and return its size.
  pub(crate) fn measure(&mut self, clamp: BoxClamp) -> Size {
    self
      .get_calculated_size(self.id, clamp)
      .unwrap_or_else(|| {
        // Safety: the `tree` just use to get the widget of `id`, and `tree2` not drop
        // or modify it during measure.
        let tree2 = unsafe { &*(self.tree as *mut WidgetTree) };

        let id = self.id();

        debug_assert!(clamp.min.is_finite());
        let size = id.assert_get(tree2).measure(clamp, self);
        debug_assert!(size.is_finite());
        let info = self.tree.store.layout_info_or_default(id);
        info.clamp = clamp;
        info.size = Some(size);

        size
      })
  }

  /// Layout the widget of the context (position its children).
  pub(crate) fn layout(&mut self, size: Size) {
    // Safety: the `tree` just use to get the widget of `id`, and `tree2` not drop
    // or modify it during layout.
    let tree2 = unsafe { &*(self.tree as *mut WidgetTree) };
    let id = self.id();

    id.assert_get(tree2).layout(size, self);

    {
      VisualCtx::from_layout_ctx(self).update_visual_box();
    }

    self.provider_ctx.pop_providers_for(id);
    self.laid_out_queue.push(id);
  }

  /// Measure the `child` and return its size.
  pub fn measure_child(&mut self, child: WidgetId, clamp: BoxClamp) -> Size {
    self
      .get_calculated_size(child, clamp)
      .unwrap_or_else(|| {
        // The position needs to be reset, as some parent render widgets may not have
        // set the position.
        self.update_anchor(child, AnchorX::default(), AnchorY::default());

        let id = std::mem::replace(&mut self.id, child);
        let size = self.measure(clamp);
        self.id = id;

        size
      })
  }

  /// Layout the `child` (position its children).
  pub fn layout_child(&mut self, child: WidgetId) {
    let size = self
      .tree
      .store
      .layout_info(child)
      .and_then(|info| info.size)
      .expect("Child must be measured before layout");

    let id = std::mem::replace(&mut self.id, child);
    self.layout(size);
    self.id = id;
  }

  /// Adjust the position of the widget where it should be placed relative to
  /// its parent.
  #[inline]
  pub fn update_anchor(&mut self, child: WidgetId, pos_x: AnchorX, pos_y: AnchorY) {
    let info = self.tree.store.layout_info_or_default(child);
    info.pos_x = pos_x;
    info.pos_y = pos_y;
  }

  #[inline]
  pub fn update_anchor_x(&mut self, child: WidgetId, pos_x: AnchorX) {
    self
      .tree
      .store
      .layout_info_or_default(child)
      .pos_x = pos_x;
  }

  #[inline]
  pub fn update_anchor_y(&mut self, child: WidgetId, pos_y: AnchorY) {
    self
      .tree
      .store
      .layout_info_or_default(child)
      .pos_y = pos_y;
  }

  /// Return the stored anchor rules of the widget.
  /// Returns (AnchorX, AnchorY) so caller can apply offset methods without
  /// calculation.
  #[inline]
  pub fn anchor(&mut self, child: WidgetId) -> Option<(AnchorX, AnchorY)> {
    self
      .tree
      .store
      .layout_info(child)
      .map(|info| (info.pos_x.clone(), info.pos_y.clone()))
  }

  /// Adjust the size of the layout widget. Use this method to directly modify
  /// the size of a widget. In most cases, it is unnecessary to call this
  /// method; using clamp to constrain the child size is typically sufficient.
  /// Only use this method if you are certain of its effects.
  #[inline]
  pub fn update_size(&mut self, child: WidgetId, size: Size) {
    self.tree.store.layout_info_or_default(child).size = Some(size);
  }

  /// Split a children iterator from the context, returning a tuple of `&mut
  /// LayoutCtx` and the iterator of the children.
  pub fn split_children(&mut self) -> (&mut Self, impl Iterator<Item = WidgetId> + '_) {
    // Safety: The widget tree structure is immutable during the layout phase, so we
    // can safely split an iterator of children from the layout.
    let tree = unsafe { &*(self.tree as *mut WidgetTree) };
    let id = self.id;
    (self, id.children(tree))
  }

  /// Quick method to measure the single child and return its size.
  ///
  /// # Panic
  /// panic if there is more than one child.
  pub fn measure_single_child(&mut self, clamp: BoxClamp) -> Option<Size> {
    self
      .single_child()
      .map(|child| self.measure_child(child, clamp))
  }

  /// Quick method to measure the single child and return its size.
  ///
  /// # Panic
  /// panic if there is not exactly one child.
  pub fn assert_measure_single_child(&mut self, clamp: BoxClamp) -> Size {
    let child = self.assert_single_child();
    self.measure_child(child, clamp)
  }

  /// Quick method to layout the single child.
  ///
  /// # Panic
  /// panic if there is not exactly one child.
  pub fn layout_single_child(&mut self) {
    if let Some(child) = self.single_child() {
      self.layout_child(child);
    }
  }

  /// Clear the child layout information, so the `child` will be force layout
  /// when call `measure_child` even if it has layout cache
  /// information with same input.
  #[inline]
  pub fn force_child_relayout(&mut self, child: WidgetId) -> bool {
    assert_eq!(child.parent(self.tree), Some(self.id));
    self.tree.store.force_layout(child).is_some()
  }

  fn get_calculated_size(&self, child: WidgetId, clamp: BoxClamp) -> Option<Size> {
    let info = self.tree.store.layout_info(child)?;
    if info.clamp == clamp { info.size } else { None }
  }

  pub(crate) fn visual_box(&mut self, id: WidgetId) -> VisualBox {
    let info = self.tree.store.layout_info_or_default(id);
    info.visual_box
  }

  pub(crate) fn update_visual_box(&mut self) -> VisualBox {
    VisualCtx::from_layout_ctx(self).update_visual_box()
  }
}

impl<'w> AsRef<ProviderCtx> for LayoutCtx<'w> {
  fn as_ref(&self) -> &ProviderCtx { &self.provider_ctx }
}

impl<'w> AsMut<ProviderCtx> for LayoutCtx<'w> {
  fn as_mut(&mut self) -> &mut ProviderCtx { &mut self.provider_ctx }
}
