use rayon::prelude::*;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use std::fs::File;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Serialize)]
struct Thiruppugazh {
    id: u32,
    lyrics: String,
    meaning: String,
}

pub fn thiruppugazh(output_path: &str) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client");

    // Allowing variable amounts of dots and whitespace
    let padal_re = Regex::new(r"\.+\s*பாடல்\s*\.+").unwrap();
    let vilakkam_re = Regex::new(r"\.+\s*சொல் விளக்கம்\s*\.+").unwrap();
    let tag_re = Regex::new(r"(?i)<[^>]+>").unwrap();

    // The range defined by the user
    const START_ID: u32 = 6;
    const END_ID: u32 = 1340;

    // Process using Rayon for parallel execution
    let mut songs: Vec<Thiruppugazh> = (START_ID..=END_ID)
        .into_par_iter()
        .filter_map(|id| {
            let url = format!("https://www.kaumaram.com/thiru/nnt{:04}_u.html", id);

            // Retry logic (up to 3 times) for robust networking
            let mut html = None;
            for _ in 0..3 {
                if let Ok(resp) = client.get(&url).send()
                    && resp.status().is_success()
                    && let Ok(text) = resp.text()
                {
                    html = Some(text);
                    println!("Scrapped thiruppugazh {:04}", &id);
                    break;
                }
                std::thread::sleep(Duration::from_millis(512));
            }

            let html_text = html?;
            parse_html(id, &html_text, &padal_re, &vilakkam_re, &tag_re)
        })
        .collect();

    // Sort by ID to ensure the resulting JSON is perfectly ordered
    // (Rayon processes unordered so we must sort at the end)
    songs.sort_by_key(|s| s.id);

    // Write to a pretty-formatted JSON file
    let file = File::create(output_path).expect("Failed to create output file");
    serde_json::to_writer_pretty(file, &songs).expect("Failed to write JSON");

    println!(
        "Successfully scraped and saved {} entries to output.json",
        songs.len()
    );
}
static SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td.t2l.ttxt.pad30").unwrap());

fn parse_html(
    id: u32,
    html: &str,
    padal_re: &Regex,
    vilakkam_re: &Regex,
    tag_re: &Regex,
) -> Option<Thiruppugazh> {
    let document = Html::parse_document(html);

    // Select the table cell holding the target data
    let container = document.select(&SELECTOR).next()?;
    let inner_html = container.inner_html();

    // Split HTML by "......... பாடல் ........."
    let parts: Vec<&str> = padal_re.split(&inner_html).collect();
    if parts.len() < 2 {
        return None; // Cannot find lyrics start point
    }

    // Split the remaining section by "......... சொல் விளக்கம் ........."
    let sub_parts: Vec<&str> = vilakkam_re.split(parts[1]).collect();

    let lyrics_raw = sub_parts[0];
    let meaning_raw = if sub_parts.len() > 1 {
        sub_parts[1]
    } else {
        ""
    };

    Some(Thiruppugazh {
        id,
        lyrics: clean_text(lyrics_raw, tag_re),
        meaning: clean_text(meaning_raw, tag_re),
    })
}

fn clean_text(input: &str, tag_re: &Regex) -> String {
    /* Replace <br> variations with newline strings
    Replace HTML &nbsp; entity with real spaces*/
    let with_newlines = input
        .replace("<br>", "\n")
        .replace("<BR>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("&nbsp;", " ");

    // Strip all leftover HTML tags like <strong>, <td>, etc.
    let no_tags = tag_re.replace_all(&with_newlines, "");

    // Remove excessive blank lines while preserving intentional paragraph breaks
    let mut lines = Vec::new();
    let mut prev_empty = false;

    for line in no_tags.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                lines.push("");
                prev_empty = true;
            }
        } else {
            lines.push(trimmed);
            prev_empty = false;
        }
    }

    lines.join("\n").trim().to_string()
}
