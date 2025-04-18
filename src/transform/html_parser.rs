use crate::config;
use crate::error::{AppError, AppResult};
use crate::model::common::EntryId;
use crate::model::html::HtmlNode;
use crate::transform::bulk::{resolve_icon, resolve_name, BulkStore};
use crate::transform::util;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Node, Selector};
use serde_json::Value;
use std::fmt::Write as FmtWrite;

use super::bulk::resolve_desc;

static RB_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("rb").expect("Invalid rb selector"));
static RT_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("rt").expect("Invalid rt selector"));

#[inline]
fn clean_consecutive_slashes(text: &str) -> String {
    let mut cleaned = text.to_string();
    while cleaned.contains("//") {
        cleaned = cleaned.replace("//", "/");
    }
    cleaned
}

#[inline]
pub(crate) fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_css_color_to_hex(css_color: &str) -> Option<String> {
    match css_color.parse::<csscolorparser::Color>() {
        Ok(color) => Some(color.to_hex_string()[1..].to_lowercase()),
        Err(_) => None,
    }
}

fn get_element_style_color_value(element_ref: &ElementRef) -> Option<String> {
    element_ref.value().attr("style").and_then(|style| {
        style.split(';').find_map(|prop| {
            let prop = prop.trim();
            if prop.starts_with("color:") {
                prop.splitn(2, ':').nth(1).map(|val| val.trim().to_string())
            } else {
                None
            }
        })
    })
}

fn get_element_style_color_hex(element_ref: &ElementRef) -> Option<String> {
    get_element_style_color_value(element_ref).and_then(|v| parse_css_color_to_hex(&v))
}

fn extract_plain_text(element_ref: ElementRef<'_>) -> String {
    let combined_text = element_ref.text().collect::<String>();
    normalize_whitespace(&combined_text)
}

struct RichTextBuilder {
    buffer: String,
    color_stack: Vec<Option<String>>,
    needs_space: bool,
}

impl RichTextBuilder {
    fn new() -> Self {
        Self {
            buffer: String::with_capacity(256),
            color_stack: vec![None],
            needs_space: false,
        }
    }

    #[inline]
    fn current_color(&self) -> Option<&str> {
        self.color_stack.last().and_then(|c| c.as_deref())
    }

    fn push_color(&mut self, new_color_hex: Option<String>) {
        let effective_color = new_color_hex.or_else(|| self.current_color().map(String::from));
        self.color_stack.push(effective_color);
    }

    fn pop_color(&mut self) {
        if self.color_stack.len() > 1 {
            self.color_stack.pop();
        }
    }

    fn add_text(&mut self, text: &str) {
        let cleaned = text.replace('\u{A0}', " ");
        let normalized = normalize_whitespace(&cleaned);
        if !normalized.is_empty() {
            if self.needs_space
                && !self.buffer.is_empty()
                && !self.buffer.ends_with(char::is_whitespace)
            {
                self.buffer.push(' ');
            }
            let color_hex_owned: Option<String> = self.current_color().map(String::from);
            if let Some(color_hex) = color_hex_owned {
                let _ = write!(self.buffer, "<color=#{}>{}</color>", color_hex, normalized);
            } else {
                self.buffer.push_str(&normalized);
            }
            self.needs_space = !normalized.ends_with(char::is_whitespace);
        } else if cleaned.contains(char::is_whitespace) {
            if !self.buffer.is_empty() && !self.buffer.ends_with(char::is_whitespace) {
                self.needs_space = true;
            }
        }
    }

    fn add_newline(&mut self) {
        if !self.buffer.is_empty() && !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        self.needs_space = false;
    }

