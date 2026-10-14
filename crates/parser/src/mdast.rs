//! PageMD syntax tree data types. Parsing is owned by PageMD, using
//! pulldown-cmark events; these types contain no alternate parser implementation.
use crate::unist::Position;
pub type Stop = (usize, usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum ReferenceKind {
    Shortcut,
    Collapsed,
    Full,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum AlignKind {
    Left,
    Right,
    Center,
    None,
}

macro_rules! structures {
    ($($name:ident { $($field:ident : $ty:ty),* $(,)? })*) => { $(
        #[derive(Clone, Debug, Eq, PartialEq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name { pub position: Option<Position>, $(pub $field: $ty,)* }
    )* };
}
structures! {
    Root { children: Vec<Node> }
    Paragraph { children: Vec<Node> }
    Blockquote { children: Vec<Node> }
    Emphasis { children: Vec<Node> }
    Strong { children: Vec<Node> }
    Delete { children: Vec<Node> }
    TableRow { children: Vec<Node> }
    TableCell { children: Vec<Node> }
    Heading { children: Vec<Node>, depth: u8 }
    List { children: Vec<Node>, ordered: bool, start: Option<u32>, spread: bool }
    ListItem { children: Vec<Node>, checked: Option<bool>, spread: bool }
    Table { children: Vec<Node>, align: Vec<AlignKind> }
    FootnoteDefinition { children: Vec<Node>, identifier: String, label: Option<String> }
    Link { children: Vec<Node>, url: String, title: Option<String> }
    LinkReference { children: Vec<Node>, identifier: String, label: Option<String>, reference_kind: ReferenceKind }
    Image { alt: String, url: String, title: Option<String> }
    ImageReference { alt: String, identifier: String, label: Option<String>, reference_kind: ReferenceKind }
    Definition { identifier: String, label: Option<String>, url: String, title: Option<String> }
    FootnoteReference { identifier: String, label: Option<String> }
    Code { value: String, lang: Option<String>, meta: Option<String> }
    Math { value: String, meta: Option<String> }
    Text { value: String }
    InlineCode { value: String }
    InlineMath { value: String }
    Html { value: String }
    Yaml { value: String }
    Toml { value: String }
    ThematicBreak {}
    Break {}
    MdxjsEsm { value: String, stops: Vec<Stop> }
    MdxTextExpression { value: String, stops: Vec<Stop> }
    MdxFlowExpression { value: String, stops: Vec<Stop> }
    MdxJsxTextElement { children: Vec<Node>, name: Option<String>, attributes: Vec<AttributeContent> }
    MdxJsxFlowElement { children: Vec<Node>, name: Option<String>, attributes: Vec<AttributeContent> }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AttributeContent {
    Expression(MdxJsxExpressionAttribute),
    Property(MdxJsxAttribute),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AttributeValue {
    Literal(String),
    Expression(AttributeValueExpression),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeValueExpression {
    pub value: String,
    pub stops: Vec<Stop>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MdxJsxExpressionAttribute {
    pub value: String,
    pub stops: Vec<Stop>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MdxJsxAttribute {
    pub name: String,
    pub value: Option<AttributeValue>,
}

macro_rules! syntax_tree {
    (parents: $($parent:ident),*; leaves: $($leaf:ident),*;) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum Node { $($parent($parent),)* $($leaf($leaf),)* }
        impl Node {
            pub fn children(&self) -> Option<&Vec<Node>> { match self { $(Self::$parent(node) => Some(&node.children),)* _ => None } }
            pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> { match self { $(Self::$parent(node) => Some(&mut node.children),)* _ => None } }
            pub fn position(&self) -> Option<&Position> { match self { $(Self::$parent(node) => node.position.as_ref(),)* $(Self::$leaf(node) => node.position.as_ref(),)* } }
            pub fn position_mut(&mut self) -> Option<&mut Position> { match self { $(Self::$parent(node) => node.position.as_mut(),)* $(Self::$leaf(node) => node.position.as_mut(),)* } }
            pub fn position_set(&mut self, position: Option<Position>) { match self { $(Self::$parent(node) => node.position = position,)* $(Self::$leaf(node) => node.position = position,)* } }
        }
    };
}
syntax_tree! {
    parents: Root, Paragraph, Blockquote, Emphasis, Strong, Delete, TableRow, TableCell, Heading, List, ListItem, Table, FootnoteDefinition, Link, LinkReference, MdxJsxTextElement, MdxJsxFlowElement;
    leaves: Image, ImageReference, Definition, FootnoteReference, Code, Math, Text, InlineCode, InlineMath, Html, Yaml, Toml, ThematicBreak, Break, MdxjsEsm, MdxTextExpression, MdxFlowExpression;
}
impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(children) = self.children() {
            for child in children {
                write!(f, "{child}")?;
            }
            return Ok(());
        }
        let value = match self {
            Self::Text(n) => &n.value,
            Self::InlineCode(n) => &n.value,
            Self::InlineMath(n) => &n.value,
            Self::Code(n) => &n.value,
            Self::Math(n) => &n.value,
            Self::Html(n) => &n.value,
            Self::Yaml(n) => &n.value,
            Self::Toml(n) => &n.value,
            Self::MdxjsEsm(n) => &n.value,
            Self::MdxTextExpression(n) => &n.value,
            Self::MdxFlowExpression(n) => &n.value,
            _ => "",
        };
        f.write_str(value)
    }
}
