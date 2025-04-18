use crate::model::common::{EntryId, MenuId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum HtmlNode {
    RichText {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alignment: Option<String>,
    },
    Heading {
        level: u8,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alignment: Option<String>,
    },
    CustomEntry {
        #[serde(rename = "epId")]
        ep_id: EntryId,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        desc: Option<String>,
        #[serde(default, skip_serializing_if = "String::is_empty", rename = "iconUrl")]
        icon_url: String,
        #[serde(default, skip_serializing_if = "is_zero")]
        amount: i64,
        #[serde(
            default = "default_display_style",
            skip_serializing_if = "is_default_display_style",
            rename = "displayStyle"
        )]
        display_style: String,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "menuId")]
        menu_id: Option<MenuId>,
    },
    CustomImage {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alignment: Option<String>,
    },
    CustomRuby {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        rb: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        rt: String,
    },
    CustomPost {
        #[serde(rename = "postId")]
        post_id: EntryId,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        name: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        icon_url: String,
    },
    CustomVideo {
        url: String,
    },
    CustomMap {
        url: String,
    },
}

#[inline]
pub fn default_display_style() -> String {
    "link".to_string()
}

#[inline]
fn is_default_display_style(style: &str) -> bool {
    style == "link"
}

#[inline]
fn is_zero(num: &i64) -> bool {
    *num == 0
}

impl HtmlNode {
    pub fn is_empty_text(&self) -> bool {
        match self {
            HtmlNode::RichText { text, .. } => text.trim().is_empty(),
            HtmlNode::Heading { text, .. } => text.trim().is_empty(),
            HtmlNode::CustomRuby { rb, rt } => rb.trim().is_empty() && rt.trim().is_empty(),
            _ => false,
        }
    }
}
