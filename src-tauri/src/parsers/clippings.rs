use crate::models::{Clipping, ClippingType};
use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct ClippingsParser;

impl ClippingsParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Clipping>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read clippings file: {}", e))?;
        Ok(Self::parse_str(&content))
    }

    pub fn parse_str(content: &str) -> Vec<Clipping> {
        // Strip BOM if present
        let clean_content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let blocks = clean_content.split("==========");

        let mut clippings = Vec::new();

        for block in blocks {
            let lines: Vec<&str> = block
                .lines()
                .map(|l| l.trim_matches('\r'))
                .filter(|l| !l.trim().is_empty())
                .collect();

            if lines.len() < 2 {
                continue;
            }

            let title_line = lines[0].trim();
            let meta_line = lines[1].trim();

            // The content is lines[2..] joined
            let body = if lines.len() > 2 {
                lines[2..].join("\n").trim().to_string()
            } else {
                String::new()
            };

            // Parse title & author
            let (title, author) = Self::parse_title_author(title_line);

            // Parse metadata (type, location, page, added date)
            let (clipping_type, location, page, created_at) = Self::parse_metadata(meta_line);

            // If it's a bookmark with no content, body might be empty
            if body.is_empty() && clipping_type != ClippingType::Bookmark {
                continue;
            }

            let id = Self::generate_id(&title, &location, &clipping_type, &body);

            clippings.push(Clipping {
                id,
                book_title: title,
                author,
                clipping_type,
                location,
                page,
                created_at,
                content: body,
            });
        }

        clippings
    }

    fn parse_title_author(line: &str) -> (String, Option<String>) {
        // Formats like: Title (Author) or Title (Author Name)
        // Note: Title might also have parentheses inside, so we grab the last ( ... )
        if let Some(open_idx) = line.rfind('(') {
            if let Some(close_idx) = line.rfind(')') {
                if close_idx > open_idx {
                    let title = line[..open_idx].trim().to_string();
                    let author = line[open_idx + 1..close_idx].trim().to_string();
                    if !title.is_empty() {
                        return (title, if author.is_empty() { None } else { Some(author) });
                    }
                }
            }
        }
        (line.to_string(), None)
    }

    fn parse_metadata(
        line: &str,
    ) -> (ClippingType, Option<String>, Option<u32>, Option<DateTime<Utc>>) {
        let lower = line.to_lowercase();

        // 1. Clipping Type
        let clipping_type = if lower.contains("highlight") || line.contains("ハイライト") {
            ClippingType::Highlight
        } else if lower.contains("note") || line.contains("メモ") {
            ClippingType::Note
        } else if lower.contains("bookmark") || line.contains("しおり") {
            ClippingType::Bookmark
        } else {
            ClippingType::Highlight
        };

        // 2. Page
        let page = {
            let page_re = Regex::new(r"(?i)(?:page\s*([0-9]+)|([0-9]+)\s*ページ)").unwrap();
            page_re.captures(line).and_then(|c| {
                c.get(1)
                    .or_else(|| c.get(2))
                    .and_then(|m| m.as_str().parse::<u32>().ok())
            })
        };

        // 3. Location
        let location = {
            let loc_re = Regex::new(
                r"(?i)(?:location|位置No\.?|位置)\s*([0-9]+(?:\s*-\s*[0-9]+)?)",
            )
            .unwrap();
            loc_re
                .captures(line)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().replace(' ', ""))
        };

        // 4. Date/Time
        let created_at = Self::parse_date(line);

        (clipping_type, location, page, created_at)
    }

    fn parse_date(line: &str) -> Option<DateTime<Utc>> {
        // Japanese format: 作成日時: 2026年9月18日金曜日 21:30:15 or 2026年9月18日 21:30:15
        let jp_re = Regex::new(r"(\d{4})年(\d{1,2})月(\d{1,2})日(?:[^\d\s]+)?\s+(\d{1,2}):(\d{2}):(\d{2})").unwrap();
        if let Some(caps) = jp_re.captures(line) {
            let year: i32 = caps[1].parse().ok()?;
            let month: u32 = caps[2].parse().ok()?;
            let day: u32 = caps[3].parse().ok()?;
            let hour: u32 = caps[4].parse().ok()?;
            let min: u32 = caps[5].parse().ok()?;
            let sec: u32 = caps[6].parse().ok()?;

            let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)?
                .and_hms_opt(hour, min, sec)?;
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }

        // English format: Added on Tuesday, September 15, 2026 8:43:10 PM
        // or Added on 15 September 2026 20:43:10
        let en_prefixes = ["Added on ", "on "];
        let mut date_str = "";
        for prefix in en_prefixes {
            if let Some(idx) = line.find(prefix) {
                date_str = line[idx + prefix.len()..].trim();
                break;
            }
        }

        if !date_str.is_empty() {
            // Try standard formats
            let formats = [
                "%A, %B %d, %Y %I:%M:%S %p",
                "%B %d, %Y %I:%M:%S %p",
                "%A, %d %B %Y %H:%M:%S",
                "%d %B %Y %H:%M:%S",
                "%Y-%m-%d %H:%M:%S",
            ];

            for fmt in formats {
                if let Ok(naive) = NaiveDateTime::parse_from_str(date_str, fmt) {
                    return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
                }
            }
        }

        None
    }

    fn generate_id(
        title: &str,
        location: &Option<String>,
        clipping_type: &ClippingType,
        content: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(title.trim().as_bytes());
        hasher.update(b":");
        hasher.update(location.as_deref().unwrap_or("").as_bytes());
        hasher.update(b":");
        hasher.update(clipping_type.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(content.trim().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_japanese_clipping() {
        let sample = r#"﻿クリーンアーキテクチャ (Robert C. Martin)
- あなたのハイライト (位置No. 245-247) | 作成日時: 2026年9月18日金曜日 21:30:15

アーキテクチャの目的は、求められるシステムを構築・保守するために必要な人材を最小限に抑えることである。
==========
クリーンアーキテクチャ (Robert C. Martin)
- あなたのメモ (位置No. 250) | 作成日時: 2026年9月18日金曜日 21:32:00

ここは重要な設計原則
=========="#;

        let clippings = ClippingsParser::parse_str(sample);
        assert_eq!(clippings.len(), 2);

        assert_eq!(clippings[0].book_title, "クリーンアーキテクチャ");
        assert_eq!(clippings[0].author, Some("Robert C. Martin".to_string()));
        assert_eq!(clippings[0].clipping_type, ClippingType::Highlight);
        assert_eq!(clippings[0].location, Some("245-247".to_string()));
        assert!(clippings[0].content.contains("アーキテクチャの目的"));
        assert!(clippings[0].created_at.is_some());

        assert_eq!(clippings[1].clipping_type, ClippingType::Note);
        assert_eq!(clippings[1].location, Some("250".to_string()));
        assert_eq!(clippings[1].content, "ここは重要な設計原則");
    }

    #[test]
    fn test_parse_english_clipping() {
        let sample = r#"Thinking, Fast and Slow (Daniel Kahneman)
- Your Highlight on page 12 | location 180-181 | Added on Tuesday, September 15, 2026 8:43:10 PM

A reliable way to make people believe in falsehoods is frequent repetition.
==========
"#;

        let clippings = ClippingsParser::parse_str(sample);
        assert_eq!(clippings.len(), 1);
        assert_eq!(clippings[0].book_title, "Thinking, Fast and Slow");
        assert_eq!(clippings[0].author, Some("Daniel Kahneman".to_string()));
        assert_eq!(clippings[0].page, Some(12));
        assert_eq!(clippings[0].location, Some("180-181".to_string()));
        assert!(clippings[0].created_at.is_some());
    }

    #[test]
    fn test_parse_scribe_actual_clipping() {
        let sample = r#"﻿言語化するための小説思考 (小川哲)
- 6ページ|位置No. 34-35のハイライト |作成日: 2026年5月4日月曜日 20:26:13

小説の技術的な側面とは、（誤解を恐れつつ小声で言うならば）「読者の動物的なバグを利用したハッキング技術」に近い。
==========
"#;
        let clippings = ClippingsParser::parse_str(sample);
        assert_eq!(clippings.len(), 1);
        assert_eq!(clippings[0].book_title, "言語化するための小説思考");
        assert_eq!(clippings[0].author, Some("小川哲".to_string()));
        assert_eq!(clippings[0].page, Some(6));
        assert_eq!(clippings[0].location, Some("34-35".to_string()));
        assert_eq!(clippings[0].clipping_type, ClippingType::Highlight);
        assert!(clippings[0].created_at.is_some());
    }
}
