use crate::element::{ElementKind, SlideElement};
use crate::theme::SlideTheme;
use serde::{Deserialize, Serialize};
use specta::Type;

fn default_transition() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    pub layout: String, // "title" | "title_body" | "section" | "two_col" | "image_caption" | "blank"
    pub elements: Vec<SlideElement>,
    pub notes: String,
    pub bg_override: Option<String>,
    /// Per-slide transition: "none" | "fade" | "slide-left" | "slide-right"
    #[serde(default = "default_transition")]
    pub transition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeckModel {
    pub slides: Vec<Slide>,
    pub theme: SlideTheme,
    pub canvas_width: f64,
    pub canvas_height: f64,
    pub active_slide_index: usize,
    #[serde(default)]
    pub fade_between_slides: bool,
}

impl DeckModel {
    pub fn new_default() -> Self {
        let default_slide = Slide {
            id: uuid::Uuid::now_v7().to_string(),
            layout: "title".to_string(),
            elements: vec![
                SlideElement {
                    id: uuid::Uuid::now_v7().to_string(),
                    x: 100.0,
                    y: 180.0,
                    width: 760.0,
                    height: 80.0,
                    rotation: 0.0,
                    z_index: 1,
                    entrance: "none".to_string(),
                    kind: ElementKind::Text {
                        text: "Click to add title".to_string(),
                        font_size: 44.0,
                        font_family: "Inter, sans-serif".to_string(),
                        color: "#1e293b".to_string(),
                        align: "center".to_string(),
                        bold: false,
                        italic: false,
                        underline: false,
                        bullets: false,
                    },
                },
                SlideElement {
                    id: uuid::Uuid::now_v7().to_string(),
                    x: 150.0,
                    y: 280.0,
                    width: 660.0,
                    height: 50.0,
                    rotation: 0.0,
                    z_index: 2,
                    entrance: "none".to_string(),
                    kind: ElementKind::Text {
                        text: "Click to add subtitle".to_string(),
                        font_size: 24.0,
                        font_family: "Inter, sans-serif".to_string(),
                        color: "#64748b".to_string(),
                        align: "center".to_string(),
                        bold: false,
                        italic: false,
                        underline: false,
                        bullets: false,
                    },
                },
            ],
            notes: "".to_string(),
            bg_override: None,
            transition: default_transition(),
        };

        Self {
            slides: vec![default_slide],
            theme: SlideTheme::default(),
            canvas_width: 960.0,
            canvas_height: 540.0,
            active_slide_index: 0,
            fade_between_slides: false,
        }
    }

    pub fn add_slide(&mut self, layout: &str) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let slide = Slide {
            id: id.clone(),
            layout: layout.to_string(),
            elements: Vec::new(),
            notes: "".to_string(),
            bg_override: None,
            transition: default_transition(),
        };
        self.slides.push(slide);
        self.active_slide_index = self.slides.len() - 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::DeckModel;
    use crate::element::ElementKind;

    #[test]
    fn fade_transition_round_trips() {
        let mut deck = DeckModel::new_default();
        deck.fade_between_slides = true;
        let value = serde_json::to_value(&deck).expect("serialize deck");
        let restored: DeckModel = serde_json::from_value(value).expect("deserialize deck");
        assert!(restored.fade_between_slides);
    }

    #[test]
    fn per_slide_transition_defaults_and_round_trips() {
        let mut deck = DeckModel::new_default();
        assert_eq!(deck.slides[0].transition, "none");
        deck.slides[0].transition = "slide-left".to_string();
        let value = serde_json::to_value(&deck).expect("serialize deck");
        let restored: DeckModel = serde_json::from_value(value).expect("deserialize deck");
        assert_eq!(restored.slides[0].transition, "slide-left");
        // Older decks without transition still deserialize.
        let legacy = serde_json::json!({
            "slides": [{
                "id": "s1",
                "layout": "blank",
                "elements": [],
                "notes": "",
                "bgOverride": null
            }],
            "theme": restored.theme,
            "canvasWidth": 960.0,
            "canvasHeight": 540.0,
            "activeSlideIndex": 0
        });
        let from_legacy: DeckModel = serde_json::from_value(legacy).expect("legacy deck");
        assert_eq!(from_legacy.slides[0].transition, "none");
    }

    #[test]
    fn element_entrance_defaults_and_round_trips() {
        let mut deck = DeckModel::new_default();
        assert_eq!(deck.slides[0].elements[0].entrance, "none");
        deck.slides[0].elements[0].entrance = "fade".to_string();
        let value = serde_json::to_value(&deck).expect("serialize deck");
        let restored: DeckModel = serde_json::from_value(value).expect("deserialize deck");
        assert_eq!(restored.slides[0].elements[0].entrance, "fade");

        // Older elements without entrance still deserialize as "none".
        let mut legacy_value = serde_json::to_value(&restored).expect("serialize restored");
        legacy_value["slides"][0]["elements"][0]
            .as_object_mut()
            .expect("element object")
            .remove("entrance");
        let from_legacy: DeckModel =
            serde_json::from_value(legacy_value).expect("legacy element without entrance");
        assert_eq!(from_legacy.slides[0].elements[0].entrance, "none");
    }

    #[test]
    fn text_formatting_properties_roundtrip() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements[0].kind = ElementKind::Text {
            text: "Formatted Title".to_string(),
            font_size: 36.0,
            font_family: "Roboto, sans-serif".to_string(),
            color: "#ff0000".to_string(),
            align: "right".to_string(),
            bold: true,
            italic: true,
            underline: true,
            bullets: true,
        };

        let json_val = serde_json::to_value(&deck).expect("serialize deck with formatted text");
        let restored: DeckModel =
            serde_json::from_value(json_val).expect("deserialize deck with formatted text");

        if let ElementKind::Text {
            text,
            font_size,
            font_family,
            color,
            align,
            bold,
            italic,
            underline,
            bullets,
        } = &restored.slides[0].elements[0].kind
        {
            assert_eq!(text, "Formatted Title");
            assert_eq!(*font_size, 36.0);
            assert_eq!(font_family, "Roboto, sans-serif");
            assert_eq!(color, "#ff0000");
            assert_eq!(align, "right");
            assert!(*bold);
            assert!(*italic);
            assert!(*underline);
            assert!(*bullets);
        } else {
            panic!("expected ElementKind::Text");
        }

        // Test backwards compatibility deserializing legacy JSON without bold, italic, underline, bullets
        let legacy_kind_json = serde_json::json!({
            "text": {
                "text": "Legacy Title",
                "fontSize": 24.0,
                "fontFamily": "Inter",
                "color": "#000000",
                "align": "left"
            }
        });
        let legacy_kind: ElementKind = serde_json::from_value(legacy_kind_json)
            .expect("deserialize legacy ElementKind::Text without formatting fields");
        if let ElementKind::Text {
            text,
            bold,
            italic,
            underline,
            bullets,
            ..
        } = legacy_kind
        {
            assert_eq!(text, "Legacy Title");
            assert!(!bold);
            assert!(!italic);
            assert!(!underline);
            assert!(!bullets);
        } else {
            panic!("expected ElementKind::Text");
        }
    }
}
