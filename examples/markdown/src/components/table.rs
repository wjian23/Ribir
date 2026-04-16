use std::cell::RefCell;

use ribir::prelude::*;

use super::{MarkdownTableData, MixedInline, TableAlign, TableCell, TableRow};

const CELL_HORIZONTAL_PADDING: f32 = 12.0;
const CELL_VERTICAL_PADDING: f32 = 8.0;
const TABLE_BORDER_WIDTH: f32 = 1.0;
const HEADER_BACKGROUND: u32 = 0xFFF5F5F5;
const BORDER_COLOR: u32 = 0xFFE0E0E0;
const TABLE_MARGIN_BOTTOM: f32 = 16.0;
const EMPTY_TABLE_MIN_HEIGHT: f32 = 28.0;
const WIDTH_EPSILON: f32 = 0.5;

#[derive(Clone, Default)]
struct TableLayout {
  column_widths: Vec<f32>,
  row_heights: Vec<f32>,
}

#[derive(Declare, MultiChild)]
struct MarkdownTable {
  pub columns: usize,
  #[declare(default)]
  pub header_rows: usize,
  #[declare(default)]
  pub alignments: Vec<TableAlign>,
  #[declare(skip, default = RefCell::new(TableLayout::default()))]
  layout: RefCell<TableLayout>,
}

impl Render for MarkdownTable {
  fn measure(&self, clamp: BoxClamp, ctx: &mut MeasureCtx) -> Size {
    let children: Vec<_> = {
      let (_, children) = ctx.split_children();
      children.collect()
    };

    if self.columns == 0 || children.is_empty() {
      *self.layout.borrow_mut() = TableLayout::default();
      return clamp.clamp(Size::new(0.0, EMPTY_TABLE_MIN_HEIGHT));
    }

    let row_count = children.len().div_ceil(self.columns);
    let mut intrinsic_sizes = vec![Size::zero(); children.len()];
    let mut column_widths = vec![0.0f32; self.columns];

    for (index, child) in children.iter().copied().enumerate() {
      let size = ctx.layout_child(child, BoxClamp::UNLIMITED);
      intrinsic_sizes[index] = size;
      let column = index % self.columns;
      column_widths[column] = column_widths[column].max(size.width + CELL_HORIZONTAL_PADDING * 2.0);
    }

    let intrinsic_total_width: f32 = column_widths.iter().sum();
    let target_table_width = if clamp.max.width.is_finite() {
      intrinsic_total_width.clamp(clamp.min.width, clamp.max.width)
    } else {
      intrinsic_total_width.max(clamp.min.width)
    };
    let resolved_widths = resolve_column_widths(&column_widths, target_table_width);

    let mut row_heights = vec![EMPTY_TABLE_MIN_HEIGHT; row_count];
    for (index, child) in children.iter().copied().enumerate() {
      let column = index % self.columns;
      let row = index / self.columns;
      let available_content_width =
        (resolved_widths[column] - CELL_HORIZONTAL_PADDING * 2.0).max(0.0);
      let size = if intrinsic_sizes[index].width <= available_content_width + WIDTH_EPSILON {
        intrinsic_sizes[index]
      } else {
        ctx.layout_child(child, BoxClamp::max_width(available_content_width))
      };
      row_heights[row] = row_heights[row].max(size.height + CELL_VERTICAL_PADDING * 2.0);
    }

    *self.layout.borrow_mut() =
      TableLayout { column_widths: resolved_widths.clone(), row_heights: row_heights.clone() };

    let total_width: f32 = resolved_widths.iter().sum();
    let total_height: f32 = row_heights.iter().sum();
    clamp.clamp(Size::new(total_width, total_height))
  }

