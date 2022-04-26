use difference::{Changeset, Difference};
use horrorshow::Raw;
use regex::Regex;
use super::ordio_test::IODiff;
use super::testresult::HTMLError;

fn decdata_to_hexdump(decdata: &str, offset: &mut usize, num_lines: &mut isize) -> String {
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
        hexdump.push_str(&format!("0x{:0>7X}  &#x250a  {:<47}  &#x250a  {:<16}<br>", offset, hex, ascii).replace(" ", "&nbsp;"));
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

    let changeset = Changeset::new(&giv_str ,&ref_str, " ");
    let distance = changeset.distance;

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();
                        let mut linesright: isize = 0;
                        let mut linesleft: isize = 0;
                        let mut linescarry: isize = 0;
                        let mut offleft = 0;
                        let mut offright = 0;

                        for c in &changeset.diffs {
                            match *c {
                                Difference::Same(ref z)=>
                                {
                                    if linescarry > 0 {
                                        for _ in 0..linescarry {
                                            diffleft.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                                            linesleft += 1;
                                        }
                                        linesright += linescarry;
                                        linescarry = 0;
                                    }

                                    diffright.push_str(&format!("{}\n", decdata_to_hexdump(z, &mut offright, &mut linesright)));
                                    diffleft.push_str(&format!("{}\n", decdata_to_hexdump(z, &mut offleft, &mut linesleft)));
                                }
                                Difference::Rem(ref z) =>
                                {
                                    diffright.push_str(&format!("<span id =\"diff-remove\">{}\n</span>",
                                            decdata_to_hexdump(z, &mut offright, &mut linesright)));
                                    linescarry = linesright - linesleft;
                                    linesright -= linescarry;
                                }

                                Difference::Add(ref z) =>
                                {
                                    diffleft.push_str(&format!("<span id =\"diff-add\">{}\n</span>",
                                            decdata_to_hexdump(z, &mut offleft, &mut linesleft)));
                                    linesright += linescarry;
                                    linescarry = 0;
                                }
                            }

                            let linesdiff = linesright - linesleft;
                            if linesdiff > 0 {
                                for _ in 0..linesdiff {
                                    diffleft.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                                    linesleft += 1;
                                }
                            }
                            else if linesdiff < 0 {
                                let linesdiff = linesdiff * -1;
                                for _ in 0..linesdiff {
                                    diffright.push_str(&format!("{}&#x250a{}&#x250a{}<br>", "&nbsp;".repeat(11), "&nbsp;".repeat(51), "&nbsp;".repeat(18)));
                                    linesright += 1;
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

pub fn changeset_to_html(changes: &Changeset, compare_mode: &str, with_ws_hints: bool, source_name: &str) -> Result<String, HTMLError> {
    let line_end = if compare_mode == "\n" { "\n" } else { "" };

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();

                        let re = Regex::new(r"(?P<m>(?:&middot;|\t|\n|\x00)+)").unwrap();

                        for c in &changes.diffs {
                            match *c {
                                Difference::Same(ref z)=>
                                {
                                    if with_ws_hints {
                                        diffright.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                    &z.replace(" ", "&middot;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                        diffleft.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                    &z.replace(" ", "&middot;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                    else {
                                        diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;"), line_end));
                                        diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                }
                                Difference::Rem(ref z) =>
                                {
                                    if with_ws_hints {
                                        diffright.push_str(&format!("<span id=\"diff-add\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                re.replace_all(&z.replace(" ", "&middot;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                    else {
                                        diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                }

                                Difference::Add(ref z) =>
                                {
                                    if with_ws_hints {
                                        diffleft.push_str(&format!("<span id=\"diff-remove\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                re.replace_all(&z.replace(" ", "&middot;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                    else {
                                        diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;"), line_end));
                                    }
                                }
                            }
                        }

                        if with_ws_hints {
                            diffright = diffright.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
                            diffleft = diffleft.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
                        }
                        else {
                            diffright = diffright.replace("\n", "<br />").replace("\0", "<br />");
                            diffleft = diffleft.replace("\n", "<br />").replace("\0", "<br />");
                        }

                        &mut *templ << Raw(format!("<tr><th>Reference {}</th><th>Your {}</th></tr><tr><td id=\"orig\">{}</td><td id=\"edit\">{}</td></tr>", source_name, source_name, diffright, diffleft));
                    }
                }
            }
    });
    Ok(String::from(retvar))
}

pub fn iodiff_to_html(changes: &[IODiff], compare_mode: &str, with_ws_hints: bool, source_name: &str) -> Result<String, HTMLError> {
    let line_end = if compare_mode == "\n" { "\n" } else { "" };

    let retvar = format!(
        "{}",
        box_html! {
            div(id="diff") {
                table(id="differences") {
                    |templ| {
                        let mut diffright = String::new();
                        let mut diffleft = String::new();

                        let re = Regex::new(r"(?P<m>(?:&middot;|\t|\n|\x00)+)").unwrap();

                        changes.iter().for_each(|io_diff| {
                            match io_diff {
                                IODiff::Input(input) => {
                                    if with_ws_hints {
                                        diffright.push_str(&format!("<span id=\"diff-input\">{}</span>", re.replace_all(
                                                &input.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;")));
                                        diffleft.push_str(&format!("<span id=\"diff-input\">{}</span>", re.replace_all(
                                                &input.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;")));
                                    }
                                    else {
                                        diffright.push_str(&format!("{}", input.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;")));
                                        diffleft.push_str(&format!("{}", input.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;")));
                                    }
                                },
                                IODiff::Output(changeset) => {
                                    for c in &changeset.diffs {
                                        match *c {
                                            Difference::Same(ref z)=>
                                            {
                                                if with_ws_hints {
                                                    diffright.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                                &z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                    diffleft.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                                &z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }
                                            Difference::Rem(ref z) =>
                                            {
                                                if with_ws_hints {
                                                    diffright.push_str(&format!("<span id=\"diff-add\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                            re.replace_all(&z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }

                                            Difference::Add(ref z) =>
                                            {
                                                if with_ws_hints {
                                                    diffleft.push_str(&format!("<span id=\"diff-remove\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                            re.replace_all(&z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }
                                        }
                                    }
                                },
                                IODiff::OutputQuery(changeset) => {
                                    let mut it = changeset.diffs.iter().peekable();
                                    while let Some(c) = it.next() {
                                        let line_end;
                                        if it.peek().is_none() {
                                            line_end = "";
                                        }
                                        else {
                                            line_end = if compare_mode == "\n" { "\n" } else { "" };
                                        }

                                        match *c {
                                            Difference::Same(ref z)=>
                                            {
                                                if with_ws_hints {
                                                    diffright.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                                &z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                    diffleft.push_str(&format!("{}<span class=\"whitespace-hint\">{}</span>", re.replace_all(
                                                                &z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }
                                            Difference::Rem(ref z) =>
                                            {
                                                if with_ws_hints {
                                                    diffright.push_str(&format!("<span id=\"diff-add\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                            re.replace_all(&z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffright.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }

                                            Difference::Add(ref z) =>
                                            {
                                                if with_ws_hints {
                                                    diffleft.push_str(&format!("<span id=\"diff-remove\">{}<span class=\"whitespace-hint\">{}</span></span>",
                                                            re.replace_all(&z.replace(" ", "&middot;").replace("<", "&lt;").replace(">", "&gt;"), "<span class=\"whitespace-hint\">${m}</span>").replace("\t", "&#x21a6;&nbsp;&nbsp;&nbsp;"), line_end));
                                                }
                                                else {
                                                    diffleft.push_str(&format!("{}{}", z.replace(" ", "&nbsp;").replace("\t", "&nbsp;&nbsp;&nbsp;&nbsp;").replace("<", "&lt;").replace(">", "&gt;"), line_end));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });

                        if with_ws_hints {
                            diffright = diffright.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
                            diffleft = diffleft.replace("\n", "&#x21b5;<br />").replace("\0", "&#x2205;<br />");
                        }
                        else {
                            diffright = diffright.replace("\n", "<br />").replace("\0", "<br />");
                            diffleft = diffleft.replace("\n", "<br />").replace("\0", "<br />");
                        }

                        &mut *templ << Raw(format!("<tr><th>Reference {}</th><th>Your {}</th></tr><tr><td id=\"orig\">{}</td><td id=\"edit\">{}</td></tr>", source_name, source_name, diffright, diffleft));
                    }
                }
            }
    });
    Ok(String::from(retvar))
}

