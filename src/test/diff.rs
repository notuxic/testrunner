use std::time::{Duration, Instant};

use regex::Regex;
use serde_derive::Serialize;
use similar::{Algorithm, ChangeTag, TextDiff, capture_diff_slices_deadline, get_diff_ratio};

use super::ordio_test::IODiff;


#[derive(Clone, Debug, Serialize)]
pub enum ChangesetFlat<T> {
    Same(T),
    Add(T),
    Remove(T),
}

#[derive(Clone, Debug, Serialize)]
pub enum ChangesetInline<T> {
    Same(Vec<ChangesetFlat<T>>),
    Add(Vec<ChangesetFlat<T>>),
    Remove(Vec<ChangesetFlat<T>>),
}


pub fn diff_plaintext(old: &str, new: &str, timeout: Duration) -> (Vec<ChangesetInline<String>>, f32) {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .timeout(timeout)
        .newline_terminated(true)
        .diff_lines(old, new);

    let mut changeset = Vec::with_capacity(diff.ops().len());
    diff.ops().iter().for_each(|op| {
        let mut changes = diff.iter_inline_changes(op).map(|change| {
            match change.tag() {
                ChangeTag::Equal => {
                    ChangesetInline::Same(change.iter_strings_lossy().map(|(_, value)| {
                        ChangesetFlat::Same(value.to_string())
                    }).collect::<Vec<ChangesetFlat<String>>>())
                },
                ChangeTag::Delete => {
                    ChangesetInline::Remove(change.iter_strings_lossy().map(|(emph, value)| {
                        if emph {
                            ChangesetFlat::Remove(value.to_string())
                        }
                        else {
                            ChangesetFlat::Same(value.to_string())
                        }
                    }).collect::<Vec<ChangesetFlat<String>>>())
                },
                ChangeTag::Insert => {
                    ChangesetInline::Add(change.iter_strings_lossy().map(|(emph, value)| {
                        if emph {
                            ChangesetFlat::Add(value.to_string())
                        }
                        else {
                            ChangesetFlat::Same(value.to_string())
                        }
                    }).collect::<Vec<ChangesetFlat<String>>>())
                },
            }
        }).collect::<Vec<ChangesetInline<String>>>();
        changeset.append(&mut changes);
    });

    (changeset, diff.ratio())
}

pub fn diff_binary(old: &[u8], new: &[u8], timeout: Duration) -> (Vec<ChangesetFlat<Vec<u8>>>, f32) {
    let diff = capture_diff_slices_deadline(
        Algorithm::Patience,
        old,
        new,
        Some(Instant::now() + timeout)
    );

    let changeset = diff.iter()
        .flat_map(|op| op.iter_slices(old, new))
        .map(|(tag, value)| {
            match tag {
                ChangeTag::Equal => ChangesetFlat::Same(Vec::from(value)),
                ChangeTag::Delete => ChangesetFlat::Remove(Vec::from(value)),
                ChangeTag::Insert => ChangesetFlat::Add(Vec::from(value)),
            }
        }).collect();

    (changeset, get_diff_ratio(&diff, old.len(), new.len()))
}

pub fn with_ws_hints(text: &str, ws_hints: bool) -> String {
    if ws_hints {
        let re = Regex::new(r"(?P<m>(?:&middot;|\t|\n|\x00)+)").unwrap();
        re.replace_all(
            &text.replace("&", "&amp;").replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"),
            "<span class=\"whitespace-hint\">${m}</span>"
            ).replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;")
    }
    else {
        text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;")
    }
}

pub fn textdiff_to_html(changeset: &Vec<ChangesetInline<String>>, ws_hints: bool) -> (String, String) {
    let mut diff_left = String::new();
    let mut diff_right = String::new();

    changeset.iter().for_each(|change| {
        match change {
            ChangesetInline::Same(line) => {
                line.iter().for_each(|segment| {
                    match segment {
                        ChangesetFlat::Same(text) => {
                            diff_left.push_str(&with_ws_hints(text, ws_hints));
                            diff_right.push_str(&with_ws_hints(text, ws_hints));
                        },
                        _ => {}, // ChangesetInline::Same only contains ChangesetFlat::Same
                    }
                });
            },
            ChangesetInline::Remove(line) => {
                diff_left.push_str("<span class=\"diff-add\">");
                line.iter().for_each(|segment| {
                    match segment {
                        ChangesetFlat::Same(text) => diff_left.push_str(&with_ws_hints(text, ws_hints)),
                        ChangesetFlat::Remove(text) => diff_left.push_str(&format!("<span class=\"diff-add-inline\">{}</span>", &with_ws_hints(text, ws_hints))),
                        _ => {},
                    }
                });
                diff_left.push_str("</span>");
            },
            ChangesetInline::Add(line) => {
                diff_right.push_str("<span class=\"diff-remove\">");
                line.iter().for_each(|segment| {
                    match segment {
                        ChangesetFlat::Same(text) => diff_right.push_str(&with_ws_hints(text, ws_hints)),
                        ChangesetFlat::Add(text) => diff_right.push_str(&format!("<span class=\"diff-remove-inline\">{}</span>", &with_ws_hints(text, ws_hints))),
                        _ => {},
                    }
                });
                diff_right.push_str("</span>");
            },
        }
    });

    if ws_hints {
        diff_left = diff_left.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
        diff_right = diff_right.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
    }
    else {
        diff_left = diff_left.replace("\n", "<br />").replace("\0", "<br />");
        diff_right = diff_right.replace("\n", "<br />").replace("\0", "<br />");
    }

    (diff_left, diff_right)
}

