//! PageMD owns native view state, revisions and source-aware selection.
struct Keybindings;
impl Global for Keybindings {}

use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

pub use gpui_kit::base::text::{
    RangeHighlight, RangeHighlightError, SelectionFormat,
};
use gpui_kit::{
    base::{TextSelectionEvent, TextSelectionHandle, TextSelectionRun},
    *,
};
use pagemd_parser::ParsedDocument;

use crate::{
    model::{self, Block, Index},
    RenderedText,
};

#[derive(Default)]
pub(crate) struct Frame {
    pub runs: Vec<(Range<usize>, TextSelectionRun)>,
    pub objects: Vec<(Range<usize>, Bounds<Pixels>)>,
    pub selected: Option<Range<usize>>,
    pub bounds: Bounds<Pixels>,
}
pub struct MarkdownViewState {
    pub(crate) document: ParsedDocument,
    pub(crate) blocks: Arc<Vec<Block>>,
    index: Arc<Index>,
    revision: usize,
    request: usize,
    requested_source: String,
    parse_task: Option<Task<()>>,
    pub(crate) frame: Rc<RefCell<Frame>>,
    pub(crate) selection: TextSelectionHandle,
    pub(crate) all_selected: bool,
    pub(crate) selectable: bool,
    pub(crate) format: SelectionFormat,
    pub(crate) focus: FocusHandle,
    pub(crate) highlights: Vec<RangeHighlight>,
    pub(crate) reveal: Option<Range<usize>>,
    pub(crate) scroll: ScrollHandle,
    pub(crate) layout_revision: usize,
    owner: EntityId,
}
impl MarkdownViewState {
    pub fn from_document(
        document: &ParsedDocument,
        cx: &mut Context<Self>,
    ) -> Self {
        if !cx.has_global::<Keybindings>() {
            cx.bind_keys([
                KeyBinding::new(
                    "cmd-c",
                    gpui_kit::component::input::Copy,
                    Some("PageMdView"),
                ),
                KeyBinding::new(
                    "ctrl-c",
                    gpui_kit::component::input::Copy,
                    Some("PageMdView"),
                ),
                KeyBinding::new(
                    "cmd-a",
                    gpui_kit::component::input::SelectAll,
                    Some("PageMdView"),
                ),
                KeyBinding::new(
                    "ctrl-a",
                    gpui_kit::component::input::SelectAll,
                    Some("PageMdView"),
                ),
            ]);
            cx.set_global(Keybindings);
        }
        let (blocks, index) = model::build(document);
        let selection = TextSelectionHandle::new(document.source(), cx);
        let view = cx.entity().downgrade();
        let for_copy = view.clone();
        selection.copy_with(
            move |cx| {
                for_copy
                    .upgrade()
                    .map(|v| v.read(cx).selected_text())
                    .unwrap_or_default()
            },
            cx,
        );
        let for_clear = view.clone();
        selection.clear_with(
            move |cx| {
                let _ = for_clear.update(cx, |v, cx| v.clear_selection(cx));
            },
            cx,
        );
        let for_focus = view.clone();
        selection.focus_with(
            move |window, cx| {
                if let Some(view) = for_focus.upgrade() {
                    let focus = view.read(cx).focus.clone();
                    focus.focus(window, cx);
                }
            },
            cx,
        );
        selection
            .subscribe(
                move |event, cx| {
                    let _ = view.update(cx, |v, cx| {
                        if matches!(
                            event,
                            TextSelectionEvent::SelectionChanged(_)
                                | TextSelectionEvent::Cleared
                        ) {
                            v.all_selected = false;
                            v.frame.borrow_mut().selected = None;
                            v.refresh_selection(cx);
                        }
                        cx.notify();
                    });
                },
                cx,
            )
            .detach();
        Self {
            document: document.clone(),
            blocks: Arc::new(blocks),
            index,
            revision: 0,
            request: 0,
            requested_source: document.source().to_owned(),
            parse_task: None,
            frame: Rc::default(),
            selection,
            all_selected: false,
            selectable: true,
            format: SelectionFormat::Source,
            focus: cx.focus_handle(),
            highlights: vec![],
            reveal: None,
            scroll: ScrollHandle::new(),
            layout_revision: 0,
            owner: cx.entity_id(),
        }
    }
    pub fn set_text(&mut self, source: &str, cx: &mut Context<Self>) {
        if source == self.requested_source {
            return;
        }
        self.request += 1;
        self.requested_source = source.to_owned();
        self.parse_task = None;
        let options = self.document.options().clone();
        if source.len() <= 4096 {
            if let Ok(doc) = pagemd_parser::parse(source, &options) {
                let (blocks, index) = model::build(&doc);
                self.install(doc, blocks, index, cx);
            }
        } else {
            let source = source.to_owned();
            let request = self.request;
            let executor = cx.background_executor().clone();
            self.parse_task = Some(cx.spawn(async move |view, cx| {
                let result = executor
                    .spawn(async move {
                        let doc =
                            pagemd_parser::parse(&source, &options).ok()?;
                        let (blocks, index) = model::build(&doc);
                        Some((doc, blocks, index))
                    })
                    .await;
                let _ = view.update(cx, |v, cx| {
                    if v.request == request {
                        if let Some((doc, blocks, index)) = result {
                            v.install(doc, blocks, index, cx);
                        }
                    }
                });
            }));
        }
    }
    fn install(
        &mut self,
        document: ParsedDocument,
        blocks: Vec<Block>,
        index: Arc<Index>,
        cx: &mut Context<Self>,
    ) {
        let previous_selection = document
            .source()
            .starts_with(self.document.source())
            .then(|| self.selected_source_range())
            .flatten();
        self.document = document;
        self.blocks = Arc::new(blocks);
        self.index = index;
        self.revision += 1;
        self.highlights.clear();
        self.reveal = None;
        self.all_selected = false;
        self.frame.borrow_mut().selected = previous_selection
            .and_then(|range| self.rendered_text().range_for_source(range));
        self.selection.set_local_selection(false, cx);
        cx.notify();
    }
    pub fn push_str(&mut self, tail: &str, cx: &mut Context<Self>) {
        let source = format!("{}{tail}", self.requested_source);
        self.set_text(&source, cx);
    }
    pub fn rendered_text(&self) -> RenderedText {
        RenderedText::new(self.owner, self.revision, self.index.clone())
    }
    pub fn selected_source_range(&self) -> Option<Range<usize>> {
        if self.all_selected {
            Some(0..self.document.source().len())
        } else {
            self.rendered_text()
                .source_for_range(self.frame.borrow().selected.clone()?)
        }
    }
    pub fn selected_text(&self) -> String {
        let rendered = self.rendered_text();
        if self.all_selected {
            return match self.format {
                SelectionFormat::Source
                | SelectionFormat::SourceAcrossStyles => {
                    self.document.source().to_owned()
                }
                SelectionFormat::Plain => rendered.as_str().to_owned(),
            };
        }
        let Some(range) = self.frame.borrow().selected.clone() else {
            return String::new();
        };
        let format = match self.format {
            SelectionFormat::SourceAcrossStyles
                if model::selection_has_one_style(&self.blocks, &range) =>
            {
                SelectionFormat::Plain
            }
            SelectionFormat::SourceAcrossStyles => SelectionFormat::Source,
            format => format,
        };
        match format {
            SelectionFormat::Plain => {
                rendered.as_str().get(range).unwrap_or_default().to_owned()
            }
            SelectionFormat::Source | SelectionFormat::SourceAcrossStyles => {
                self.selected_source_range()
                    .and_then(|r| self.document.source().get(r))
                    .unwrap_or_default()
                    .to_owned()
            }
        }
    }
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.all_selected = true;
        self.frame.borrow_mut().selected = Some(0..self.rendered_text().len());
        self.selection.set_local_selection(true, cx);
        cx.notify();
    }
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.all_selected = false;
        self.frame.borrow_mut().selected = None;
        self.selection.set_local_selection(false, cx);
        cx.notify();
    }
    pub fn set_selection_format(
        &mut self,
        format: SelectionFormat,
        cx: &mut Context<Self>,
    ) {
        self.format = format;
        cx.notify();
    }
    pub fn selectable(mut self, value: bool) -> Self {
        self.selectable = value;
        self
    }
    pub fn selection_format(mut self, format: SelectionFormat) -> Self {
        self.format = format;
        self
    }
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.frame.borrow().bounds
    }
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }
    pub fn invalidate_inline_layout(&mut self, cx: &mut Context<Self>) {
        self.layout_revision += 1;
        cx.notify();
    }
    pub fn set_range_highlights(
        &mut self,
        highlights: Vec<RangeHighlight>,
        cx: &mut Context<Self>,
    ) -> Result<(), RangeHighlightError> {
        let text = self.rendered_text();
        for (i, highlight) in highlights.iter().enumerate() {
            if text.as_str().get(highlight.range()).is_none() {
                return Err(RangeHighlightError::InvalidRange(i));
            }
        }
        if self.highlights != highlights {
            self.highlights = highlights;
            cx.notify();
        }
        Ok(())
    }
    pub fn clear_range_highlights(&mut self, cx: &mut Context<Self>) {
        if !self.highlights.is_empty() {
            self.highlights.clear();
            cx.notify();
        }
    }
    pub fn reveal_range(
        &mut self,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), RangeHighlightError> {
        if self.rendered_text().as_str().get(range.clone()).is_none() {
            return Err(RangeHighlightError::InvalidRange(0));
        }
        self.reveal = Some(range);
        cx.notify();
        Ok(())
    }
    pub(crate) fn refresh_selection(&mut self, cx: &mut App) {
        if self.all_selected {
            return;
        }
        let runs: Vec<_> = self
            .frame
            .borrow()
            .runs
            .iter()
            .map(|(_, run)| run.clone())
            .collect();
        let projection = self.selection.update_runs(&runs, cx);
        // A passive selection remains anchored to rendered/source bytes during
        // reflow and streaming. A new gesture event clears it before projecting.
        if self.frame.borrow().selected.is_some()
            && self
                .selection
                .snapshot(cx)
                .is_some_and(|s| !s.is_selecting())
        {
            return;
        }
        let frame = self.frame.borrow();
        let mut result: Option<Range<usize>> = None;
        for ((offset, _), range) in frame
            .runs
            .iter()
            .zip(projection.ranges())
            .filter_map(|(run, range)| range.as_ref().map(|r| (run, r)))
        {
            let range = offset.start + range.start..offset.start + range.end;
            result = Some(match result {
                Some(previous) => {
                    previous.start.min(range.start)..previous.end.max(range.end)
                }
                None => range,
            });
        }
        if let Some(snapshot) = self.selection.snapshot(cx) {
            use gpui_kit::base::TextSelectionCoverage;
            for (range, bounds) in &frame.objects {
                let selected = match snapshot.coverage() {
                    TextSelectionCoverage::Full => true,
                    _ => snapshot.window_points().is_some_and(|p| {
                        let (a, b) = if p.anchor().y < p.cursor().y {
                            (p.anchor(), p.cursor())
                        } else {
                            (p.cursor(), p.anchor())
                        };
                        let center = bounds.center();
                        center.y >= a.y
                            && center.y <= b.y + px(1.)
                            && (center.y > a.y + px(1.) || center.x >= a.x)
                            && (center.y < b.y - px(1.) || center.x <= b.x)
                    }),
                };
                if selected {
                    result = Some(match result {
                        Some(previous) => {
                            previous.start.min(range.start)
                                ..previous.end.max(range.end)
                        }
                        None => range.clone(),
                    });
                }
            }
        }
        drop(frame);
        self.frame.borrow_mut().selected = result;
    }
}
