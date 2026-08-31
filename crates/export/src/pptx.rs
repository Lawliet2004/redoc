use crate::base64_util::decode_base64;
use crate::pdf::ExportError;
use redoc_slide_engine::{DeckModel, ElementKind};
use std::fmt::Write;
use std::io::Write as IoWrite;
use zip::write::FileOptions;

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn emu(value: f64) -> u64 {
    (value.max(0.0) * 9_525.0) as u64
}

/// OOXML DrawingML rotation: 1/60000 of a degree, positive = clockwise.
fn ooxml_rot(degrees: f64) -> i64 {
    let normalized = degrees.rem_euclid(360.0);
    if normalized < 1e-9 || (360.0 - normalized) < 1e-9 {
        0
    } else {
        (normalized * 60_000.0).round() as i64
    }
}

fn xfrm_xml(element: &redoc_slide_engine::SlideElement) -> String {
    let x = emu(element.x);
    let y = emu(element.y);
    let width = emu(element.width);
    let height = emu(element.height);
    let rot = ooxml_rot(element.rotation);
    if rot == 0 {
        format!(r#"<a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></a:xfrm>"#)
    } else {
        format!(
            r#"<a:xfrm rot="{rot}"><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></a:xfrm>"#
        )
    }
}

fn graphic_xfrm_xml(element: &redoc_slide_engine::SlideElement) -> String {
    let x = emu(element.x);
    let y = emu(element.y);
    let width = emu(element.width);
    let height = emu(element.height);
    let rot = ooxml_rot(element.rotation);
    if rot == 0 {
        format!(r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></p:xfrm>"#)
    } else {
        format!(
            r#"<p:xfrm rot="{rot}"><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></p:xfrm>"#
        )
    }
}

fn native_chart_type(chart_type: &str) -> Option<&'static str> {
    match chart_type.to_ascii_lowercase().as_str() {
        "bar" | "column" => Some("bar"),
        "line" => Some("line"),
        "pie" => Some("pie"),
        _ => None,
    }
}

fn image_bytes(element: &redoc_slide_engine::SlideElement) -> Option<Vec<u8>> {
    let ElementKind::Image { asset_hash, .. } = &element.kind else {
        return None;
    };
    let (_, encoded) = asset_hash.split_once(',')?;
    if !asset_hash.starts_with("data:") {
        return None;
    }
    decode_base64(encoded)
}

fn image_extension(element: &redoc_slide_engine::SlideElement) -> &'static str {
    match &element.kind {
        ElementKind::Image { mime, .. } if mime.contains("jpeg") || mime.contains("jpg") => "jpg",
        _ => "png",
    }
}

fn chart_part_xml(chart_type: &str, data: &[f64], labels: &[String]) -> Option<String> {
    let chart_type = native_chart_type(chart_type)?;
    let title = labels.first().map(String::as_str).unwrap_or("Chart");
    let categories = data
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let label = labels
                .get(index + 1)
                .or_else(|| labels.get(index))
                .map(String::as_str)
                .unwrap_or_else(|| "Item");
            format!(
                r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
                xml_escape(label)
            )
        })
        .collect::<String>();
    let values = data
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let value = if value.is_finite() { *value } else { 0.0 };
            format!(r#"<c:pt idx="{index}"><c:v>{value}</c:v></c:pt>"#)
        })
        .collect::<String>();
    let point_count = data.len();
    let series = format!(
        r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:tx><c:v>Series 1</c:v></c:tx><c:cat><c:strLit><c:ptCount val="{point_count}"/>{categories}</c:strLit></c:cat><c:val><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="{point_count}"/>{values}</c:numLit></c:val></c:ser>"#
    );
    let title_xml = format!(
        r#"<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:layout/><c:overlay val="0"/></c:title>"#,
        xml_escape(title)
    );
    let plot_xml = if chart_type == "bar" {
        format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{series}<c:axId val="-2068027336"/><c:axId val="-2019485262"/></c:barChart>"#
        )
    } else if chart_type == "line" {
        format!(
            r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>{series}<c:axId val="-2068027336"/><c:axId val="-2019485262"/></c:lineChart>"#
        )
    } else {
        format!(r#"<c:pieChart><c:varyColors val="1"/>{series}</c:pieChart>"#)
    };
    let axes = if chart_type == "pie" {
        String::new()
    } else {
        r#"<c:catAx><c:axId val="-2068027336"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="-2019485262"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/></c:catAx><c:valAx><c:axId val="-2019485262"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="-2068027336"/><c:crosses val="autoZero"/><c:crossBetween val="midCat"/></c:valAx>"#.to_string()
    };
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:date1904 val="0"/><c:lang val="en-US"/><c:roundedCorners val="0"/><c:chart>{title_xml}<c:plotArea><c:layout/>{plot_xml}{axes}</c:plotArea><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart><c:printSettings><c:headerFooter/><c:pageMargins b="0.75" l="0.7" r="0.7" t="0.75" header="0.3" footer="0.3"/><c:pageSetup/></c:printSettings></c:chartSpace>"#
    ))
}

