//! Mermaid figures keep their title outside the exported image.
use bezel_mermaid::{diagrams::Model, Drawing, Error, Host, Options, Parsed};

pub struct Figure {
    pub drawing: Drawing,
    pub caption: Option<String>,
}

fn prepare(
    source: &str,
    options: &Options,
) -> Result<(Parsed, Option<String>), Error> {
    let mut parsed = bezel_mermaid::parse(source, options)?;
    // Take only the whole-diagram title from the semantic model. Node labels,
    // subgraph headings, axis labels and accessibility metadata stay intact.
    let title = match &mut parsed.model {
        Model::GitGraph(m) => m.titles.title.take(),
        Model::Gantt(m) => m.titles.title.take(),
        Model::Info(m) => m.titles.title.take(),
        Model::Flowchart(m) => m.titles.title.take(),
        Model::XyChart(m) => m.titles.title.take(),
        Model::Quadrant(m) => m.titles.title.take(),
        Model::Sankey(m) => m.titles.title.take(),
        Model::Treemap(m) => m.titles.title.take(),
        Model::Radar(m) => m.titles.title.take(),
        Model::Pie(m) => m.titles.title.take(),
        Model::Mindmap(m) => m.title.take(),
        Model::Packet(m) => m.titles.title.take(),
        Model::TreeView(m) => m.titles.title.take(),
        Model::Ishikawa(m) => m.title.take(),
        Model::Railroad(m) => m.titles.title.take(),
        Model::Venn(m) => m.title.take(),
        Model::Journey(m) => m.titles.title.take(),
        Model::Kanban(m) => m.title.take(),
        Model::Timeline(m) => m.titles.title.take(),
        Model::Class(m) => m.titles.title.take(),
        Model::State(m) => m.titles.title.take(),
        Model::Er(m) => m.titles.title.take(),
        Model::Requirement(m) => m.titles.title.take(),
        Model::Sequence(m) => m.titles.title.take(),
        Model::Cynefin(m) => m.titles.title.take(),
        Model::Wardley(m) => m.titles.title.take(),
        Model::EventModeling(m) => m.titles.title.take(),
        Model::Architecture(m) => m.titles.title.take(),
        Model::Block(m) => m.title.take(),
        Model::C4(m) => m.titles.title.take(),
        Model::Agentflow(m) => m.store.titles.title.take(),
        Model::Usecase(m) => m.title.take(),
        Model::Swimlane(m) => m.0.titles.title.take(),
    };
    Ok((
        parsed,
        title.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()),
    ))
}

/// Read a diagram's title without laying it out or modifying its source.
pub fn caption(source: &str) -> Result<Option<String>, Error> {
    Ok(prepare(source, &Options::default())?.1)
}

/// Render a title-free SVG and return its title separately as a figure caption.
/// Both frontmatter and diagram-specific `title` statements are supported.
pub fn render(
    source: &str,
    options: &Options,
    host: &impl Host,
) -> Result<Figure, Error> {
    let (parsed, caption) = prepare(source, options)?;
    Ok(Figure {
        drawing: bezel_mermaid::render_parsed(&parsed, options, host)?,
        caption,
    })
}