fn binarydata_to_hexdump(data: &[u8], offset: &mut usize, num_lines: &mut isize) -> String {
    let mut hexdump = String::with_capacity(81);
    for chunk in data.chunks(16) {
        let hex = chunk.iter().map(|c| {
            format!("{:0>2X}", c)
        }).collect::<Vec<String>>().join(" ");
        let ascii = chunk.iter().map(|c| {
            let c = *c as char;
            if c.is_ascii_graphic() {
                c
            } else {
                '.'
            }
        }).collect::<String>();
        hexdump.push_str(&format!("0x{:0>7X}  &#x250a  {:<47}  &#x250a  {:<16}<br>", offset, hex, ascii).replace(" ", "&nbsp;"));
        *offset += chunk.len();
        *num_lines += 1;
    }
    hexdump
}

pub fn binarydiff_to_html(changeset: &Vec<ChangesetFlat<Vec<u8>>>) -> (String, String) {
    let mut diff_left = String::new();
    let mut diff_right = String::new();

    let mut lines_left: isize = 0;
    let mut lines_right: isize = 0;
    let mut lines_carry: isize = 0;
    let mut off_left = 0;
    let mut off_right = 0;

    changeset.iter().for_each(|change| {
        match change {
            ChangesetFlat::Same(block) => {
                if lines_carry > 0 {
                    for _ in 0..lines_carry {
                        diff_right.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                        lines_left += 1;
                    }
                    lines_right += lines_carry;
                    lines_carry = 0;
                }

                diff_left.push_str(&binarydata_to_hexdump(block, &mut off_right, &mut lines_right));
                diff_right.push_str(&binarydata_to_hexdump(block, &mut off_left, &mut lines_left));
            },
            ChangesetFlat::Remove(block) => {
                diff_left.push_str("<span class=\"diff-add\">");
                diff_left.push_str(&binarydata_to_hexdump(block, &mut off_right, &mut lines_right));
                diff_left.push_str("</span>");

                lines_carry = lines_right - lines_left;
                lines_right -= lines_carry;
            },
            ChangesetFlat::Add(block) => {
                diff_right.push_str("<span class=\"diff-remove\">");
                diff_right.push_str(&binarydata_to_hexdump(block, &mut off_left, &mut lines_left));
                diff_right.push_str("</span>");

                lines_right += lines_carry;
                lines_carry = 0;
            }
        }

        let lines_diff = lines_right - lines_left;
        if lines_diff > 0 {
            for _ in 0..lines_diff {
                diff_right.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                lines_left += 1;
            }
        }
        else if lines_diff < 0 {
            let lines_diff = lines_diff * -1;
            for _ in 0..lines_diff {
                diff_left.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                lines_right += 1;
            }
        }
    });

    (diff_left, diff_right)
}

pub fn iodiff_to_html(changeset: &[IODiff], ws_hints: bool) -> (String, String) {
    let mut diff_left = String::new();
    let mut diff_right = String::new();

    changeset.iter().for_each(|io_diff| {
        match io_diff {
            IODiff::Input(input) => {
                diff_left.push_str(&format!("<span class=\"diff-input\">{}</span>", &with_ws_hints(input, ws_hints)));
                diff_right.push_str(&format!("<span class=\"diff-input\">{}</span>", &with_ws_hints(input, ws_hints)));
            },
            IODiff::InputUnsent(input) => {
                diff_left.push_str(&format!("<span class=\"diff-input-unsent\">{}</span>", &with_ws_hints(input, ws_hints)));
            },
            IODiff::Output(changes) => {
                changes.iter().for_each(|change| {
                    match change {
                        ChangesetInline::Same(line) => {
                            line.iter().for_each(|segment| {
                                match segment {
                                    ChangesetFlat::Same(text) => {
                                        diff_left.push_str(&with_ws_hints(text, ws_hints));
                                        diff_right.push_str(&with_ws_hints(text, ws_hints));
                                    },
                                    _ => {}, // ChangesetInline::Same only contains ChangesetFlat::Same
                                }
                            });
                        },
                        ChangesetInline::Remove(line) => {
                            diff_left.push_str("<span class=\"diff-add\">");
                            line.iter().for_each(|segment| {
                                match segment {
                                    ChangesetFlat::Same(text) => diff_left.push_str(&with_ws_hints(text, ws_hints)),
                                    ChangesetFlat::Remove(text) => diff_left.push_str(&format!("<span class=\"diff-add-inline\">{}</span>", &with_ws_hints(text, ws_hints))),
                                    _ => {},
                                }
                            });
                            diff_left.push_str("</span>");
                        },
                        ChangesetInline::Add(line) => {
                            diff_right.push_str("<span class=\"diff-remove\">");
                            line.iter().for_each(|segment| {
                                match segment {
                                    ChangesetFlat::Same(text) => diff_right.push_str(&with_ws_hints(text, ws_hints)),
                                    ChangesetFlat::Add(text) => diff_right.push_str(&format!("<span class=\"diff-remove-inline\">{}</span>", &with_ws_hints(text, ws_hints))),
                                    _ => {},
                                }
                            });
                            diff_right.push_str("</span>");
                        },
                    }
                });
            }
        }
    });

    if ws_hints {
        diff_left = diff_left.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
        diff_right = diff_right.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
    }
    else {
        diff_left = diff_left.replace("\n", "<br />").replace("\0", "<br />");
        diff_right = diff_right.replace("\n", "<br />").replace("\0", "<br />");
    }

    (diff_left, diff_right)
}