  fn place_children(&self, _size: Size, ctx: &mut PlaceCtx) {
    let layout = self.layout.borrow().clone();
    if layout.column_widths.is_empty() || layout.row_heights.is_empty() {
      return;
    }

    let (ctx, children) = ctx.split_children();
    let mut x = 0.0;
    let mut y = 0.0;

    for (index, child) in children.enumerate() {
      let column = index % self.columns;
      let row = index / self.columns;
      let column_width = layout.column_widths[column];
      let row_height = layout.row_heights[row];
      let child_size = ctx.widget_box_size(child).unwrap_or_default();
      let align = self
        .alignments
        .get(column)
        .cloned()
        .unwrap_or_default();
      let horizontal = align.align_offset(child_size.width, column_width);
      let vertical = (row_height - child_size.height).max(0.0) * 0.5;
      ctx.update_position(child, Point::new(x + horizontal, y + vertical));

      x += column_width;
      if column + 1 == self.columns {
        x = 0.0;
        y += row_height;
      }
    }
  }

  fn paint(&self, ctx: &mut PaintingCtx) {
    let layout = self.layout.borrow().clone();
    let Some(size) = ctx.box_size() else {
      return;
    };
    if layout.column_widths.is_empty() || layout.row_heights.is_empty() || size.is_empty() {
      return;
    }

    let painter = ctx.painter();
    let old_fill = painter.fill_brush().clone();

    let mut y = 0.0;
    for row in 0..self.header_rows.min(layout.row_heights.len()) {
      let row_height = layout.row_heights[row];
      let mut x = 0.0;
      for column_width in &layout.column_widths {
        painter
          .set_fill_brush(Color::from_u32(HEADER_BACKGROUND))
          .rect(&Rect::new(Point::new(x, y), Size::new(*column_width, row_height)), true)
          .fill();
        x += *column_width;
      }
      y += row_height;
    }

    let border_color = Color::from_u32(BORDER_COLOR);
    painter.set_fill_brush(border_color);

    let total_width: f32 = layout.column_widths.iter().sum();
    let total_height: f32 = layout.row_heights.iter().sum();

    painter
      .rect(&Rect::new(Point::new(0.0, 0.0), Size::new(total_width, TABLE_BORDER_WIDTH)), true)
      .fill();
    painter
      .rect(
        &Rect::new(
          Point::new(0.0, (total_height - TABLE_BORDER_WIDTH).max(0.0)),
          Size::new(total_width, TABLE_BORDER_WIDTH),
        ),
        true,
      )
      .fill();
    painter
      .rect(&Rect::new(Point::new(0.0, 0.0), Size::new(TABLE_BORDER_WIDTH, total_height)), true)
      .fill();
    painter
      .rect(
        &Rect::new(
          Point::new((total_width - TABLE_BORDER_WIDTH).max(0.0), 0.0),
          Size::new(TABLE_BORDER_WIDTH, total_height),
        ),
        true,
      )
      .fill();

    let mut x = 0.0;
    for column_width in layout
      .column_widths
      .iter()
      .take(layout.column_widths.len().saturating_sub(1))
    {
      x += *column_width;
      painter
        .rect(&Rect::new(Point::new(x, 0.0), Size::new(TABLE_BORDER_WIDTH, total_height)), true)
        .fill();
    }

    let mut y = 0.0;
    for row_height in layout
      .row_heights
      .iter()
      .take(layout.row_heights.len().saturating_sub(1))
    {
      y += *row_height;
      painter
        .rect(&Rect::new(Point::new(0.0, y), Size::new(total_width, TABLE_BORDER_WIDTH)), true)
        .fill();
    }

    painter.set_fill_brush(old_fill);
  }

  #[cfg(feature = "debug")]
  fn debug_name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("markdown_table")
  }
}

impl TableAlign {
  fn align_offset(&self, child_width: f32, column_width: f32) -> f32 {
    let content_start = CELL_HORIZONTAL_PADDING;
    let content_width = (column_width - CELL_HORIZONTAL_PADDING * 2.0).max(0.0);
    match self {
      TableAlign::Start => content_start,
      TableAlign::Center => content_start + (content_width - child_width).max(0.0) * 0.5,
      TableAlign::End => content_start + (content_width - child_width).max(0.0),
    }
  }
}