fn shape_xml(
    id: u32,
    element: &redoc_slide_engine::SlideElement,
    image_rel: Option<&str>,
    chart_rel: Option<&str>,
) -> String {
    let xfrm = xfrm_xml(element);
    match &element.kind {
        ElementKind::Text {
            text,
            font_size,
            font_family,
            color,
            align,
            bold,
            italic,
            underline,
            bullets,
        } => {
            let algn = match align.as_str() {
                "center" | "ctr" => "ctr",
                "right" | "r" => "r",
                "justify" => "j",
                _ => "l",
            };
            let bu_xml = if *bullets {
                r#"<a:buChar char="•"/>"#
            } else {
                ""
            };
            let b_attr = if *bold { r#" b="1""# } else { "" };
            let i_attr = if *italic { r#" i="1""# } else { "" };
            let u_attr = if *underline { r#" u="sng""# } else { "" };

            let clean_color = color.trim_start_matches('#');
            let color_xml = if !clean_color.is_empty() {
                format!(r#"<a:solidFill><a:srgbClr val="{clean_color}"/></a:solidFill>"#)
            } else {
                String::new()
            };

            let font_name = font_family.split(',').next().unwrap_or("Calibri").trim();
            let font_xml = if !font_name.is_empty() {
                format!(r#"<a:latin typeface="{}"/>"#, xml_escape(font_name))
            } else {
                String::new()
            };

            let sz = (*font_size * 100.0) as u64;

            let paragraphs: Vec<String> = text
                .split('\n')
                .map(|line| {
                    format!(
                        r#"<a:p><a:pPr algn="{algn}">{bu_xml}</a:pPr><a:r><a:rPr sz="{sz}"{b_attr}{i_attr}{u_attr}>{color_xml}{font_xml}</a:rPr><a:t>{}</a:t></a:r></a:p>"#,
                        xml_escape(line)
                    )
                })
                .collect();

            let tx_body_content = paragraphs.join("");

            format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Text {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{xfrm}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/>{tx_body_content}</p:txBody></p:sp>"#
            )
        }
        ElementKind::Shape {
            shape_type,
            fill_color,
            stroke_color,
            stroke_width,
            text: _,
        } => {
            let stroke_color_clean = stroke_color.trim_start_matches('#');
            let stroke_w_emu = (stroke_width.max(0.0) * 12_700.0) as u64;

            match shape_type.as_str() {
                "line" | "arrow" => {
                    let tail_end = if shape_type == "arrow" {
                        r#"<a:tailEnd type="triangle"/>"#
                    } else {
                        ""
                    };
                    let stroke_xml = if stroke_w_emu > 0 && !stroke_color_clean.is_empty() {
                        format!(
                            r#"<a:ln w="{stroke_w_emu}"><a:solidFill><a:srgbClr val="{stroke_color_clean}"/></a:solidFill>{tail_end}</a:ln>"#
                        )
                    } else if !stroke_color_clean.is_empty() {
                        format!(
                            r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="{stroke_color_clean}"/></a:solidFill>{tail_end}</a:ln>"#
                        )
                    } else {
                        format!(
                            r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill>{tail_end}</a:ln>"#
                        )
                    };

                    let name_prefix = if shape_type == "arrow" {
                        "Arrow"
                    } else {
                        "Line"
                    };

                    format!(
                        r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{id}" name="{name_prefix} {id}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>{xfrm}<a:prstGeom prst="line"><a:avLst/></a:prstGeom>{stroke_xml}</p:spPr></p:cxnSp>"#
                    )
                }
                _ => {
                    let preset = if shape_type == "ellipse" {
                        "ellipse"
                    } else {
                        "rect"
                    };
                    let fill_color_clean = fill_color.trim_start_matches('#');
                    let fill_xml = if !fill_color_clean.is_empty() {
                        format!(
                            r#"<a:solidFill><a:srgbClr val="{fill_color_clean}"/></a:solidFill>"#
                        )
                    } else {
                        r#"<a:noFill/>"#.to_string()
                    };
                    let stroke_xml = if stroke_w_emu > 0 && !stroke_color_clean.is_empty() {
                        format!(
                            r#"<a:ln w="{stroke_w_emu}"><a:solidFill><a:srgbClr val="{stroke_color_clean}"/></a:solidFill></a:ln>"#
                        )
                    } else {
                        String::new()
                    };

                    format!(
                        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Shape {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{xfrm}<a:prstGeom prst="{preset}"><a:avLst/></a:prstGeom>{fill_xml}{stroke_xml}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>"#
                    )
                }
            }
        }
        ElementKind::Image { asset_hash, .. } => match image_rel {
            Some(relationship) => format!(
                r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Image {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{relationship}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
            ),
            None => {
                let warning_text = format!("[Image unavailable: {}]", xml_escape(asset_hash));
                format!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Image Warning {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FEF2F2"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="EF4444"/></a:solidFill></a:ln></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="1200"><a:solidFill><a:srgbClr val="991B1B"/></a:solidFill></a:rPr><a:t>{warning_text}</a:t></a:r></a:p></p:txBody></p:sp>"#
                )
            }
        },
        ElementKind::Table { rows, cols, data } => {
            let rows = (*rows).max(1);
            let cols = (*cols).max(1);
            let row_height = emu(element.height / rows as f64);
            let col_width = emu(element.width / cols as f64);
            let xfrm_inner = format!(
                r#"<a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/>"#,
                emu(element.x),
                emu(element.y),
                emu(element.width),
                emu(element.height)
            );
            let xfrm = if ooxml_rot(element.rotation) == 0 {
                format!(r#"<p:xfrm>{xfrm_inner}</p:xfrm>"#)
            } else {
                format!(
                    r#"<p:xfrm rot="{}">{xfrm_inner}</p:xfrm>"#,
                    ooxml_rot(element.rotation)
                )
            };
            let grid = (0..cols)
                .map(|_| format!(r#"<a:gridCol w="{col_width}"/>"#))
                .collect::<String>();
            let rows_xml = (0..rows)
                .map(|row_index| {
                    let cells_xml = (0..cols)
                        .map(|col_index| {
                            let value = data
                                .get(row_index)
                                .and_then(|row| row.get(col_index))
                                .map(String::as_str)
                                .unwrap_or_default();
                            format!(
                                r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1000"/><a:t>{}</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc>"#,
                                xml_escape(value)
                            )
                        })
                        .collect::<String>();
                    format!(r#"<a:tr h="{row_height}">{cells_xml}</a:tr>"#)
                })
                .collect::<String>();
            format!(
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Table {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>{xfrm}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1" bandRow="1"><a:tableStyleId>{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}</a:tableStyleId></a:tblPr><a:tblGrid>{grid}</a:tblGrid>{rows_xml}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
            )
        }
        ElementKind::Chart {
            chart_type,
            data,
            labels,
        } => {
            if let Some(chart_rel) = chart_rel {
                return format!(
                    r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Chart {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>{}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{chart_rel}"/></a:graphicData></a:graphic></p:graphicFrame>"#,
                    graphic_xfrm_xml(element)
                );
            }
            let title = labels
                .first()
                .map(|s| xml_escape(s))
                .unwrap_or_else(|| "Chart".into());
            let summary = data
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let label = labels
                        .get(i + 1)
                        .or_else(|| labels.get(i))
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    format!("{}: {}", xml_escape(label), v)
                })
                .collect::<Vec<_>>()
                .join(" · ");
            let body = format!(
                r#"<a:p><a:r><a:rPr sz="1400" b="1"/><a:t>{title}</a:t></a:r></a:p><a:p><a:r><a:rPr sz="1000"/><a:t>{} ({})</a:t></a:r></a:p>"#,
                xml_escape(&summary),
                xml_escape(chart_type)
            );
            format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Chart {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="F8FAFC"/></a:solidFill><a:ln w="9525"><a:solidFill><a:srgbClr val="94A3B8"/></a:solidFill></a:ln></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp>"#
            )
        }
    }
}

fn theme1_xml(theme: &redoc_slide_engine::SlideTheme) -> String {
    let name = xml_escape(&theme.name);
    let bg_color = theme.bg_color.trim_start_matches('#');
    let text_color = theme.text_color.trim_start_matches('#');
    let accent_color = theme.accent_color.trim_start_matches('#');
    let font_name = theme
        .font_family
        .split(',')
        .next()
        .unwrap_or("Calibri")
        .trim();
    let font_escaped = xml_escape(font_name);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}"><a:themeElements><a:clrScheme name="{name}"><a:dk1><a:srgbClr val="{text_color}"/></a:dk1><a:lt1><a:srgbClr val="{bg_color}"/></a:lt1><a:accent1><a:srgbClr val="{accent_color}"/></a:accent1></a:clrScheme><a:fontScheme name="{name}"><a:majorFont><a:latin typeface="{font_escaped}"/></a:majorFont><a:minorFont><a:latin typeface="{font_escaped}"/></a:minorFont></a:fontScheme><a:fmtScheme name="{name}"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectLst/></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    )
}

fn slide_background_xml(slide: &redoc_slide_engine::Slide) -> String {
    let Some(color) = slide.bg_override.as_deref() else {
        return String::new();
    };
    let color = color.trim_start_matches('#');
    if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return String::new();
    }
    format!(
        r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{color}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#
    )
}

fn notes_master_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Header Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="hdr"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Date Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="dt"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="4" name="Slide Image Placeholder 3"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="5" name="Notes Placeholder 4"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="6" name="Footer Placeholder 5"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="ftr"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="7" name="Slide Number Placeholder 6"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap accent1="accent1" bg1="lt1" tx1="dk1"/></p:notesMaster>"#.to_string()
}

fn notes_slide_xml(notes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#,
        xml_escape(notes)
    )
}

fn transition_xml(transition: &str) -> String {
    match transition {
        "fade" => r#"<p:transition><p:fade/></p:transition>"#.to_string(),
        "slide-left" => r#"<p:transition><p:push dir="l"/></p:transition>"#.to_string(),
        "slide-right" => r#"<p:transition><p:push dir="r"/></p:transition>"#.to_string(),
        _ => String::new(),
    }
}

/// Emit the supported entrance-animation subset as native PresentationML timing.
///
/// The editor currently supports a fade entrance.  PowerPoint identifies targets
/// by the shape's `cNvPr/@id`; the shape ids below intentionally mirror the ids
/// emitted by `shape_xml` (element index + 2).  We use a deterministic sequence
/// so exported files play in document order and remain stable for round trips.
fn animation_timing_xml(slide: &redoc_slide_engine::Slide) -> String {
    let animated: Vec<u32> = slide
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| element.entrance.eq_ignore_ascii_case("fade"))
        .map(|(index, _)| (index + 2) as u32)
        .collect();
    if animated.is_empty() {
        return String::new();
    }

    let children = animated
        .iter()
        .enumerate()
        .map(|(index, shape_id)| {
            let sequence_id = 10 + (index as u32 * 3);
            let behavior_id = sequence_id + 1;
            format!(
                r#"<p:par><p:cTn id="{sequence_id}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="{behavior_id}" dur="350" fill="hold"/><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par>"#
            )
        })
        .collect::<String>();

    format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{children}</p:childTnLst></p:cTn></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
    )
}

fn slide_xml(slide: &redoc_slide_engine::Slide) -> String {
    let mut shapes = String::new();
    let mut image_index = 0usize;
    let image_count = slide
        .elements
        .iter()
        .filter(|element| image_bytes(element).is_some())
        .count();
    let mut chart_index = 0usize;
    for (index, element) in slide.elements.iter().enumerate() {
        let relationship = if image_bytes(element).is_some() {
            image_index += 1;
            Some(format!("rId{}", image_index + 1))
        } else {
            None
        };
        let chart_relationship = if matches!(&element.kind, ElementKind::Chart { chart_type, .. } if native_chart_type(chart_type).is_some())
        {
            chart_index += 1;
            Some(format!("rId{}", image_count + chart_index + 1))
        } else {
            None
        };
        shapes.push_str(&shape_xml(
            (index + 2) as u32,
            element,
            relationship.as_deref(),
            chart_relationship.as_deref(),
        ));
    }
    let transition = transition_xml(&slide.transition);
    let timing = animation_timing_xml(slide);
    let background = slide_background_xml(slide);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld>{background}<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{transition}{timing}</p:sld>"#
    )
}

#[allow(clippy::drop_non_drop)]
pub fn export_deck_to_pptx(deck: &DeckModel) -> Result<Vec<u8>, ExportError> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut add = |path: &str, contents: String| -> Result<(), ExportError> {
        zip.start_file(path, options)?;
        zip.write_all(contents.as_bytes())?;
        Ok(())
    };

    let mut overrides = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpg" ContentType="image/jpeg"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>"#,
    );
    for index in 1..=deck.slides.len() {
        write!(
            overrides,
            r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        )
        .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
    }
    for (index, slide) in deck.slides.iter().enumerate() {
        if !slide.notes.trim().is_empty() {
            let notes_index = index + 1;
            write!(
                overrides,
                r#"<Override PartName="/ppt/notesSlides/notesSlide{notes_index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#
            )
            .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
        }
        let mut chart_index = 0usize;
        for element in &slide.elements {
            if let ElementKind::Chart { chart_type, .. } = &element.kind {
                if native_chart_type(chart_type).is_some() {
                    chart_index += 1;
                    write!(
                        overrides,
                        r#"<Override PartName="/ppt/charts/chart{index}_{chart_index}.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>"#
                    )
                    .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
                }
            }
        }
    }
    overrides.push_str("</Types>");
    add("[Content_Types].xml", overrides)?;
    add(
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#.to_string(),
    )?;

    let mut slide_ids = String::new();
    let mut presentation_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for (index, _) in deck.slides.iter().enumerate() {
        let id = index + 2;
        write!(
            slide_ids,
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + id,
            id
        )
        .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
        write!(
            presentation_rels,
            r#"<Relationship Id="rId{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
            id - 1
        )
        .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
    }
    let notes_master_rid = deck.slides.len() + 2;
    write!(
        presentation_rels,
        r#"<Relationship Id="rId{notes_master_rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/>"#
    )
    .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;

    presentation_rels.push_str("</Relationships>");
    add(
        "ppt/presentation.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id="rId{notes_master_rid}"/></p:notesMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
        ),
    )?;
    add("ppt/_rels/presentation.xml.rels", presentation_rels)?;
    add(
        "ppt/slideMasters/slideMaster1.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles/><p:clrMap accent1="accent1" bg1="lt1" tx1="dk1"/></p:sldMaster>"#.to_string(),
    )?;
    add(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#.to_string(),
    )?;
    add(
        "ppt/slideLayouts/slideLayout1.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#.to_string(),
    )?;
    add(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#.to_string(),
    )?;
    add("ppt/theme/theme1.xml", theme1_xml(&deck.theme))?;
    add("ppt/notesMasters/notesMaster1.xml", notes_master_xml())?;
    add(
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#.to_string(),
    )?;

    let mut media_files: Vec<(String, Vec<u8>)> = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        let slide_number = index + 1;
        add(
            &format!("ppt/slides/slide{slide_number}.xml"),
            slide_xml(slide),
        )?;
        let mut relationships = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#,
        );
        let mut next_rel = 2usize;
        for element in &slide.elements {
            if image_bytes(element).is_some() {
                write!(
                    relationships,
                    r#"<Relationship Id="rId{next_rel}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{}_{}.{}"/>"#,
                    slide_number,
                    next_rel - 1,
                    image_extension(element)
                )
                .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
                next_rel += 1;
            }
        }
        let mut chart_index = 0usize;
        for element in &slide.elements {
            if let ElementKind::Chart {
                chart_type,
                data,
                labels,
            } = &element.kind
            {
                if native_chart_type(chart_type).is_some() {
                    chart_index += 1;
                    write!(
                        relationships,
                        r#"<Relationship Id="rId{next_rel}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart{slide_number}_{chart_index}.xml"/>"#
                    )
                    .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
                    next_rel += 1;
                    add(
                        &format!("ppt/charts/chart{slide_number}_{chart_index}.xml"),
                        chart_part_xml(chart_type, data, labels).expect("native chart type"),
                    )?;
                }
            }
        }
        let has_notes = !slide.notes.trim().is_empty();
        if has_notes {
            write!(
                relationships,
                r#"<Relationship Id="rId{next_rel}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide{slide_number}.xml"/>"#
            )
            .map_err(|e| ExportError::Io(std::io::Error::other(e.to_string())))?;
        }
        relationships.push_str("</Relationships>");
        add(
            &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
            relationships,
        )?;
        if has_notes {
            add(
                &format!("ppt/notesSlides/notesSlide{slide_number}.xml"),
                notes_slide_xml(slide.notes.trim()),
            )?;
            add(
                &format!("ppt/notesSlides/_rels/notesSlide{slide_number}.xml.rels"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide{slide_number}.xml"/></Relationships>"#
                ),
            )?;
        }
        let mut image_index = 0usize;
        for element in &slide.elements {
            if let Some(bytes) = image_bytes(element) {
                image_index += 1;
                let path = format!(
                    "ppt/media/image{}_{}.{}",
                    slide_number,
                    image_index,
                    image_extension(element)
                );
                media_files.push((path, bytes));
            }
        }
    }
    drop(add);
    for (path, bytes) in media_files {
        zip.start_file(path, options)?;
        zip.write_all(&bytes)?;
    }
    let cursor = zip
        .finish()
        .map_err(|error| ExportError::Io(std::io::Error::other(error.to_string())))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn writes_presentation_and_slide_parts() {
        let bytes = export_deck_to_pptx(&DeckModel::new_default()).expect("export pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/_rels/presentation.xml.rels").is_ok());
        assert!(archive.by_name("ppt/notesMasters/notesMaster1.xml").is_ok());
    }

    #[test]
    fn embeds_data_uri_images() {
        let mut deck = DeckModel::new_default();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "image-1".to_string(),
                x: 20.0,
                y: 20.0,
                width: 100.0,
                height: 100.0,
                rotation: 0.0,
                z_index: 3,
                entrance: "none".to_string(),
                kind: ElementKind::Image {
                    asset_hash: "data:image/png;base64,aGVsbG8=".to_string(),
                    mime: "image/png".to_string(),
                },
            });
        let bytes = export_deck_to_pptx(&deck).expect("export image pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        assert!(archive.by_name("ppt/media/image1_1.png").is_ok());
        let mut rels = String::new();
        archive
            .by_name("ppt/slides/_rels/slide1.xml.rels")
            .expect("slide relationships")
            .read_to_string(&mut rels)
            .expect("read relationships");
        assert!(rels.contains("relationships/image"));
    }

    #[test]
    fn writes_fade_transition_when_configured() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].transition = "fade".to_string();
        let bytes = export_deck_to_pptx(&deck).expect("export pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut slide = archive.by_name("ppt/slides/slide1.xml").expect("slide1");
        let mut xml = String::new();
        slide.read_to_string(&mut xml).expect("read slide xml");
        assert!(
            xml.contains("<p:fade/>"),
            "expected fade transition in slide xml"
        );
    }

    #[test]
    fn writes_native_fade_entrance_timing() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements[0].entrance = "fade".to_string();
        let bytes = export_deck_to_pptx(&deck).expect("export animated pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut xml)
            .expect("read slide xml");
        assert!(xml.contains("<p:timing>"));
        assert!(xml.contains("<p:animEffect transition=\"in\" filter=\"fade\">"));
        assert!(xml.contains("<p:spTgt spid=\"2\"/>"));
    }

    #[test]
    fn writes_native_chart_parts_for_supported_types() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].bg_override = Some("#334455".to_string());
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "chart-1".to_string(),
                x: 20.0,
                y: 20.0,
                width: 320.0,
                height: 180.0,
                rotation: 0.0,
                z_index: 1,
                entrance: "none".to_string(),
                kind: ElementKind::Chart {
                    chart_type: "bar".to_string(),
                    data: vec![1.0, 2.5],
                    labels: vec!["Revenue".to_string(), "Q1".to_string(), "Q2".to_string()],
                },
            });
        let bytes = export_deck_to_pptx(&deck).expect("export chart pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut slide = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut slide)
            .expect("read slide xml");
        assert!(slide.contains("drawingml/2006/chart"));
        assert!(slide.contains("<p:bg>") && slide.contains("val=\"334455\""));
        assert!(slide.contains("r:id=\"rId2\""));
        let mut chart = String::new();
        archive
            .by_name("ppt/charts/chart1_1.xml")
            .expect("chart part")
            .read_to_string(&mut chart)
            .expect("read chart xml");
        assert!(chart.contains("<c:barChart>"));
        assert!(chart.contains("Revenue"));
        assert!(chart.contains("Q2"));
        let mut content_types = String::new();
        archive
            .by_name("[Content_Types].xml")
            .expect("content types")
            .read_to_string(&mut content_types)
            .expect("read content types");
        assert!(content_types.contains("drawingml.chart+xml"));
    }

    #[test]
    fn writes_rotation_and_speaker_notes() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].notes = "Remember to pause here.".to_string();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "rotated-text".to_string(),
                x: 120.0,
                y: 80.0,
                width: 300.0,
                height: 60.0,
                rotation: 45.0,
                z_index: 1,
                entrance: "none".to_string(),
                kind: ElementKind::Text {
                    text: "Tilted".to_string(),
                    font_size: 28.0,
                    font_family: "Helvetica".to_string(),
                    color: "#111111".to_string(),
                    align: "left".to_string(),
                    bold: false,
                    italic: false,
                    underline: false,
                    bullets: false,
                },
            });
        let bytes = export_deck_to_pptx(&deck).expect("export pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");

        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut slide_xml)
            .expect("read slide");
        assert!(
            slide_xml.contains(r#"rot="2700000""#),
            "expected OOXML rot for 45 degrees, got: {slide_xml}"
        );

        assert!(
            archive.by_name("ppt/notesSlides/notesSlide1.xml").is_ok(),
            "expected notesSlide1.xml in pptx zip"
        );
        let mut notes_xml = String::new();
        archive
            .by_name("ppt/notesSlides/notesSlide1.xml")
            .expect("notes")
            .read_to_string(&mut notes_xml)
            .expect("read notes");
        assert!(notes_xml.contains("Remember to pause here."));

        let mut rels = String::new();
        archive
            .by_name("ppt/slides/_rels/slide1.xml.rels")
            .expect("slide rels")
            .read_to_string(&mut rels)
            .expect("read rels");
        assert!(rels.contains("relationships/notesSlide"));
        assert!(archive
            .by_name("ppt/notesSlides/_rels/notesSlide1.xml.rels")
            .is_ok());
    }

    #[test]
    fn exports_formatted_text_properties() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "fmt-text".to_string(),
                x: 50.0,
                y: 50.0,
                width: 400.0,
                height: 100.0,
                rotation: 0.0,
                z_index: 1,
                entrance: "none".to_string(),
                kind: ElementKind::Text {
                    text: "Line 1\nLine 2".to_string(),
                    font_size: 32.0,
                    font_family: "Roboto, sans-serif".to_string(),
                    color: "#FF5722".to_string(),
                    align: "center".to_string(),
                    bold: true,
                    italic: true,
                    underline: true,
                    bullets: true,
                },
            });
        let bytes = export_deck_to_pptx(&deck).expect("export formatted text pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut slide_xml)
            .expect("read slide xml");

        assert!(slide_xml.contains(r#"algn="ctr""#));
        assert!(slide_xml.contains(r#"<a:buChar char="•"/>"#));
        assert!(slide_xml.contains(r#"b="1""#));
        assert!(slide_xml.contains(r#"i="1""#));
        assert!(slide_xml.contains(r#"u="sng""#));
        assert!(slide_xml.contains(r#"val="FF5722""#));
        assert!(slide_xml.contains(r#"typeface="Roboto""#));
        assert!(slide_xml.contains("Line 1"));
        assert!(slide_xml.contains("Line 2"));
    }

    #[test]
    fn exports_line_and_arrow_connector_shapes() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "line-1".to_string(),
                x: 100.0,
                y: 100.0,
                width: 200.0,
                height: 0.0,
                rotation: 0.0,
                z_index: 1,
                entrance: "none".to_string(),
                kind: ElementKind::Shape {
                    shape_type: "line".to_string(),
                    fill_color: "".to_string(),
                    stroke_color: "#00FF00".to_string(),
                    stroke_width: 2.0,
                    text: String::new(),
                },
            });
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "arrow-1".to_string(),
                x: 100.0,
                y: 200.0,
                width: 200.0,
                height: 0.0,
                rotation: 0.0,
                z_index: 2,
                entrance: "none".to_string(),
                kind: ElementKind::Shape {
                    shape_type: "arrow".to_string(),
                    fill_color: "".to_string(),
                    stroke_color: "#FF0000".to_string(),
                    stroke_width: 3.0,
                    text: String::new(),
                },
            });

        let bytes = export_deck_to_pptx(&deck).expect("export connectors pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut slide_xml)
            .expect("read slide xml");

        assert!(slide_xml.contains("<p:cxnSp>"));
        assert!(slide_xml.contains(r#"prst="line""#));
        assert!(slide_xml.contains(r#"val="00FF00""#));
        assert!(slide_xml.contains(r#"val="FF0000""#));
        assert!(slide_xml.contains(r#"<a:tailEnd type="triangle"/>"#));
    }

    #[test]
    fn exports_deck_theme_colors_and_notes_master() {
        let mut deck = DeckModel::new_default();
        deck.theme.name = "Custom Theme".to_string();
        deck.theme.bg_color = "#121212".to_string();
        deck.theme.text_color = "#F5F5F5".to_string();
        deck.theme.accent_color = "#9C27B0".to_string();
        deck.theme.font_family = "Poppins, sans-serif".to_string();

        let bytes = export_deck_to_pptx(&deck).expect("export pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");

        // Check theme
        let mut theme_xml = String::new();
        archive
            .by_name("ppt/theme/theme1.xml")
            .expect("theme1")
            .read_to_string(&mut theme_xml)
            .expect("read theme xml");

        assert!(theme_xml.contains(r#"val="F5F5F5""#));
        assert!(theme_xml.contains(r#"val="121212""#));
        assert!(theme_xml.contains(r#"val="9C27B0""#));
        assert!(theme_xml.contains(r#"typeface="Poppins""#));

        // Check notesMaster
        assert!(archive.by_name("ppt/notesMasters/notesMaster1.xml").is_ok());
    }

    #[test]
    fn handles_missing_or_url_images_gracefully() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.clear();
        deck.slides[0]
            .elements
            .push(redoc_slide_engine::SlideElement {
                id: "url-image".to_string(),
                x: 100.0,
                y: 100.0,
                width: 200.0,
                height: 150.0,
                rotation: 0.0,
                z_index: 1,
                entrance: "none".to_string(),
                kind: ElementKind::Image {
                    asset_hash: "https://example.com/test.png".to_string(),
                    mime: "image/png".to_string(),
                },
            });

        let bytes = export_deck_to_pptx(&deck).expect("export missing image pptx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("read pptx zip");
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide1")
            .read_to_string(&mut slide_xml)
            .expect("read slide xml");

        assert!(slide_xml.contains("[Image unavailable: https://example.com/test.png]"));
        assert!(slide_xml.contains("Image Warning"));
    }
}
