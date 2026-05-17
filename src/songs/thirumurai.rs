use rayon::prelude::*;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use std::cell::LazyCell;
use std::fs::File;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Serialize)]
struct Thirumurai {
    id: String,      // e.g., "1.001"
    url: String,     // The resolved URL
    lyrics: String,  // Holds the title, metadata, and stanzas
    meaning: String, // Kept empty as per the schema (no meaning section on site)
}

pub fn thirumurai() {
    println!("Starting Mayuragiri scraper...");

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client");

    let tag_re = Regex::new(r"(?i)<[^>]+>").unwrap();

    // Define the ranges for Volumes 1 through 8
    const RANGES: [(u32, u32); 8] = [
        (1, 136),
        (2, 122),
        (3, 90),
        (4, 113),
        (5, 100),
        (6, 99),
        (7, 101),
        (8, 50),
    ];

    // Flatten targets into a single array for Rayon
    let mut targets = Vec::new();
    for (vol, max_song) in RANGES {
        for song in 1..=max_song {
            targets.push((vol, song));
        }
    }

    println!("Total URLs to process: {}", targets.len());

    let mut songs: Vec<Thirumurai> = targets
        .into_par_iter()
        .filter_map(|(vol, song)| {
            // Smart URL generation: Try 3-digit format first, then fallback to 2-digit format
            let urls_to_try = vec![
                format!("https://mayuragiri.com/{}sivasiva{:03}/", vol, song),
                format!("https://mayuragiri.com/{}sivasiva{:02}/", vol, song),
            ];

            let mut html_text = None;
            let mut resolved_url = String::new();

            for url in urls_to_try {
                // Retry network logic for each URL formulation
                for _ in 0..2 {
                    if let Ok(resp) = client.get(&url).send()
                        && resp.status().is_success()
                        && let Ok(text) = resp.text()
                    {
                        html_text = Some(text);
                        resolved_url = url.clone();
                        println!("Scrapped song {song:03} from {vol}.");

                        break;
                    }
                    std::thread::sleep(Duration::from_millis(312));
                }
                if html_text.is_some() {
                    break;
                }
            }

            let html = html_text?;
            parse_html(vol, song, &resolved_url, &html, &tag_re)
        })
        .collect();

    // Sort the final output by Volume, then Song Number
    songs.sort_by_key(|s| {
        let parts: Vec<&str> = s.id.split('.').collect();
        let vol: u32 = parts[0].parse().unwrap_or(0);
        let num: u32 = parts[1].parse().unwrap_or(0);
        (vol, num)
    });

    // Save formatted JSON output
    let file = File::create("data/thirumurai.json").expect("Failed to create output file");
    serde_json::to_writer_pretty(file, &songs).expect("Failed to write JSON");

    println!(
        "Successfully scraped and saved {} entries to data/thirumurai.json",
        songs.len()
    );
}

static CONTENT_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.entry-content").unwrap());

fn parse_html(vol: u32, song: u32, url: &str, html: &str, tag_re: &Regex) -> Option<Thirumurai> {
    let document = Html::parse_document(html);

    // The main container of the post
    let container = document.select(&CONTENT_SEL).next()?;

    let mut text_blocks = Vec::new();

    // Iterate over immediate child elements inside `div.entry-content`
    for child in container.children() {
        if let Some(element) = ElementRef::wrap(child) {
            // Skip the "sharedaddy" social media sharing block
            let class = element.value().attr("class").unwrap_or("");
            if class.contains("sharedaddy") {
                continue;
            }

            // Extract inner HTML to preserve <br> lines properly
            let inner_html = element.inner_html();
            let clean = clean_text(&inner_html, tag_re);

            if !clean.is_empty() {
                text_blocks.push(clean);
            }
        }
    }

    // Join all paragraphs/stanzas with double newlines
    let lyrics = text_blocks.join("\n\n");

    if lyrics.is_empty() {
        return None;
    }

    Some(Thirumurai {
        id: format!("{}.{:03}", vol, song), // e.g., "1.001"
        url: url.to_string(),
        lyrics,
        meaning: String::new(),
    })
}

// Cleans raw HTML nodes into pure structured text
fn clean_text(input: &str, tag_re: &Regex) -> String {
    // Convert all known line break tags into newline characters
    let with_newlines = input
        .replace("<br>", "\n")
        .replace("<BR>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("&nbsp;", " ");

    // Remove remaining HTML structural tags
    let no_tags = tag_re.replace_all(&with_newlines, "");

    // Decode common basic HTML entities natively
    let decoded = no_tags
        .replace("&#8211;", "-")
        .replace("&#8217;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    // Clean up empty lines and trailing spaces
    let mut lines = Vec::new();
    for line in decoded.lines() {
        let trimmed = line.trim();
        lines.push(trimmed);
    }

    lines.join("\n").trim().to_string()
}
