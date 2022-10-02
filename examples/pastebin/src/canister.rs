use std::collections::HashMap;

use serde::Serialize;
use tinytemplate::TinyTemplate;

use ic_kit::prelude::*;

pub type Data = HashMap<String, Vec<u8>>;

#[derive(Serialize)]
struct HtmlContext {
    manpage: String,
}

#[derive(Serialize)]
struct ManpageContext {
    url: String,
}

static HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html>
    <head>
        <title>IC Pastebin</title>
        <style>
            body \{
                background: #1e1e1e;
                color: #d4d4d4;
            \}
        </style>
    </head>
    <body>
        <pre>
            <code>
{manpage}
            </code>
        </pre>
    </body>
</html>
"#;

static MANPAGE_TEMPLATE: &str = r#"
IC PASTEBIN(1)                      IC PASTEBIN                       IC PASTEBIN(1)

NAME

        ic-pastebin - HTTP pastebin canister for the Internet Computer 

DESCRIPTION

        The ic-pastebin canister is a simple pastebin canister that allows users to
        upload text and get a URL to share it with others.

        The canister is written in Rust and uses the ic-kit library to provide
        access to the Internet Computer.

USAGE

        curl -T file.txt {url}
        curl {url}/file.txt
"#;

/// Index handler
#[get(route = "/")]
fn index_handler(r: HttpRequest, _: Params) -> HttpResponse {
    ic::print(format!("{:?}", r));
    let url = match r.header("host") {
        Some(host) => format!("http://{}", host),
        None => format!("https://{}.raw.ic0.app", id()),
    };

    let mut tt = TinyTemplate::new();

    tt.add_template("manpage", MANPAGE_TEMPLATE).unwrap();
    let manpage = tt.render("manpage", &ManpageContext { url }).unwrap();

    // Just return the manpage if client is a terminal (curl or wget)
    if let Some(ua) = r.header("User-Agent") {
        if ua.starts_with("curl") || ua.starts_with("wget") {
            return HttpResponse::ok().with_body(manpage);
        }
    }

    tt.add_template("html", HTML_TEMPLATE).unwrap();
    let html = tt.render("html", &HtmlContext { manpage }).unwrap();

    HttpResponse::ok().with_body(html)
}

/// Get paste handler
#[get(route = "/:file")]
fn get_file(_: HttpRequest, p: Params) -> HttpResponse {
    let file = p.get("file").unwrap();
    with(|data: &Data| match data.get(file) {
        Some(content) => HttpResponse::ok().with_body(content.clone()),
        None => HttpResponse::new(404).with_body(format!("file not found `{}`\n", file)),
    })
}

/// Upload paste handler
#[put(route = "/:file", upgrade = true)]
fn put_file(req: HttpRequest, p: Params) -> HttpResponse {
    let filename = p.get("file").unwrap();
    let url = req.header("host").unwrap_or("unknown");

    let res = format!("{}.{}/{}", id().to_text(), "localhost:8000", filename);

    with_mut(|d: &mut Data| {
        d.insert(filename.to_string(), req.body);
    });

    HttpResponse::ok().with_body(res)
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct PastebinCanister;