pub fn render_table(table: MarkdownTableData) -> Widget<'static> {
  let columns = table.columns();
  if columns == 0 {
    return fn_widget! {
      @Container {
        margin: EdgeInsets::only_bottom(TABLE_MARGIN_BOTTOM),
        @Text { text: "" }
      }
    }
    .into_widget();
  }

  let header_rows = usize::from(!table.header.cells.is_empty());
  let alignments = normalize_alignments(&table.alignments, columns);
  let cells = normalized_rows(&table)
    .into_iter()
    .flat_map(|row| row.cells.into_iter())
    .map(render_cell)
    .collect::<Vec<_>>();

  fn_widget! {
    @MarkdownTable {
      columns,
      header_rows,
      alignments,
      margin: EdgeInsets::new(0.0, 0.0, 0.0, TABLE_MARGIN_BOTTOM),
      @ { cells }
    }
  }
  .into_widget()
}

fn render_cell(cell: TableCell) -> Widget<'static> {
  if cell.inlines.is_empty() {
    fn_widget! { @Void {} }.into_widget()
  } else {
    MixedInline::new(cell.inlines).into_widget()
  }
}

fn normalize_alignments(alignments: &[TableAlign], columns: usize) -> Vec<TableAlign> {
  let mut normalized = vec![TableAlign::Start; columns];
  for (index, align) in alignments
    .iter()
    .take(columns)
    .cloned()
    .enumerate()
  {
    normalized[index] = align;
  }
  normalized
}

fn normalized_rows(table: &MarkdownTableData) -> Vec<TableRow> {
  let columns = table.columns();
  let mut rows = Vec::new();
  if !table.header.cells.is_empty() {
    rows.push(normalize_row(&table.header, columns));
  }
  rows.extend(
    table
      .rows
      .iter()
      .map(|row| normalize_row(row, columns)),
  );
  rows
}

fn normalize_row(row: &TableRow, columns: usize) -> TableRow {
  let mut cells = row.cells.clone();
  cells.resize_with(columns, TableCell::default);
  TableRow { cells }
}

fn resolve_column_widths(intrinsic: &[f32], target_total: f32) -> Vec<f32> {
  if intrinsic.is_empty() {
    return Vec::new();
  }

  let total: f32 = intrinsic.iter().sum();
  if (total - target_total).abs() <= WIDTH_EPSILON {
    return intrinsic.to_vec();
  }

  if target_total > total {
    let extra = (target_total - total) / intrinsic.len() as f32;
    return intrinsic
      .iter()
      .map(|width| width + extra)
      .collect();
  }

  if target_total <= 0.0 {
    return vec![0.0; intrinsic.len()];
  }

  let mut widths = intrinsic.to_vec();
  let mut flexible = vec![true; intrinsic.len()];
  let mut remaining_width = target_total;
  let mut remaining_columns = intrinsic.len();

  loop {
    let share = remaining_width / remaining_columns as f32;
    let mut fixed_any = false;

    for (index, width) in intrinsic.iter().copied().enumerate() {
      if flexible[index] && width <= share + WIDTH_EPSILON {
        flexible[index] = false;
        remaining_width -= width;
        remaining_columns -= 1;
        fixed_any = true;
      }
    }

    if !fixed_any || remaining_columns == 0 {
      break;
    }
  }

  let share = (remaining_width / remaining_columns.max(1) as f32).max(0.0);
  for (index, width) in widths.iter_mut().enumerate() {
    if flexible[index] {
      *width = share;
    }
  }

  widths
}