    fn flush(mut self, alignment: Option<String>) -> Option<HtmlNode> {
        loop {
            let before_len = self.buffer.len();
            let mut optimized_pass1 = String::with_capacity(self.buffer.len());
            let mut last_end = 0;

            for cap in config::RE_ADJACENT_CLR.captures_iter(&self.buffer) {
                optimized_pass1.push_str(&self.buffer[last_end..cap.get(0).unwrap().start()]);
                let next_color = cap.get(1).unwrap().as_str();
                let text_before_match = &self.buffer[..cap.get(0).unwrap().start()];
                let previous_color_opt = text_before_match.rfind("<color=#").and_then(|start| {
                    text_before_match[start + 7..]
                        .find('>')
                        .map(|end_offset| &text_before_match[start + 7..start + 7 + end_offset])
                });

                if !previous_color_opt
                    .map_or(false, |prev_col| prev_col.eq_ignore_ascii_case(next_color))
                {
                    optimized_pass1.push_str(cap.get(0).unwrap().as_str());
                }
                last_end = cap.get(0).unwrap().end();
            }
            optimized_pass1.push_str(&self.buffer[last_end..]);
            self.buffer = optimized_pass1;

            self.buffer = config::RE_EMPTY_COLOR
                .replace_all(&self.buffer, "")
                .into_owned();

            if self.buffer.len() == before_len {
                break;
            }
        }

        let trimmed_text = self.buffer.trim_matches(char::is_whitespace).to_string();
        if !trimmed_text.is_empty() {
            Some(HtmlNode::RichText {
                text: trimmed_text,
                alignment,
            })
        } else {
            None
        }
    }
}

fn process_custom_element(element_ref: ElementRef<'_>) -> Option<HtmlNode> {
    let el_val = element_ref.value();
    let tag_name = el_val.name().to_lowercase();

    match tag_name.as_str() {
        "custom-entry" => {
            util::parse_value_as_optional_i64(&Value::from(el_val.attr("epid").unwrap_or("")))
                .filter(|&id| id > 0)
                .map(|id| HtmlNode::CustomEntry {
                    ep_id: id,
                    name: extract_plain_text(element_ref),
                    desc: Some(el_val.attr("desc").unwrap_or("").trim().to_string()),
                    icon_url: el_val.attr("icon").unwrap_or("").trim().to_string(),
                    amount: el_val
                        .attr("amount")
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0),
                    display_style: el_val
                        .attr("displaystyle")
                        .unwrap_or("link")
                        .trim()
                        .to_string(),
                    menu_id: el_val.attr("menuid").and_then(|s| s.trim().parse().ok()),
                })
        }
        "custom-image" => el_val
            .attr("url")
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| HtmlNode::CustomImage {
                url: url.to_string(),
                alignment: util::get_alignment_attr(element_ref)
                    .or_else(|| util::get_alignment_style(element_ref)),
            }),
        "custom-ruby" => {
            let rb_text = element_ref
                .select(&RB_SELECTOR)
                .next()
                .map(|rb| rb.text().collect::<String>())
                .unwrap_or_default();
            let rt_text = element_ref
                .select(&RT_SELECTOR)
                .next()
                .map(|rt| rt.text().collect::<String>())
                .unwrap_or_default();
            let rb = normalize_whitespace(&rb_text);
            let rt = normalize_whitespace(&rt_text);
            if !rb.is_empty() && !rt.is_empty() {
                Some(HtmlNode::CustomRuby { rb, rt })
            } else {
                None
            }
        }
        "custom-post" => {
            util::parse_value_as_optional_i64(&Value::from(el_val.attr("postid").unwrap_or("")))
                .filter(|&id| id > 0)
                .map(|id| HtmlNode::CustomPost {
                    post_id: id,
                    name: extract_plain_text(element_ref),
                    icon_url: String::new(),
                })
        }
        "custom-video" => el_val
            .attr("url")
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| HtmlNode::CustomVideo {
                url: url.to_string(),
            }),
        "custom-map" => el_val
            .attr("url")
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| HtmlNode::CustomMap {
                url: url.to_string(),
            }),
        _ => None,
    }
}

