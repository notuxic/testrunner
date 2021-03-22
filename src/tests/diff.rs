use difference::{Changeset, Difference};
use horrorshow::Raw;
use super::testresult::HTMLError;

fn decdata_to_hexdump(decdata: &str, offset: &mut usize, num_lines: &mut isize) -> String {
    // let hexdata: Vec<u8> = hex_to_u8(hexdata);
    let decdata: Vec<u8> = decdata.split(' ').map(|c| c.parse::<u8>().unwrap()).collect();
    let mut hexdump = String::with_capacity(81);
    for chunk in decdata.chunks(16) {
        let hex = chunk.iter().map(|c| {
            String::from(format!("{:0>2X}", c))
        }).collect::<Vec<String>>().join(" ");
        let ascii = chunk.iter().map(|c| {
            let c = *c as char;
            if c.is_ascii_graphic() {
                c
            } else {
                '.'
            }
        }).collect::<String>();
        hexdump.push_str(&format!("0x{:0>7X}  &#x2502  {:<47}  &#x2502  {:<16}<br>", offset, hex, ascii).replace(" ", "&nbsp;"));
        *offset += chunk.len();
        *num_lines += 1;
    }
    hexdump
}

pub fn diff_binary_to_html(reference: &[u8], given: &[u8]) -> Result<(String, i32), HTMLError> {
    let mut ref_str = String::with_capacity(reference.len() * 4);
    let mut giv_str = String::with_capacity(given.len() * 4);

    for value in reference.iter() {
        ref_str.push_str(&format!("{} ", value));
    }
    ref_str.pop();

    for value in given.iter() {
        giv_str.push_str(&format!("{} ", value));
    }
    giv_str.pop();

    let changeset = Changeset::new(&ref_str, &giv_str, " ");
    let distance = changeset.distance;

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();
                        let mut linesleft: isize = 0;
                        let mut linesright: isize = 0;
                        let mut offright = 0;
                        let mut offleft = 0;

                        for c in &changeset.diffs {
                            match *c {
                                Difference::Same(ref z)=>
                                {
                                    diffright.push_str(&format!("{}\n", decdata_to_hexdump(z, &mut offright, &mut linesright)));
                                    diffleft.push_str(&format!("{}\n", decdata_to_hexdump(z, &mut offleft, &mut linesleft)));
                                }
                                Difference::Rem(ref z) =>
                                {
                                    diffleft.push_str(&format!("<span id =\"diff-add\">{}\n</span>",
                                            decdata_to_hexdump(z, &mut offleft, &mut linesleft)));
                                }

                                Difference::Add(ref z) =>
                                {
                                    diffright.push_str(&format!("<span id =\"diff-remove\">{}\n</span>",
                                            decdata_to_hexdump(z, &mut offright, &mut linesright)));
                                }
                            }

                            let linesdiff = linesleft - linesright;
                            if linesdiff > 0 {
                                for _ in 0..linesdiff {
                                    diffright.push_str(&format!("{}&#x2502{}&#x2502{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                                    linesright += 1;
                                }
                            }
                            else if linesdiff < 0 {
                                let linesdiff = linesdiff * -1;
                                for _ in 0..linesdiff {
                                    diffleft.push_str(&format!("{}&#x2502{}&#x2502{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                                    linesleft += 1;
                                }
                            }
                        }

                        &mut *templ << Raw(format!("<tr><th>Reference File</th><th>Your File</th></tr><tr><td id=\"orig\">{}</td><td id=\"edit\">{}</td></tr>",
                                diffleft, diffright));
                    }
                }
            }
    });
    Ok((String::from(retvar), distance))
}

pub fn changeset_to_html(changes: &Changeset, compare_mode : &str) -> Result<String, HTMLError> {
    let line_end = if compare_mode == "\n" { "\n" } else { "" };

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();

                        for c in &changes.diffs {
                            match *c {
                                Difference::Same(ref z)=>
                                {
                                    diffright.push_str(&format!("{}{}", z.replace(" ", "<span class=\"whitespace-hint\">&middot;</span>").replace("\t", "<span class=\"whitespace-hint\">&#x21a6;&nbsp;&nbsp;&nbsp;</span>"), line_end));
                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "<span class=\"whitespace-hint\">&middot;</span>").replace("\t", "<span class=\"whitespace-hint\">&#x21a6;&nbsp;&nbsp;&nbsp;</span>"), line_end));
                                }
                                Difference::Rem(ref z) =>
                                {
                                    diffleft.push_str(&format!("<span id =\"diff-add\">{}{}</span>",
                                            z.replace(" ", "<span class=\"whitespace-hint\">&middot;</span>").replace("\t", "<span class=\"whitespace-hint\">&#x21a6;&nbsp;&nbsp;&nbsp;</span>"), line_end));
                                }

                                Difference::Add(ref z) =>
                                {
                                    diffright.push_str(&format!("<span id =\"diff-remove\">{}{}</span>",
                                            z.replace(" ", "<span class=\"whitespace-hint\">&middot;</span>").replace("\t", "<span class=\"whitespace-hint\">&#x21a6;&nbsp;&nbsp;&nbsp;</span>"), line_end));
                                }

                            }
                        }

                        &mut *templ << Raw(format!("<tr><th>Reference Output</th><th>Your Output</th></tr><tr><td id=\"orig\">{}</td><td id=\"edit\">{}</td></tr>",
                                diffleft.replace("\n", "<span class=\"whitespace-hint\">&#x21b5;</span><br>").replace("\0", "<span class=\"whitespace-hint\">&#x2205;</span><br>"),
                                diffright.replace("\n", "<span class=\"whitespace-hint\">&#x21b5;</span><br>").replace("\0", "<span class=\"whitespace-hint\">&#x2205;</span><br>")));
                    }
                }
            }
    });
    Ok(String::from(retvar))
}

