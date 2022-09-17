use ic_kit::prelude::*;
use std::collections::HashMap;

pub type Data = HashMap<String, Vec<u8>>;

use serde::Serialize;

use std::error::Error;
use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct IndexContext {
    manpage: String,
}

#[derive(Serialize)]
struct ManpageContext {
    canister_id: String,
}

static INDEX_HTML: &'static str = r#"
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

const INDEX_MANPAGE: &str = r#"
IC PASTEBIN(1)                      IC PASTEBIN                       IC PASTEBIN(1)

NAME

        ic-pastebin - HTTP pastebin canister for the Internet Computer 

DESCRIPTION

        The ic-pastebin canister is a simple pastebin canister that allows users to
        upload text and get a URL to share it with others.

        The canister is written in Rust and uses the ic-kit library to provide
        access to the Internet Computer.

USAGE

        curl -T file.txt https://{canister_id}.raw.ic0.app
        curl https://{canister_id}.raw.ic0.app/file.txt"#;

/// Index handler
#[get(route = "/")]
fn index_handler(r: HttpRequest, _: Params) -> HttpResponse {
    let mut tt = TinyTemplate::new();

    tt.add_template("manpage", INDEX_MANPAGE).unwrap();
    let manpage = tt
        .render(
            "manpage",
            &ManpageContext {
                canister_id: ic::id().to_text(),
            },
        )
        .unwrap();

    // Just return the manpage if client is a terminal (curl or wget)
    if let Some(ua) = r.header("User-Agent") {
        if ua.starts_with("curl") || ua.starts_with("wget") {
            return HttpResponse::ok().with_body(manpage);
        }
    }

    tt.add_template("html", INDEX_HTML).unwrap();
    let html = tt.render("html", &IndexContext { manpage }).unwrap();

    HttpResponse::ok().with_body(html)
}

/// Get paste handler
#[get(route = "/:file")]
fn get_file(_: HttpRequest, p: Params) -> HttpResponse {
    let file = p.get("file").unwrap();
    ic::with(|data: &Data| match data.get(file) {
        Some(content) => HttpResponse::ok().with_body(content.clone()),
        None => HttpResponse::new(404).with_body(format!("file not found `{}`", file)),
    })
}

/// Upload paste handler
#[put(route = "/:file", upgrade = true)]
fn put_file(req: HttpRequest, p: Params) -> HttpResponse {
    let filename = p.get("file").unwrap();
    let res = format!("recieved file: {} ({} bytes)", filename, req.body.len(),);

    ic::with_mut(|d: &mut Data| {
        d.insert(filename.to_string(), req.body);
    });

    HttpResponse::ok().with_body(res)
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct PastebinCanister;