fn merge_consecutive_rich_text_nodes(nodes: Vec<HtmlNode>) -> Vec<HtmlNode> {
    let mut merged: Vec<HtmlNode> = Vec::with_capacity(nodes.len());
    let mut iter = nodes.into_iter().peekable();

    while let Some(node) = iter.next() {
        match node {
            HtmlNode::RichText {
                mut text,
                alignment,
            } => {
                while let Some(HtmlNode::RichText {
                    text: next_text,
                    alignment: next_alignment,
                }) = iter.peek()
                {
                    if alignment == *next_alignment && !next_text.trim().is_empty() {
                        if !text.is_empty() && !text.ends_with('\n') {
                            text.push('\n');
                        }
                        text.push_str(next_text);
                        iter.next();
                    } else {
                        break;
                    }
                }
                if !text.trim().is_empty() {
                    merged.push(HtmlNode::RichText { text, alignment });
                }
            }
            other => merged.push(other),
        }
    }
    merged
}

fn process_nested_inline_children(
    inline_element: ElementRef<'_>,
    builder: &mut RichTextBuilder,
    depth: u32,
    page_id: EntryId,
    lang: &str,
) -> AppResult<()> {
    if depth > config::MAX_RECURSION_DEPTH {
        builder.add_text(" [Inline Depth Limit] ");
        return Ok(());
    }

    for child_node in inline_element.children() {
        match child_node.value() {
            Node::Text(text) => {
                builder.add_text(&text.text);
            }
            Node::Element(el_data) => {
                if let Some(child_element_ref) = ElementRef::wrap(child_node) {
                    let tag_name = el_data.name().to_lowercase();
                    if config::HTML_STRIP_TAGS.contains(tag_name.as_str()) {
                        continue;
                    }

                    match tag_name.as_str() {
                        "br" | "hr" => builder.add_newline(),
                        tag if config::HTML_INLINE_TAGS.contains(tag) => {
                            let color_hex = get_element_style_color_hex(&child_element_ref);
                            builder.push_color(color_hex);
                            process_nested_inline_children(
                                child_element_ref,
                                builder,
                                depth + 1,
                                page_id,
                                lang,
                            )?;
                            builder.pop_color();
                        }
                        tag if config::TARGET_HTML_CUSTOM_TAGS.contains(tag) => {
                            if tag == "custom-ruby" {
                                if let Some(HtmlNode::CustomRuby { rb, rt }) =
                                    process_custom_element(child_element_ref)
                                {
                                    let _ = write!(builder.buffer, "{}({})", rb, rt);
                                    builder.needs_space = true;
                                } else {
                                    builder.add_text(&extract_plain_text(child_element_ref));
                                }
                            } else {
                                builder.add_text(&extract_plain_text(child_element_ref));
                            }
                        }
                        _ => {
                            builder.add_text(&extract_plain_text(child_element_ref));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_element_recursive(
    element_ref: ElementRef<'_>,
    depth: u32,
    page_id: EntryId,
    lang: &str,
) -> AppResult<Vec<HtmlNode>> {
    if depth > config::MAX_RECURSION_DEPTH {
        return Ok(vec![HtmlNode::RichText {
            text: "[HTML Depth Limit Exceeded]".to_string(),
            alignment: None,
        }]);
    }

    let mut results: Vec<HtmlNode> = Vec::new();
    let element_alignment = util::get_alignment_style(element_ref);
    let mut current_rich_text_builder = RichTextBuilder::new();

    for child_node in element_ref.children() {
        match child_node.value() {
            Node::Text(text_node) => {
                current_rich_text_builder.add_text(&text_node.text);
            }
            Node::Element(el_data) => {
                if let Some(child_element_ref) = ElementRef::wrap(child_node) {
                    let tag_name = el_data.name().to_lowercase();
                    if config::HTML_STRIP_TAGS.contains(tag_name.as_str()) {
                        continue;
                    }

                    match tag_name.as_str() {
                        tag if config::HEADING_TAGS.contains(tag) => {
                            results
                                .extend(current_rich_text_builder.flush(element_alignment.clone()));
                            current_rich_text_builder = RichTextBuilder::new();
                            if let Ok(level) = tag[1..].parse::<u8>() {
                                let text = extract_plain_text(child_element_ref);
                                let align = util::get_alignment_style(child_element_ref)
                                    .or_else(|| element_alignment.clone());
                                if !text.is_empty() {
                                    results.push(HtmlNode::Heading {
                                        level,
                                        text,
                                        alignment: align,
                                    });
                                }
                            }
                        }
                        tag if config::TARGET_HTML_CUSTOM_TAGS.contains(tag) => {
                            results
                                .extend(current_rich_text_builder.flush(element_alignment.clone()));
                            current_rich_text_builder = RichTextBuilder::new();
                            if let Some(node) = process_custom_element(child_element_ref) {
                                let node_align = match node {
                                    HtmlNode::CustomImage {
                                        url,
                                        alignment: None,
                                    } => HtmlNode::CustomImage {
                                        url,
                                        alignment: element_alignment.clone(),
                                    },
                                    other => other,
                                };
                                results.push(node_align);
                            }
                        }
                        tag if config::HTML_BLOCK_TAGS.contains(tag) || tag == "li" => {
                            results
                                .extend(current_rich_text_builder.flush(element_alignment.clone()));
                            current_rich_text_builder = RichTextBuilder::new();
                            results.extend(parse_element_recursive(
                                child_element_ref,
                                depth + 1,
                                page_id,
                                lang,
                            )?);
                        }
                        "br" | "hr" => {
                            current_rich_text_builder.add_newline();
                        }
                        tag if config::HTML_INLINE_TAGS.contains(tag) => {
                            let color_hex = get_element_style_color_hex(&child_element_ref);
                            current_rich_text_builder.push_color(color_hex);
                            process_nested_inline_children(
                                child_element_ref,
                                &mut current_rich_text_builder,
                                depth + 1,
                                page_id,
                                lang,
                            )?;
                            current_rich_text_builder.pop_color();
                        }
                        _ => {
                            results
                                .extend(current_rich_text_builder.flush(element_alignment.clone()));
                            current_rich_text_builder = RichTextBuilder::new();
                            results.extend(parse_element_recursive(
                                child_element_ref,
                                depth + 1,
                                page_id,
                                lang,
                            )?);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    results.extend(current_rich_text_builder.flush(element_alignment));
    results.retain(|node| !node.is_empty_text());
    Ok(merge_consecutive_rich_text_nodes(results))
}

pub fn parse_html_content(
    html_string: &str,
    page_id: EntryId,
    lang: &str,
) -> AppResult<Vec<HtmlNode>> {
    let trimmed_html = html_string.trim();
    if trimmed_html.is_empty() {
        return Ok(vec![]);
    }
    let cleaned_html = clean_consecutive_slashes(trimmed_html);
    let fragment = Html::parse_fragment(&cleaned_html);
    parse_element_recursive(fragment.root_element(), 0, page_id, lang).map_err(|e| {
        AppError::HtmlParseError(format!("HTML Parse Err [{} / {}]: {}", lang, page_id, e))
    })
}

pub async fn post_process_html_nodes(
    nodes: Vec<HtmlNode>,
    bulk_store: &BulkStore,
) -> AppResult<Vec<HtmlNode>> {
    let mut processed_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            HtmlNode::CustomEntry {
                ep_id,
                name: _,
                desc,
                icon_url: _,
                amount,
                display_style,
                menu_id,
            } => {
                processed_nodes.push(HtmlNode::CustomEntry {
                    ep_id,
                    name: resolve_name(ep_id, bulk_store).unwrap_or_default(),
                    desc: resolve_desc(ep_id, bulk_store).or(desc),
                    icon_url: resolve_icon(ep_id, bulk_store).unwrap_or_default(),
                    amount,
                    display_style,
                    menu_id,
                });
            }
            HtmlNode::CustomPost {
                post_id,
                name: _,
                icon_url: _,
            } => {
                processed_nodes.push(HtmlNode::CustomPost {
                    post_id,
                    name: resolve_name(post_id, bulk_store).unwrap_or_default(),
                    icon_url: resolve_icon(post_id, bulk_store).unwrap_or_default(),
                });
            }
            _ => processed_nodes.push(node),
        }
    }
    Ok(processed_nodes)
}