impl MarkdownTableData {
  pub fn columns(&self) -> usize {
    let header_columns = self.header.cells.len();
    let body_columns = self
      .rows
      .iter()
      .map(|row| row.cells.len())
      .max()
      .unwrap_or(0);
    header_columns
      .max(body_columns)
      .max(self.alignments.len())
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use ribir::core::{reset_test_env, test_helper::TestWindow};

  use super::*;
  use crate::components::InlineNode;

  fn assert_close(actual: f32, expected: f32, label: &str) {
    assert!((actual - expected).abs() <= 1.0, "{label} expected {expected}, got {actual}");
  }

  #[test]
  fn resolve_column_widths_preserves_narrow_columns_when_shrinking() {
    let widths = resolve_column_widths(&[64.0, 184.0], 190.0);
    assert_close(widths[0], 64.0, "narrow column");
    assert_close(widths[1], 126.0, "wide column");
  }

  #[test]
  fn table_only_remeasures_compressed_cells() {
    reset_test_env!();

    #[derive(Clone, Declare, Default)]
    struct MeasureCounterBox {
      pub size: Size,
      #[declare(default = Rc::new(Cell::new(0)))]
      pub hits: Rc<Cell<usize>>,
    }

    impl Render for MeasureCounterBox {
      fn measure(&self, clamp: BoxClamp, _ctx: &mut MeasureCtx) -> Size {
        self.hits.set(self.hits.get() + 1);
        clamp.clamp(self.size)
      }

      fn size_affected_by_child(&self) -> bool { false }
    }

    let narrow_hits = Rc::new(Cell::new(0));
    let wide_hits = Rc::new(Cell::new(0));
    let narrow_hits_for_widget = narrow_hits.clone();
    let wide_hits_for_widget = wide_hits.clone();

    let wnd = TestWindow::from_widget(fn_widget! {
      @MarkdownTable {
        columns: 2usize,
        clamp: BoxClamp::max_width(190.0),
        @MeasureCounterBox {
          size: Size::new(40.0, 20.0),
          hits: narrow_hits_for_widget.clone(),
        }
        @MeasureCounterBox {
          size: Size::new(160.0, 20.0),
          hits: wide_hits_for_widget.clone(),
        }
      }
    });
    wnd.draw_frame();

    assert_eq!(narrow_hits.get(), 1);
    assert_eq!(wide_hits.get(), 2);
  }

  #[test]
  fn table_places_cells_with_row_height_centering() {
    reset_test_env!();

    #[derive(Clone, Declare, Default)]
    struct FixedSizeBox {
      pub size: Size,
    }

    impl Render for FixedSizeBox {
      fn measure(&self, clamp: BoxClamp, _ctx: &mut MeasureCtx) -> Size { clamp.clamp(self.size) }

      fn size_affected_by_child(&self) -> bool { false }
    }

    let wnd = TestWindow::from_widget(fn_widget! {
      @MarkdownTable {
        columns: 2usize,
        @FixedSizeBox { size: Size::new(40.0, 20.0) }
        @FixedSizeBox { size: Size::new(80.0, 40.0) }
      }
    });
    wnd.draw_frame();

    let left = wnd.layout_info_by_path(&[0, 0]).unwrap();
    let right = wnd.layout_info_by_path(&[0, 1]).unwrap();

    assert_close(left.pos.x, 12.0, "left x");
    assert_close(left.pos.y, 18.0, "left vertical center");
    assert_close(right.pos.x, 76.0, "right x");
    assert_close(right.pos.y, 8.0, "right top padding");
  }

  #[test]
  fn normalized_rows_fill_missing_cells() {
    let rows = normalized_rows(&MarkdownTableData {
      alignments: vec![],
      header: TableRow {
        cells: vec![TableCell {
          inlines: vec![InlineNode::Text { content: "h".into(), style: Default::default() }],
        }],
      },
      rows: vec![TableRow { cells: vec![] }],
    });

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].cells.len(), 1);
    assert_eq!(rows[1].cells.len(), 1);
  }
}
